#![allow(dead_code)]

use serde_json::json;

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

async fn create_flashcard(app: &common::TestApp, deck_id: i64, prompt: &str, answer: &str) -> i64 {
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "flashcard",
                "prompt_md": prompt,
                "answer_md": answer,
            }),
        )
        .await;
    card["id"].as_i64().unwrap()
}

async fn create_multiple_choice(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "mc_single",
                "prompt_md": prompt,
                "choices": [
                    { "text_md": "single linkage", "is_correct": false },
                    { "text_md": "complete linkage", "is_correct": true },
                    { "text_md": "average linkage", "is_correct": false },
                    { "text_md": "ward linkage", "is_correct": false },
                ],
            }),
        )
        .await;
    card["id"].as_i64().unwrap()
}

async fn create_short_answer(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "short_answer",
                "prompt_md": prompt,
                "accepted": [
                    { "text": "k-means", "is_primary": true },
                    { "text": "lloyd's algorithm", "is_primary": false },
                ],
            }),
        )
        .await;
    card["id"].as_i64().unwrap()
}

async fn start_sm2_session(app: &common::TestApp, deck_id: i64) -> i64 {
    let (status, session) = app
        .post("/api/sessions", json!({ "mode": "sm2", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(status, 201, "could not start an sm2 session: {session}");
    session["id"].as_i64().unwrap()
}

async fn set_due_at(app: &common::TestApp, card_id: i64, due_at: &str) {
    sqlx::query("UPDATE schedule SET due_at = ? WHERE card_id = ?")
        .bind(due_at)
        .bind(card_id)
        .execute(&app.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_fresh_deck_is_entirely_due() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "answer one").await;
    create_flashcard(&app, deck_id, "two", "answer two").await;

    let (status, session) = app
        .post("/api/sessions", json!({ "mode": "sm2", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 201, "{session}");
    assert_eq!(session["target_count"], 2);
}

#[tokio::test]
async fn a_deck_with_nothing_due_is_refused_with_the_next_due_date() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "answer one").await;
    set_due_at(&app, card_id, "2099-03-04T00:00:00Z").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "sm2", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 422, "{body}");
    let message = body["fields"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("2099-03-04"),
        "the refusal must name the next due date: {message}",
    );
}

#[tokio::test]
async fn an_empty_deck_still_gets_the_empty_deck_refusal() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "sm2", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 422, "{body}");
    assert_eq!(body["fields"][0]["message"], "Those decks have no cards to practise");
}

#[tokio::test]
async fn archived_cards_are_never_due() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "live", "answer").await;
    let archived_card = create_flashcard(&app, deck_id, "archived", "answer").await;
    app.post(&format!("/api/cards/{archived_card}/archive"), json!({})).await;

    let (_, created) = app
        .post("/api/sessions", json!({ "mode": "sm2", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(created["target_count"], 1, "the archived card must not count as due");
}

#[tokio::test]
async fn a_client_supplied_target_count_is_refused() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "answer").await;

    let (status, body) = app
        .post(
            "/api/sessions",
            json!({ "mode": "sm2", "deck_ids": [deck_id], "target_count": 3 }),
        )
        .await;

    assert_eq!(status, 422, "{body}");
    assert_eq!(body["fields"][0]["field"], "target_count");
}
