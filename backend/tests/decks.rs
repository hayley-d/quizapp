mod common;

use axum::http::StatusCode;
use serde_json::json;

async fn create_module(app: &common::TestApp, name: &str) -> i64 {
    let (_, created) = app.post("/api/modules", json!({"name": name})).await;
    created["id"].as_i64().unwrap()
}

async fn create_deck_named(app: &common::TestApp, name: &str) -> i64 {
    let (_, created) = app.post("/api/decks", json!({ "name": name })).await;
    created["id"].as_i64().unwrap()
}

async fn create_flashcard(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (status, created) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard",
            "prompt_md": prompt, "answer_md": "an answer",
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "helper card create failed: {created}");
    created["id"].as_i64().unwrap()
}

async fn record_review(app: &common::TestApp, deck_id: i64, card_id: i64) -> i64 {
    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (mode, deck_ids) VALUES ('practice', ?) RETURNING id",
    )
    .bind(format!("[{deck_id}]"))
    .fetch_one(&app.pool)
    .await
    .unwrap();

    sqlx::query_scalar(
        "INSERT INTO reviews (card_id, session_id, correct) VALUES (?, ?, 1) RETURNING id",
    )
    .bind(card_id)
    .bind(session_id)
    .fetch_one(&app.pool)
    .await
    .unwrap()
}

fn names_of(list: &serde_json::Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|deck| deck["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn create_deck_in_module() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;

    let (status, body) = app
        .post("/api/decks", json!({
            "module_id": module_id, "name": "Test 1", "description": "Ch 1-3"
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "Test 1");
    assert_eq!(body["module_id"], module_id);
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
    let module_id = create_module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": module_id, "name": "Test 1"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, all) = app.get("/api/decks").await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    let (_, in_module) = app.get(&format!("/api/decks?module_id={module_id}")).await;
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

    let (status, list) = app.get("/api/decks?search=kine").await;
    assert_eq!(status, StatusCode::OK);
    let names = names_of(&list);
    assert_eq!(names, vec!["Chapter 1 - Kinematics"]);

    let (_, all) = app.get("/api/decks?search=").await;
    assert_eq!(all.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn search_does_not_match_description() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "Deck One", "description": "clustering"})).await;

    let (_, list) = app.get("/api/decks?search=clustering").await;
    assert_eq!(list.as_array().unwrap().len(), 0, "q must match name only");
}

#[tokio::test]
async fn search_treats_wildcards_literally() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "Scored 100% overall"})).await;
    app.post("/api/decks", json!({"name": "Unrelated"})).await;

    let (_, pct) = app.get("/api/decks?search=100%25").await;
    assert_eq!(names_of(&pct), vec!["Scored 100% overall"]);

    let (_, underscore) = app.get("/api/decks?search=_nrelated").await;
    assert_eq!(underscore.as_array().unwrap().len(), 0, "_ must be literal");
}

#[tokio::test]
async fn sort_newest_is_default_and_oldest_reverses_it() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "First"})).await;
    app.post("/api/decks", json!({"name": "Second"})).await;
    app.post("/api/decks", json!({"name": "Third"})).await;

    let (_, default) = app.get("/api/decks").await;
    assert_eq!(names_of(&default), vec!["Third", "Second", "First"]);

    let (_, newest) = app.get("/api/decks?sort=newest").await;
    assert_eq!(names_of(&newest), names_of(&default), "absent sort == newest");

    let (_, oldest) = app.get("/api/decks?sort=oldest").await;
    assert_eq!(names_of(&oldest), vec!["First", "Second", "Third"]);

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
    let module_id = create_module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": module_id, "name": "In module"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, absent) = app.get("/api/decks").await;
    let (_, all) = app.get("/api/decks?module_id=all").await;
    assert_eq!(names_of(&absent), names_of(&all));
    assert_eq!(all.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn criteria_combine() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": module_id, "name": "Alpha test"})).await;
    app.post("/api/decks", json!({"module_id": module_id, "name": "Beta test"})).await;
    app.post("/api/decks", json!({"name": "Alpha loose"})).await;

    let (_, list) = app
        .get(&format!("/api/decks?search=alpha&module_id={module_id}&sort=oldest"))
        .await;
    assert_eq!(names_of(&list), vec!["Alpha test"]);
}

