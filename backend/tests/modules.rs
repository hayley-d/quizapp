mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn list_starts_empty() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/modules").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn create_then_list() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/modules", json!({"name": "COS781"})).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "COS781");
    assert_eq!(body["deck_count"], 0);

    let (_, list) = app.get("/api/modules").await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn name_is_trimmed_and_required() {
    let app = common::spawn_app().await;

    let (status, body) = app.post("/api/modules", json!({"name": "   "})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "name");

    let (_, created) = app.post("/api/modules", json!({"name": "  COS781  "})).await;
    assert_eq!(created["name"], "COS781");
}

#[tokio::test]
async fn duplicate_name_conflicts() {
    let app = common::spawn_app().await;
    app.post("/api/modules", json!({"name": "COS781"})).await;
    let (status, body) = app.post("/api/modules", json!({"name": "COS781"})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
    assert_eq!(body["fields"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn created_at_is_iso8601_utc() {
    let app = common::spawn_app().await;
    let (_, created) = app.post("/api/modules", json!({"name": "COS781"})).await;
    let created_at = created["created_at"].as_str().unwrap().to_string();
    assert!(
        created_at.ends_with('Z'),
        "expected a trailing Z marker, got {created_at}"
    );
    // `%Y-%m-%dT%H:%M:%SZ`: 19 digit/separator chars plus the trailing Z.
    let bytes = created_at.as_bytes();
    assert_eq!(bytes.len(), 20, "{created_at} is not 20 chars long");
    let digit_positions = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    for &i in &digit_positions {
        assert!(bytes[i].is_ascii_digit(), "{created_at}[{i}] should be a digit");
    }
    assert_eq!(bytes[4], b'-');
    assert_eq!(bytes[7], b'-');
    assert_eq!(bytes[10], b'T');
    assert_eq!(bytes[13], b':');
    assert_eq!(bytes[16], b':');
    assert_eq!(bytes[19], b'Z');
}

#[tokio::test]
async fn missing_field_returns_json_envelope() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/modules", json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"][0]["field"], "name");
}

#[tokio::test]
async fn malformed_json_syntax_returns_json_envelope() {
    let app = common::spawn_app().await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/modules")
        .header("content-type", "application/json")
        .body(Body::from("{"))
        .unwrap();
    let res = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let content_type = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    assert!(content_type.starts_with("application/json"));
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"], "bad_request");
}

#[tokio::test]
async fn missing_content_type_returns_json_envelope() {
    let app = common::spawn_app().await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/modules")
        .body(Body::from(r#"{"name":"COS781"}"#))
        .unwrap();
    let res = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let content_type = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_string();
    assert!(content_type.starts_with("application/json"));
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"], "unsupported_media_type");
}

#[tokio::test]
async fn wrong_type_returns_json_envelope_with_field() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/modules", json!({"name": 5})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"][0]["field"], "name");
}

#[tokio::test]
async fn unknown_field_is_rejected() {
    let app = common::spawn_app().await;
    let (status, body) = app
        .post("/api/modules", json!({"name": "COS781", "bogus": true}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
}

#[tokio::test]
async fn list_is_ordered_by_name_case_insensitively() {
    let app = common::spawn_app().await;
    app.post("/api/modules", json!({"name": "Banana"})).await;
    app.post("/api/modules", json!({"name": "apple"})).await;

    let (_, list) = app.get("/api/modules").await;
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["apple", "Banana"]);
}
