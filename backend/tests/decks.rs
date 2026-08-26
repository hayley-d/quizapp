mod common;

use axum::http::StatusCode;
use serde_json::json;

async fn module(app: &common::TestApp, name: &str) -> i64 {
    let (_, m) = app.post("/api/modules", json!({"name": name})).await;
    m["id"].as_i64().unwrap()
}

#[tokio::test]
async fn create_deck_in_module() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;

    let (status, body) = app
        .post("/api/decks", json!({
            "module_id": mid, "name": "Test 1", "description": "Ch 1-3"
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "Test 1");
    assert_eq!(body["module_id"], mid);
    assert_eq!(body["module_name"], "COS781");
    assert_eq!(body["card_count"], 0);
}

#[tokio::test]
async fn create_deck_without_module() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/decks", json!({"name": "Loose"})).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(body["module_id"].is_null());
    assert!(body["module_name"].is_null());
    assert_eq!(body["description"], "");
}

#[tokio::test]
async fn unknown_module_is_rejected() {
    let app = common::spawn_app().await;
    let (status, body) = app
        .post("/api/decks", json!({"module_id": 9999, "name": "X"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "module_id");
}

#[tokio::test]
async fn empty_name_is_rejected() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/decks", json!({"name": "  "})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "name");
}

#[tokio::test]
async fn name_is_trimmed_on_create_and_patch() {
    let app = common::spawn_app().await;
    let (status, created) = app.post("/api/decks", json!({"name": "  Padded  "})).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["name"], "Padded");

    let id = created["id"].as_i64().unwrap();
    let (status, patched) = app
        .patch(&format!("/api/decks/{id}"), json!({"name": "  Patched  "}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patched["name"], "Patched");
}

#[tokio::test]
async fn description_is_trimmed_on_create_and_patch() {
    let app = common::spawn_app().await;
    let (status, created) = app
        .post("/api/decks", json!({"name": "Test 1", "description": "  padded  "}))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["description"], "padded");

    let id = created["id"].as_i64().unwrap();
    let (status, patched) = app
        .patch(&format!("/api/decks/{id}"), json!({"description": "  patched  "}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patched["description"], "patched");
}

#[tokio::test]
async fn unknown_field_on_create_is_rejected() {
    let app = common::spawn_app().await;
    let (status, body) = app
        .post("/api/decks", json!({"name": "T2", "module_id": null, "bogus": true}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
}

#[tokio::test]
async fn unknown_field_on_patch_is_rejected() {
    let app = common::spawn_app().await;
    let (_, created) = app.post("/api/decks", json!({"name": "T1"})).await;
    let id = created["id"].as_i64().unwrap();
    let (status, body) = app
        .patch(&format!("/api/decks/{id}"), json!({"moduleId": 999}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
}

#[tokio::test]
async fn unparseable_module_id_filter_is_422() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/decks?module_id=abc").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "module_id");
}

#[tokio::test]
async fn filter_by_module_and_by_none() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Test 1"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, all) = app.get("/api/decks").await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    let (_, in_module) = app.get(&format!("/api/decks?module_id={mid}")).await;
    assert_eq!(in_module.as_array().unwrap().len(), 1);
    assert_eq!(in_module[0]["name"], "Test 1");

    let (_, unparented) = app.get("/api/decks?module_id=none").await;
    assert_eq!(unparented.as_array().unwrap().len(), 1);
    assert_eq!(unparented[0]["name"], "Loose");
}

#[tokio::test]
async fn unfiltered_list_orders_by_module_then_deck_name_case_insensitively() {
    let app = common::spawn_app().await;
    // Module key: BINARY collation would sort "Banana" before "apple"
    // (uppercase < all lowercase); NOCASE sorts "apple" before "Banana".
    //
    // Deck-name key: within "apple", BINARY sorts "Zulu" before "zebra"
    // ('Z'=0x5A < 'z'=0x7A); NOCASE sorts "zebra" before "Zulu". "Deck A"
    // vs "Deck B" would NOT discriminate the second COLLATE NOCASE (both
    // collations agree on them), so the second module needs its own pair
    // that disagrees too.
    let banana = module(&app, "Banana").await;
    let apple = module(&app, "apple").await;
    app.post("/api/decks", json!({"module_id": banana, "name": "Deck B"})).await;
    app.post("/api/decks", json!({"module_id": apple, "name": "Zulu"})).await;
    app.post("/api/decks", json!({"module_id": apple, "name": "zebra"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, all) = app.get("/api/decks").await;
    let names: Vec<&str> = all
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    // NULL module names sort first in SQLite; then modules NOCASE-ordered
    // (apple, Banana); within "apple", decks NOCASE-ordered (zebra, Zulu).
    assert_eq!(names, vec!["Loose", "zebra", "Zulu", "Deck B"]);
}

#[tokio::test]
async fn patch_renames_reparents_and_unparents() {
    let app = common::spawn_app().await;
    let a = module(&app, "COS781").await;
    let b = module(&app, "COS731").await;
    let (_, deck) = app.post("/api/decks", json!({"module_id": a, "name": "Test 1"})).await;
    let id = deck["id"].as_i64().unwrap();

    let (status, renamed) = app
        .patch(&format!("/api/decks/{id}"), json!({"name": "Test One"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["name"], "Test One");
    assert_eq!(renamed["module_id"], a, "module must be untouched");

    let (_, moved) = app
        .patch(&format!("/api/decks/{id}"), json!({"module_id": b}))
        .await;
    assert_eq!(moved["module_id"], b);
    assert_eq!(moved["name"], "Test One", "name must be untouched");

    let (_, loose) = app
        .patch(&format!("/api/decks/{id}"), json!({"module_id": null}))
        .await;
    assert!(loose["module_id"].is_null());

    let (_, described) = app
        .patch(&format!("/api/decks/{id}"), json!({"description": "Ch 4-6"}))
        .await;
    assert_eq!(described["description"], "Ch 4-6");
}

#[tokio::test]
async fn patch_unknown_deck_is_404() {
    let app = common::spawn_app().await;
    let (status, _) = app.patch("/api/decks/9999", json!({"name": "X"})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn duplicate_name_in_module_conflicts() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Test 1"})).await;
    let (status, body) = app
        .post("/api/decks", json!({"module_id": mid, "name": "Test 1"}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
    assert_eq!(body["fields"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn module_deck_count_reflects_only_its_own_decks() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Test 1"})).await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Test 2"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, modules) = app.get("/api/modules").await;
    let m = modules
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"].as_i64() == Some(mid))
        .expect("module present");
    assert_eq!(m["deck_count"], 2);
}
