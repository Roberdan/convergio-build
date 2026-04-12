//! E2E tests for convergio-build HTTP route handlers.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use convergio_build::builder;
use convergio_build::routes::{router, BuildState};
use convergio_db::pool::ConnPool;
use tower::ServiceExt;

fn setup() -> (axum::Router, ConnPool) {
    let pool = convergio_db::pool::create_memory_pool().unwrap();
    let conn = pool.get().unwrap();
    for m in convergio_build::schema::migrations() {
        conn.execute_batch(m.up).unwrap();
    }
    drop(conn);
    let state = Arc::new(BuildState { pool: pool.clone() });
    (router(state), pool)
}

fn rebuild(pool: &ConnPool) -> axum::Router {
    let state = Arc::new(BuildState { pool: pool.clone() });
    router(state)
}

async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn get_req(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn post_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn route_trigger_build_returns_queued() {
    let (app, _) = setup();
    let resp = app.oneshot(post_req("/api/build/self")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["ok"], true);
    assert!(json["build_id"].is_string());
    assert!(json["commit"].is_string());
    assert_eq!(json["status"], "queued");
}

#[tokio::test]
async fn route_build_status_found() {
    let (_, pool) = setup();
    let id = builder::create_build(&pool, "abc1234").unwrap();

    let app = rebuild(&pool);
    let uri = format!("/api/build/status/{id}");
    let resp = app.oneshot(get_req(&uri)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["ok"], true);
    assert_eq!(json["build"]["status"], "queued");
    assert_eq!(json["build"]["commit_hash"], "abc1234");
}

#[tokio::test]
async fn route_build_status_not_found() {
    let (app, _) = setup();
    let resp = app
        .oneshot(get_req("/api/build/status/nonexistent"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["ok"], false);
    assert!(json["error"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn route_build_history_empty() {
    let (app, _) = setup();
    let resp = app.oneshot(get_req("/api/build/history")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["ok"], true);
    assert_eq!(json["builds"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn route_build_history_with_data() {
    let (_, pool) = setup();
    builder::create_build(&pool, "c1").unwrap();
    builder::create_build(&pool, "c2").unwrap();

    let app = rebuild(&pool);
    let resp = app.oneshot(get_req("/api/build/history")).await.unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["ok"], true);
    assert_eq!(json["builds"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn route_build_history_limit() {
    let (_, pool) = setup();
    for i in 0..5 {
        builder::create_build(&pool, &format!("c{i}")).unwrap();
    }

    let app = rebuild(&pool);
    let resp = app
        .oneshot(get_req("/api/build/history?limit=2"))
        .await
        .unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["builds"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn route_deploy_rejects_non_succeeded() {
    let (_, pool) = setup();
    let id = builder::create_build(&pool, "abc").unwrap();

    let app = rebuild(&pool);
    let uri = format!("/api/build/deploy/{id}");
    let resp = app.oneshot(post_req(&uri)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["ok"], false);
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("must be 'succeeded'"));
}

#[tokio::test]
async fn route_deploy_not_found() {
    let (app, _) = setup();
    let resp = app
        .oneshot(post_req("/api/build/deploy/nonexistent"))
        .await
        .unwrap();
    let json = body_json(resp).await;
    assert_eq!(json["ok"], false);
    assert!(json["error"].as_str().unwrap().contains("not found"));
}
