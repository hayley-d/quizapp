use serde_json::{json, Value};

mod common;

async fn create_module(app: &common::TestApp, name: &str) -> i64 {
    let (_, module) = app.post("/api/modules", json!({ "name": name })).await;
    module["id"].as_i64().unwrap()
}

async fn create_deck(app: &common::TestApp, name: &str, module_id: Option<i64>) -> i64 {
    let (_, deck) = app
        .post(
            "/api/decks",
            json!({ "name": name, "module_id": module_id, "description": "" }),
        )
        .await;
    deck["id"].as_i64().unwrap()
}

async fn create_flashcard(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "flashcard",
                "prompt_md": prompt,
                "answer_md": "an answer",
            }),
        )
        .await;
    card["id"].as_i64().unwrap()
}

fn field_errors(body: &Value) -> Vec<(String, String)> {
    body["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["field"].as_str().unwrap().to_string(),
                entry["message"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn has_field(body: &Value, field: &str) -> bool {
    field_errors(body).iter().any(|(name, _)| name == field)
}

#[tokio::test]
async fn creates_a_practice_session_from_deck_ids() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "what is k-means").await;
    create_flashcard(&app, deck_id, "what is dbscan").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 201, "body was {body}");
    assert_eq!(body["mode"], "practice");
    assert_eq!(body["deck_ids"], json!([deck_id]));
    assert_eq!(body["target_count"], Value::Null);
    assert_eq!(body["ended_at"], Value::Null);
    assert_eq!(body["pool_count"], 2);
    assert_eq!(body["answered_count"], 0);
    assert!(body["started_at"].as_str().unwrap().ends_with('Z'));
}

#[tokio::test]
async fn deck_ids_are_stored_sorted_and_deduplicated() {
    let app = common::spawn_app().await;
    let first = create_deck(&app, "alpha", None).await;
    let second = create_deck(&app, "beta", None).await;
    create_flashcard(&app, first, "a").await;
    create_flashcard(&app, second, "b").await;

    let (status, body) = app
        .post(
            "/api/sessions",
            json!({ "mode": "practice", "deck_ids": [second, first, second, first] }),
        )
        .await;

    assert_eq!(status, 201);
    assert_eq!(
        body["deck_ids"],
        json!([first, second]),
        "deck_ids must be canonical: sorted and deduplicated",
    );
    assert_eq!(body["pool_count"], 2, "a duplicated deck must not double the pool");
}

#[tokio::test]
async fn expands_a_module_into_its_decks() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;
    let first = create_deck(&app, "test one", Some(module_id)).await;
    let second = create_deck(&app, "test two", Some(module_id)).await;
    let unrelated = create_deck(&app, "unrelated", None).await;
    create_flashcard(&app, first, "a").await;
    create_flashcard(&app, second, "b").await;
    create_flashcard(&app, unrelated, "c").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "module_id": module_id }))
        .await;

    assert_eq!(status, 201, "body was {body}");
    assert_eq!(
        body["deck_ids"],
        json!([first, second]),
        "a module must expand to its own decks and no others",
    );
    assert_eq!(body["pool_count"], 2);
}

#[tokio::test]
async fn rejects_an_unknown_mode_value() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "a").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "cram", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 422);
    assert!(field_errors(&body)
        .contains(&("mode".to_string(), "mode must be practice, mock or sm2".to_string())));
}

#[tokio::test]
async fn rejects_mock_and_sm2_modes_for_now() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "a").await;

    for mode in ["mock", "sm2"] {
        let (status, body) = app
            .post("/api/sessions", json!({ "mode": mode, "deck_ids": [deck_id] }))
            .await;
        assert_eq!(status, 422, "{mode} should not be accepted yet");
        assert!(
            field_errors(&body).contains(&(
                "mode".to_string(),
                "Only practice mode is available yet".to_string()
            )),
            "{mode} must get the not-yet message, not the unknown-mode one: {body}",
        );
    }
}

#[tokio::test]
async fn rejects_both_deck_ids_and_module_id() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;
    let deck_id = create_deck(&app, "clustering", Some(module_id)).await;
    create_flashcard(&app, deck_id, "a").await;

    let (status, body) = app
        .post(
            "/api/sessions",
            json!({ "mode": "practice", "deck_ids": [deck_id], "module_id": module_id }),
        )
        .await;

    assert_eq!(status, 422);
    assert!(field_errors(&body).contains(&(
        "deck_ids".to_string(),
        "Choose either decks or a module, not both".to_string()
    )));
}

