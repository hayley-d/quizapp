mod common;

use common::{spawn_app, TestApp};
use serde_json::{json, Value};

async fn deck_with_cards(app: &TestApp, name: &str, card_count: usize) -> (i64, Vec<i64>) {
    let (_, deck) = app
        .post("/api/decks", json!({"name": name, "module_id": null, "description": ""}))
        .await;
    let deck_id = deck["id"].as_i64().unwrap();

    let mut card_ids = Vec::new();
    for card_index in 0..card_count {
        let (_, card) = app
            .post(
                "/api/cards",
                json!({
                    "deck_id": deck_id,
                    "kind": "short_answer",
                    "prompt_md": format!("Question {card_index}"),
                    "answer_md": null,
                    "explanation_md": null,
                    "choices": [],
                    "accepted": [{"text": "yes", "is_primary": true}],
                }),
            )
            .await;
        card_ids.push(card["id"].as_i64().unwrap());
    }
    (deck_id, card_ids)
}

async fn insert_session(app: &TestApp, mode: &str, deck_id: i64) -> i64 {
    let deck_ids_json = format!("[{deck_id}]");
    sqlx::query("INSERT INTO sessions (mode, deck_ids) VALUES (?, ?)")
        .bind(mode)
        .bind(deck_ids_json)
        .execute(&app.pool)
        .await
        .unwrap()
        .last_insert_rowid()
}

async fn insert_review(
    app: &TestApp,
    session_id: i64,
    card_id: i64,
    correct: bool,
    answered_at: &str,
) {
    sqlx::query(
        "INSERT INTO reviews (card_id, session_id, correct, answered_at) VALUES (?, ?, ?, ?)",
    )
    .bind(card_id)
    .bind(session_id)
    .bind(if correct { 1 } else { 0 })
    .bind(answered_at)
    .execute(&app.pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn stats_for_a_deck_with_no_reviews_are_empty_rather_than_zero() {
    let app = spawn_app().await;
    let (deck_id, _) = deck_with_cards(&app, "Data Mining", 3).await;

    let (status, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    assert_eq!(status, 200);
    assert_eq!(body["summary"]["card_count"], 3);
    assert_eq!(body["summary"]["unseen_count"], 3);
    assert_eq!(body["summary"]["mock_accuracy"], Value::Null);
    assert_eq!(body["summary"]["practice_accuracy"], Value::Null);
    assert_eq!(body["summary"]["mock_review_count"], 0);
    assert_eq!(body["summary"]["practice_review_count"], 0);
    assert_eq!(body["summary"]["last_answered_at"], Value::Null);
    assert_eq!(body["cards"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn archived_cards_are_outside_the_counts() {
    let app = spawn_app().await;
    let (deck_id, card_ids) = deck_with_cards(&app, "Data Mining", 3).await;

    let (status, _) = app
        .post(&format!("/api/cards/{}/archive", card_ids[0]), json!({}))
        .await;
    assert_eq!(status, 200);

    let (_, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    assert_eq!(body["summary"]["card_count"], 2);
    assert_eq!(body["summary"]["unseen_count"], 2);
}

#[tokio::test]
async fn mock_and_practice_accuracy_are_computed_separately() {
    let app = spawn_app().await;
    let (deck_id, card_ids) = deck_with_cards(&app, "Data Mining", 4).await;

    let mock_session = insert_session(&app, "mock", deck_id).await;
    for (card_index, card_id) in card_ids.iter().enumerate() {
        let correct = card_index < 3;
        insert_review(&app, mock_session, *card_id, correct, "2026-08-20T10:00:00Z").await;
    }

    let practice_session = insert_session(&app, "practice", deck_id).await;
    for (card_index, card_id) in card_ids.iter().enumerate() {
        let correct = card_index < 1;
        insert_review(&app, practice_session, *card_id, correct, "2026-08-21T10:00:00Z").await;
    }

    let (_, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    assert_eq!(body["summary"]["mock_accuracy"].as_f64().unwrap(), 0.75);
    assert_eq!(body["summary"]["practice_accuracy"].as_f64().unwrap(), 0.25);
    assert_eq!(body["summary"]["mock_review_count"], 4);
    assert_eq!(body["summary"]["practice_review_count"], 4);
    assert_eq!(body["summary"]["unseen_count"], 0);
    assert_eq!(body["summary"]["last_answered_at"], "2026-08-21T10:00:00Z");
}

#[tokio::test]
async fn an_overridden_review_counts_as_correct() {
    let app = spawn_app().await;
    let (deck_id, card_ids) = deck_with_cards(&app, "Data Mining", 1).await;

    let session_id = insert_session(&app, "practice", deck_id).await;
    sqlx::query(
        "INSERT INTO reviews (card_id, session_id, correct, overridden, answered_at)
         VALUES (?, ?, 1, 1, '2026-08-20T10:00:00Z')",
    )
    .bind(card_ids[0])
    .bind(session_id)
    .execute(&app.pool)
    .await
    .unwrap();

    let (_, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    assert_eq!(body["summary"]["practice_accuracy"].as_f64().unwrap(), 1.0);
}

#[tokio::test]
async fn reviews_on_another_deck_do_not_appear() {
    let app = spawn_app().await;
    let (deck_id, _) = deck_with_cards(&app, "Data Mining", 1).await;
    let (other_deck_id, other_card_ids) = deck_with_cards(&app, "Clustering", 1).await;

    let session_id = insert_session(&app, "practice", other_deck_id).await;
    insert_review(&app, session_id, other_card_ids[0], false, "2026-08-20T10:00:00Z").await;

    let (_, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    assert_eq!(body["summary"]["practice_review_count"], 0);
    assert_eq!(body["summary"]["unseen_count"], 1);
    assert_eq!(body["summary"]["last_answered_at"], Value::Null);
}

#[tokio::test]
async fn stats_for_an_unknown_deck_are_a_not_found() {
    let app = spawn_app().await;

    let (status, body) = app.get("/api/decks/9999/stats").await;

    assert_eq!(status, 404);
    assert_eq!(body["fields"].as_array().unwrap().len(), 0);
}
