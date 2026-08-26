mod common;

use axum::http::StatusCode;
use serde_json::json;

async fn module(app: &common::TestApp, name: &str) -> i64 {
    let (_, m) = app.post("/api/modules", json!({"name": name})).await;
    m["id"].as_i64().unwrap()
}

fn names_of(list: &serde_json::Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect()
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
async fn search_matches_name_case_insensitively() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "Chapter 1 - Kinematics"})).await;
    app.post("/api/decks", json!({"name": "Thermodynamics"})).await;

    let (status, list) = app.get("/api/decks?q=kine").await;
    assert_eq!(status, StatusCode::OK);
    let names = names_of(&list);
    assert_eq!(names, vec!["Chapter 1 - Kinematics"]);

    // Empty q applies no filter.
    let (_, all) = app.get("/api/decks?q=").await;
    assert_eq!(all.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn search_does_not_match_description() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "Deck One", "description": "clustering"})).await;

    let (_, list) = app.get("/api/decks?q=clustering").await;
    assert_eq!(list.as_array().unwrap().len(), 0, "q must match name only");
}

#[tokio::test]
async fn search_treats_wildcards_literally() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "Scored 100% overall"})).await;
    app.post("/api/decks", json!({"name": "Unrelated"})).await;

    let (_, pct) = app.get("/api/decks?q=100%25").await; // %25 is an encoded '%'
    assert_eq!(names_of(&pct), vec!["Scored 100% overall"]);

    // A bare '%' must not behave as "match everything".
    let (_, underscore) = app.get("/api/decks?q=_nrelated").await;
    assert_eq!(underscore.as_array().unwrap().len(), 0, "_ must be literal");
}

#[tokio::test]
async fn sort_newest_is_default_and_oldest_reverses_it() {
    let app = common::spawn_app().await;
    // Same-second creation is the normal case here, so this also exercises the id tiebreak.
    app.post("/api/decks", json!({"name": "First"})).await;
    app.post("/api/decks", json!({"name": "Second"})).await;
    app.post("/api/decks", json!({"name": "Third"})).await;

    let (_, default) = app.get("/api/decks").await;
    assert_eq!(names_of(&default), vec!["Third", "Second", "First"]);

    let (_, newest) = app.get("/api/decks?sort=newest").await;
    assert_eq!(names_of(&newest), names_of(&default), "absent sort == newest");

    let (_, oldest) = app.get("/api/decks?sort=oldest").await;
    assert_eq!(names_of(&oldest), vec!["First", "Second", "Third"]);

    // Pin the actual contract ("oldest is the exact reverse of newest") rather than
    // relying on two independently-maintained literal lists that could drift apart.
    let newest_names = names_of(&newest);
    let mut reversed_newest = newest_names.clone();
    reversed_newest.reverse();
    assert_eq!(
        names_of(&oldest),
        reversed_newest,
        "oldest must be the exact reverse of newest"
    );
}

#[tokio::test]
async fn unknown_sort_is_422_with_field_error() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/decks?sort=sideways").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"][0]["field"], "sort");
}

#[tokio::test]
async fn module_all_equals_absent() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "In module"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, absent) = app.get("/api/decks").await;
    let (_, all) = app.get("/api/decks?module_id=all").await;
    assert_eq!(names_of(&absent), names_of(&all));
    assert_eq!(all.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn criteria_combine() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Alpha test"})).await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Beta test"})).await;
    app.post("/api/decks", json!({"name": "Alpha loose"})).await;

    let (_, list) = app
        .get(&format!("/api/decks?q=alpha&module_id={mid}&sort=oldest"))
        .await;
    assert_eq!(names_of(&list), vec!["Alpha test"]);
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