#[tokio::test]
async fn rejects_neither_deck_ids_nor_module_id() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/sessions", json!({ "mode": "practice" })).await;
    assert_eq!(status, 422);
    assert!(field_errors(&body).contains(&(
        "deck_ids".to_string(),
        "Choose at least one deck or a module".to_string()
    )));
}

#[tokio::test]
async fn rejects_an_empty_deck_id_array() {
    let app = common::spawn_app().await;
    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [] }))
        .await;
    assert_eq!(status, 422);
    assert!(field_errors(&body)
        .contains(&("deck_ids".to_string(), "Choose at least one deck".to_string())));
}

#[tokio::test]
async fn rejects_an_unknown_deck_id() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "a").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id, 9999] }))
        .await;

    assert_eq!(status, 422);
    assert!(has_field(&body, "deck_ids"));
    assert_eq!(app.count("SELECT COUNT(*) FROM sessions").await, 0);
}

#[tokio::test]
async fn rejects_an_unknown_module_id() {
    let app = common::spawn_app().await;
    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "module_id": 9999 }))
        .await;
    assert_eq!(status, 422);
    assert!(
        field_errors(&body)
            .contains(&("module_id".to_string(), "That module does not exist".to_string())),
        "a missing module must say so, not be mistaken for a module with no decks: {body}",
    );
    assert_eq!(app.count("SELECT COUNT(*) FROM sessions").await, 0);
}

#[tokio::test]
async fn rejects_a_module_with_no_decks() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "empty module").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "module_id": module_id }))
        .await;

    assert_eq!(status, 422);
    assert!(field_errors(&body)
        .contains(&("module_id".to_string(), "That module has no decks".to_string())));
}

#[tokio::test]
async fn rejects_a_target_count_on_a_practice_session() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "a").await;

    let (status, body) = app
        .post(
            "/api/sessions",
            json!({ "mode": "practice", "deck_ids": [deck_id], "target_count": 20 }),
        )
        .await;

    assert_eq!(status, 422, "a target count must be refused, not silently ignored");
    assert!(field_errors(&body).contains(&(
        "target_count".to_string(),
        "Practice sessions have no target count".to_string()
    )));
    assert_eq!(app.count("SELECT COUNT(*) FROM sessions").await, 0);
}

#[tokio::test]
async fn refuses_to_create_a_session_with_no_eligible_cards() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "empty deck", None).await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 422, "an empty deck must fail at creation, not produce an empty runner");
    assert!(field_errors(&body).contains(&(
        "deck_ids".to_string(),
        "Those decks have no cards to practise".to_string()
    )));
    assert_eq!(
        app.count("SELECT COUNT(*) FROM sessions").await,
        0,
        "a refused creation must write no sessions row",
    );
}

#[tokio::test]
async fn archived_cards_do_not_count_as_eligible() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "all archived", None).await;
    let card_id = create_flashcard(&app, deck_id, "a").await;
    app.post(&format!("/api/cards/{card_id}/archive"), json!({})).await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 422, "a deck whose only card is archived has nothing to practise");
    assert!(has_field(&body, "deck_ids"));
    assert_eq!(app.count("SELECT COUNT(*) FROM sessions").await, 0);
}

#[tokio::test]
async fn the_pool_count_excludes_archived_cards() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mixed", None).await;
    create_flashcard(&app, deck_id, "live one").await;
    create_flashcard(&app, deck_id, "live two").await;
    let archived = create_flashcard(&app, deck_id, "archived").await;
    app.post(&format!("/api/cards/{archived}/archive"), json!({})).await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 201);
    assert_eq!(body["pool_count"], 2, "the archived card must not be counted");
}

#[tokio::test]
async fn an_unknown_field_is_rejected() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "a").await;

    let (status, _) = app
        .post(
            "/api/sessions",
            json!({ "mode": "practice", "deck_ids": [deck_id], "shuffle": true }),
        )
        .await;

    assert_eq!(status, 422);
}
