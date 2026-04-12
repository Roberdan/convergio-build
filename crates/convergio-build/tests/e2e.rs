//! E2E tests for convergio-build builder CRUD operations.

use convergio_build::builder;
use convergio_build::types::{BuildRecord, BuildStatus};
use convergio_db::pool::ConnPool;

fn setup_pool() -> ConnPool {
    let pool = convergio_db::pool::create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    for m in convergio_build::schema::migrations() {
        conn.execute_batch(m.up).unwrap();
    }
    drop(conn);
    pool
}

#[tokio::test]
async fn create_build_inserts_record() {
    let pool = setup_pool();
    let id = builder::create_build(&pool, "abc1234").unwrap();
    assert!(!id.is_empty());

    let rec = builder::get_build(&pool, &id).unwrap();
    assert_eq!(rec.commit_hash, "abc1234");
    assert_eq!(rec.status, BuildStatus::Queued);
}

#[tokio::test]
async fn update_status_changes_record() {
    let pool = setup_pool();
    let id = builder::create_build(&pool, "abc1234").unwrap();
    builder::update_status(&pool, &id, BuildStatus::Building).unwrap();

    let rec = builder::get_build(&pool, &id).unwrap();
    assert_eq!(rec.status, BuildStatus::Building);
}

#[tokio::test]
async fn complete_build_marks_succeeded() {
    let pool = setup_pool();
    let id = builder::create_build(&pool, "def5678").unwrap();
    let record = BuildRecord {
        id: id.clone(),
        status: BuildStatus::Succeeded,
        commit_hash: "def5678".into(),
        test_count: Some(42),
        binary_hash: Some("deadbeef".into()),
        binary_size: Some(1024),
        started_at: String::new(),
        completed_at: None,
        error: None,
        duration_secs: Some(12.5),
    };
    builder::complete_build(&pool, &id, &record).unwrap();

    let rec = builder::get_build(&pool, &id).unwrap();
    assert_eq!(rec.status, BuildStatus::Succeeded);
    assert_eq!(rec.test_count, Some(42));
    assert_eq!(rec.binary_hash.as_deref(), Some("deadbeef"));
    assert_eq!(rec.binary_size, Some(1024));
    assert!(rec.duration_secs.unwrap() > 0.0);
}

#[tokio::test]
async fn complete_build_marks_failed() {
    let pool = setup_pool();
    let id = builder::create_build(&pool, "fail999").unwrap();
    let record = BuildRecord {
        id: id.clone(),
        status: BuildStatus::Failed,
        commit_hash: "fail999".into(),
        test_count: None,
        binary_hash: None,
        binary_size: None,
        started_at: String::new(),
        completed_at: None,
        error: Some("cargo check failed".into()),
        duration_secs: Some(3.0),
    };
    builder::complete_build(&pool, &id, &record).unwrap();

    let rec = builder::get_build(&pool, &id).unwrap();
    assert_eq!(rec.status, BuildStatus::Failed);
    assert_eq!(rec.error.as_deref(), Some("cargo check failed"));
}

#[tokio::test]
async fn get_build_not_found() {
    let pool = setup_pool();
    let err = builder::get_build(&pool, "nonexistent").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn list_builds_empty() {
    let pool = setup_pool();
    let builds = builder::list_builds(&pool, 20).unwrap();
    assert!(builds.is_empty());
}

#[tokio::test]
async fn list_builds_respects_limit() {
    let pool = setup_pool();
    for i in 0..5 {
        builder::create_build(&pool, &format!("commit_{i}")).unwrap();
    }
    let all = builder::list_builds(&pool, 100).unwrap();
    assert_eq!(all.len(), 5);

    let limited = builder::list_builds(&pool, 3).unwrap();
    assert_eq!(limited.len(), 3);
}
