// Shared test harness compiled fresh into every integration-test binary; a
// given binary only ever exercises a subset of TestApp's helpers, so clippy
// would otherwise flag the rest as dead_code in that binary.
#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

pub struct TestApp {
    pub router: Router,
    pub pool: sqlx::SqlitePool,
    _dir: tempfile::TempDir,
}

pub async fn spawn_app() -> TestApp {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = format!("sqlite://{}/test.db?mode=rwc", dir.path().display());
    let pool = quizapp::db::connect(&url).await.expect("db connect");
    let router = quizapp::app(quizapp::state::AppState { pool: pool.clone() });
    TestApp { router, pool, _dir: dir }
}

impl TestApp {
    pub async fn request(&self, method: &str, uri: &str, body: Option<Value>)
        -> (StatusCode, Value)
    {
        let req = Request::builder().method(method).uri(uri);
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&b).unwrap())),
            None => req.body(Body::empty()),
        }
        .unwrap();

        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, json)
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.request("GET", uri, None).await
    }
    pub async fn post(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.request("POST", uri, Some(body)).await
    }
    pub async fn patch(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.request("PATCH", uri, Some(body)).await
    }

    /// Scalar count, for asserting on tables the HTTP surface does not expose.
    pub async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }

    /// (row count, due_at) for a card's schedule row.
    pub async fn schedule_for(&self, card_id: i64) -> (i64, String) {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schedule WHERE card_id = ?")
            .bind(card_id).fetch_one(&self.pool).await.unwrap();
        let due: String = sqlx::query_scalar("SELECT due_at FROM schedule WHERE card_id = ?")
            .bind(card_id).fetch_one(&self.pool).await.unwrap();
        (n, due)
    }
}
