use super::*;

#[test]
fn parse_test_count_from_output() {
    let output = "test result: ok. 59 passed; 0 failed; 0 ignored;\n\
                   test result: ok. 12 passed; 0 failed; 0 ignored;";
    assert_eq!(parse_test_count(output), 71);
}

#[test]
fn parse_test_count_empty() {
    assert_eq!(parse_test_count("no test output"), 0);
}

#[test]
fn current_commit_returns_something() {
    let commit = current_commit();
    // Returns a git hash or "unknown" if not in a git repo
    assert!(!commit.is_empty());
    // When there's no git context, fallback should be "unknown"
    // When there is git context, it should be a short hash
    assert!(commit == "unknown" || commit.len() >= 4);
}

#[test]
fn create_and_get_build() {
    let pool = convergio_db::pool::create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    for m in crate::schema::migrations() {
        conn.execute_batch(m.up).unwrap();
    }
    drop(conn);

    let id = create_build(&pool, "abc123").unwrap();
    let rec = get_build(&pool, &id).unwrap();
    assert_eq!(rec.commit_hash, "abc123");
    assert_eq!(rec.status, BuildStatus::Queued);
}

#[test]
fn list_builds_empty() {
    let pool = convergio_db::pool::create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    for m in crate::schema::migrations() {
        conn.execute_batch(m.up).unwrap();
    }
    drop(conn);

    let builds = list_builds(&pool, 10).unwrap();
    assert!(builds.is_empty());
}

#[test]
fn update_and_complete_build() {
    let pool = convergio_db::pool::create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    for m in crate::schema::migrations() {
        conn.execute_batch(m.up).unwrap();
    }
    drop(conn);

    let id = create_build(&pool, "def456").unwrap();
    update_status(&pool, &id, BuildStatus::Testing).unwrap();

    let rec = get_build(&pool, &id).unwrap();
    assert_eq!(rec.status, BuildStatus::Testing);

    let completed = BuildRecord {
        id: id.clone(),
        status: BuildStatus::Succeeded,
        commit_hash: "def456".into(),
        test_count: Some(971),
        binary_hash: Some("sha256abc".into()),
        binary_size: Some(50_000_000),
        started_at: rec.started_at,
        completed_at: None,
        error: None,
        duration_secs: Some(120.5),
    };
    complete_build(&pool, &id, &completed).unwrap();

    let rec = get_build(&pool, &id).unwrap();
    assert_eq!(rec.status, BuildStatus::Succeeded);
    assert_eq!(rec.test_count, Some(971));
}
