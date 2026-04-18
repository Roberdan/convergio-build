//! Build logic — runs cargo check/test/build and tracks results.

use crate::types::{BuildError, BuildRecord, BuildResult, BuildStatus};
use convergio_db::pool::ConnPool;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

/// Resolve the workspace root (where Cargo.toml lives).
pub fn workspace_root() -> PathBuf {
    // The binary runs from daemon/, so Cargo.toml is in the same dir
    let exe = std::env::current_exe().unwrap_or_default();
    // Walk up from binary location to find daemon/Cargo.toml
    for ancestor in exe.ancestors() {
        if ancestor.join("Cargo.toml").exists() && ancestor.join("crates").exists() {
            return ancestor.to_path_buf();
        }
    }
    // Fallback: assume CWD or known path
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Get current git commit hash.
pub fn current_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Create a new build record in the database.
pub fn create_build(pool: &ConnPool, commit: &str) -> BuildResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let conn = pool.get().map_err(|e| BuildError::Pool(e.to_string()))?;
    conn.execute(
        "INSERT INTO build_history (id, status, commit_hash) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, "queued", commit],
    )?;
    Ok(id)
}

/// Update build status.
pub fn update_status(pool: &ConnPool, id: &str, status: BuildStatus) -> BuildResult<()> {
    let conn = pool.get().map_err(|e| BuildError::Pool(e.to_string()))?;
    conn.execute(
        "UPDATE build_history SET status = ?1 WHERE id = ?2",
        rusqlite::params![status.to_string(), id],
    )?;
    Ok(())
}

/// Mark build as completed (success or failure).
pub fn complete_build(pool: &ConnPool, id: &str, record: &BuildRecord) -> BuildResult<()> {
    let conn = pool.get().map_err(|e| BuildError::Pool(e.to_string()))?;
    conn.execute(
        "UPDATE build_history SET status=?1, test_count=?2, binary_hash=?3, \
         binary_size=?4, completed_at=datetime('now'), error=?5, duration_secs=?6 \
         WHERE id=?7",
        rusqlite::params![
            record.status.to_string(),
            record.test_count,
            record.binary_hash,
            record.binary_size,
            record.error,
            record.duration_secs,
            id,
        ],
    )?;
    Ok(())
}

/// Run the full build pipeline: check → test → build --release.
pub fn run_build(workspace: &Path) -> BuildResult<(i64, String, i64)> {
    info!(workspace = %workspace.display(), "starting self-build pipeline");

    // Step 1: cargo check
    let out = Command::new("cargo")
        .args(["check", "--workspace"])
        .current_dir(workspace)
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(BuildError::BuildFailed(format!("cargo check: {stderr}")));
    }
    info!("cargo check passed");

    // Step 2: cargo test
    let out = Command::new("cargo")
        .args(["test", "--workspace"])
        .current_dir(workspace)
        .output()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let test_count = parse_test_count(&stdout);
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(BuildError::BuildFailed(format!("cargo test: {stderr}")));
    }
    info!(tests = test_count, "cargo test passed");

    // Step 3: cargo build --release
    let out = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(workspace)
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(BuildError::BuildFailed(format!("cargo build: {stderr}")));
    }

    // Hash + size of produced binary
    let binary = workspace.join("target/release/convergio");
    let hash = hash_file(&binary)?;
    let meta = std::fs::metadata(&binary)?;
    let size = meta.len() as i64;
    info!(hash = %hash, size, "release binary built");

    Ok((test_count, hash, size))
}

/// Parse total test count from cargo test output.
fn parse_test_count(output: &str) -> i64 {
    let mut total: i64 = 0;
    for line in output.lines() {
        // Lines like: "test result: ok. 59 passed; 0 failed; ..."
        if let Some(rest) = line.strip_prefix("test result:") {
            for part in rest.split(';') {
                let part = part.trim();
                if part.ends_with("passed") {
                    // Extract the number just before "passed"
                    let words: Vec<&str> = part.split_whitespace().collect();
                    if words.len() >= 2 {
                        if let Ok(n) = words[words.len() - 2].parse::<i64>() {
                            total += n;
                        }
                    }
                }
            }
        }
    }
    total
}

/// SHA-256 hash of a file.
fn hash_file(path: &Path) -> BuildResult<String> {
    let data = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = hasher.finalize();
    Ok(hash.iter().map(|b| format!("{b:02x}")).collect())
}

/// Get a build record by ID.
pub fn get_build(pool: &ConnPool, id: &str) -> BuildResult<BuildRecord> {
    let conn = pool.get().map_err(|e| BuildError::Pool(e.to_string()))?;
    conn.query_row(
        "SELECT id, status, commit_hash, test_count, binary_hash, binary_size, \
         started_at, completed_at, error, duration_secs FROM build_history WHERE id=?1",
        [id],
        |row| {
            Ok(BuildRecord {
                id: row.get(0)?,
                status: BuildStatus::parse_status(&row.get::<_, String>(1)?),
                commit_hash: row.get(2)?,
                test_count: row.get(3)?,
                binary_hash: row.get(4)?,
                binary_size: row.get(5)?,
                started_at: row.get(6)?,
                completed_at: row.get(7)?,
                error: row.get(8)?,
                duration_secs: row.get(9)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => BuildError::NotFound(format!("build {id}")),
        other => BuildError::Db(other),
    })
}

/// List recent builds.
pub fn list_builds(pool: &ConnPool, limit: i64) -> BuildResult<Vec<BuildRecord>> {
    let conn = pool.get().map_err(|e| BuildError::Pool(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, status, commit_hash, test_count, binary_hash, binary_size, \
         started_at, completed_at, error, duration_secs \
         FROM build_history ORDER BY started_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(BuildRecord {
            id: row.get(0)?,
            status: BuildStatus::parse_status(&row.get::<_, String>(1)?),
            commit_hash: row.get(2)?,
            test_count: row.get(3)?,
            binary_hash: row.get(4)?,
            binary_size: row.get(5)?,
            started_at: row.get(6)?,
            completed_at: row.get(7)?,
            error: row.get(8)?,
            duration_secs: row.get(9)?,
        })
    })?;
    Ok(rows
        .filter_map(|r| match r {
            Ok(rec) => Some(rec),
            Err(e) => {
                tracing::warn!(error = %e, "skipping malformed build_history row");
                None
            }
        })
        .collect())
}

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