#[tokio::test]
async fn patch_renames_reparents_and_unparents() {
    let app = common::spawn_app().await;
    let first_module_id = create_module(&app, "COS781").await;
    let second_module_id = create_module(&app, "COS731").await;
    let (_, deck) = app
        .post("/api/decks", json!({"module_id": first_module_id, "name": "Test 1"}))
        .await;
    let id = deck["id"].as_i64().unwrap();

    let (status, renamed) = app
        .patch(&format!("/api/decks/{id}"), json!({"name": "Test One"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["name"], "Test One");
    assert_eq!(renamed["module_id"], first_module_id, "module must be untouched");

    let (_, moved) = app
        .patch(&format!("/api/decks/{id}"), json!({"module_id": second_module_id}))
        .await;
    assert_eq!(moved["module_id"], second_module_id);
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
async fn get_by_id_returns_the_same_shape_as_the_list() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;
    let (_, created) = app
        .post("/api/decks", json!({
            "module_id": module_id, "name": "Test 1", "description": "Ch 1-3"
        }))
        .await;
    let id = created["id"].as_i64().unwrap();

    let (status, fetched) = app.get(&format!("/api/decks/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["id"], id);
    assert_eq!(fetched["name"], "Test 1");
    assert_eq!(fetched["module_id"], module_id);
    assert_eq!(fetched["module_name"], "COS781");
    assert_eq!(fetched["description"], "Ch 1-3");
    assert_eq!(fetched["card_count"], 0);

    let (_, list) = app.get("/api/decks").await;
    let from_list = list.as_array().unwrap().iter().find(|deck| deck["id"] == id).unwrap();
    assert_eq!(fetched, *from_list, "GET by id must match the list's shape exactly");
}

#[tokio::test]
async fn get_unknown_deck_is_404() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/decks/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn duplicate_name_in_module_conflicts() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": module_id, "name": "Test 1"})).await;
    let (status, body) = app
        .post("/api/decks", json!({"module_id": module_id, "name": "Test 1"}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
    assert_eq!(body["fields"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn module_deck_count_reflects_only_its_own_decks() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": module_id, "name": "Test 1"})).await;
    app.post("/api/decks", json!({"module_id": module_id, "name": "Test 2"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, modules) = app.get("/api/modules").await;
    let module = modules
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["id"].as_i64() == Some(module_id))
        .expect("module present");
    assert_eq!(module["deck_count"], 2);
}

#[tokio::test]
async fn deleting_a_deck_removes_its_cards_and_their_reviews() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;
    let (_, doomed) = app
        .post("/api/decks", json!({ "module_id": module_id, "name": "Test 1" }))
        .await;
    let doomed_deck_id = doomed["id"].as_i64().unwrap();
    let (_, sibling) = app
        .post("/api/decks", json!({ "module_id": module_id, "name": "Test 2" }))
        .await;
    let sibling_deck_id = sibling["id"].as_i64().unwrap();

    let card_id = create_flashcard(&app, doomed_deck_id, "what is k-means").await;
    let review_id = record_review(&app, doomed_deck_id, card_id).await;

    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM reviews WHERE id = {review_id}")).await,
        1,
        "the review must exist before the delete",
    );

    let (status, body) = app.delete(&format!("/api/decks/{doomed_deck_id}")).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "body was {body}");

    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM decks WHERE id = {doomed_deck_id}")).await,
        0,
    );
    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM cards WHERE id = {card_id}")).await,
        0,
        "the deck's cards must go with it",
    );
    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM reviews WHERE id = {review_id}")).await,
        0,
        "the cards' reviews must go with them",
    );
    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM decks WHERE id = {sibling_deck_id}")).await,
        1,
        "the other deck in the module must be untouched",
    );
}

#[tokio::test]
async fn deleting_an_unknown_deck_is_404() {
    let app = common::spawn_app().await;
    let (status, body) = app.delete("/api/decks/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
    assert_eq!(body["message"], "deck not found");
}

#[tokio::test]
async fn deletion_impact_counts_archived_cards_and_their_reviews() {
    let app = common::spawn_app().await;
    let deck_id = create_deck_named(&app, "Test 1").await;
    let visible_card_id = create_flashcard(&app, deck_id, "visible").await;
    let archived_card_id = create_flashcard(&app, deck_id, "archived").await;

    let (archive_status, archived) = app
        .post(&format!("/api/cards/{archived_card_id}/archive"), json!({}))
        .await;
    assert_eq!(archive_status, StatusCode::OK, "setup failed: {archived}");
    assert_eq!(archived["archived"], true);

    record_review(&app, deck_id, visible_card_id).await;
    record_review(&app, deck_id, archived_card_id).await;

    let (status, impact) = app
        .get(&format!("/api/decks/{deck_id}/deletion-impact"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(impact["card_count"], 2, "the archived card must be counted");
    assert_eq!(
        impact["review_count"], 2,
        "the archived card's review must be counted",
    );

    let (_, deck) = app.get(&format!("/api/decks/{deck_id}")).await;
    assert_eq!(
        deck["card_count"], 1,
        "the deck's own card_count still excludes archived cards, so the two figures \
         must differ — if they match, deletion-impact copied the archived filter",
    );
}

#[tokio::test]
async fn deletion_impact_for_an_unknown_deck_is_404() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/decks/9999/deletion-impact").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
    assert_eq!(body["message"], "deck not found");
}
