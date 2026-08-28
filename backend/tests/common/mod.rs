#![allow(dead_code)]

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

pub struct TestApp {
    pub router: Router,
    pub pool: sqlx::SqlitePool,
    pub images_directory: PathBuf,
    _temporary_directory: tempfile::TempDir,
}

pub async fn spawn_app() -> TestApp {
    let temporary_directory = tempfile::tempdir().expect("tempdir");
    let database_url = format!(
        "sqlite://{}/test.db?mode=rwc",
        temporary_directory.path().display(),
    );
    let pool = quizapp::database::connect(&database_url).await.expect("database connect");
    let images_directory = temporary_directory.path().join("images");
    std::fs::create_dir_all(&images_directory).expect("images directory");
    let router = quizapp::app(quizapp::state::AppState {
        pool: pool.clone(),
        images_directory: images_directory.clone(),
    });
    TestApp { router, pool, images_directory, _temporary_directory: temporary_directory }
}

impl TestApp {
    pub async fn request(&self, method: &str, uri: &str, body: Option<Value>)
        -> (StatusCode, Value)
    {
        let builder = Request::builder().method(method).uri(uri);
        let request = match body {
            Some(body_value) => builder
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body_value).unwrap())),
            None => builder.body(Body::empty()),
        }
        .unwrap();

        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
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

    pub async fn get_raw(&self, uri: &str) -> (StatusCode, Vec<u8>) {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes().to_vec();
        (status, bytes)
    }

    pub async fn post_file(
        &self, uri: &str, field: &str, filename: &str, bytes: &[u8],
    ) -> (StatusCode, Value) {
        const BOUNDARY: &str = "XTESTBOUNDARYX";

        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
            .body(Body::from(body))
            .unwrap();

        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }

    pub async fn image_count(&self) -> usize {
        std::fs::read_dir(&self.images_directory).expect("read images directory").count()
    }

    pub async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }

    pub async fn schedule_for(&self, card_id: i64) -> (i64, String) {
        let row_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM schedule WHERE card_id = ?")
                .bind(card_id).fetch_one(&self.pool).await.unwrap();
        let due_at: String =
            sqlx::query_scalar("SELECT due_at FROM schedule WHERE card_id = ?")
                .bind(card_id).fetch_one(&self.pool).await.unwrap();
        (row_count, due_at)
    }

    pub async fn schedule_state_for(&self, card_id: i64) -> (String, f64, f64, i64, i64) {
        sqlx::query_as(
            "SELECT due_at, interval_days, ease, reps, lapses FROM schedule WHERE card_id = ?",
        )
        .bind(card_id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    pub async fn answered_at_for_review(&self, review_id: i64) -> String {
        sqlx::query_scalar("SELECT answered_at FROM reviews WHERE id = ?")
            .bind(review_id)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    pub async fn date_advanced_by_days(&self, date_time: &str, days: i64) -> String {
        let advanced_date: String =
            sqlx::query_scalar("SELECT date(?, '+' || CAST(? AS TEXT) || ' days')")
                .bind(date_time)
                .bind(days)
                .fetch_one(&self.pool)
                .await
                .unwrap();
        format!("{advanced_date}T00:00:00Z")
    }
}
