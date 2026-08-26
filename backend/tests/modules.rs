mod common;

use axum::http::StatusCode;
use serde_json::json;

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
