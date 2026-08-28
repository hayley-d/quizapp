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

async fn create_flashcard(app: &TestApp, deck_id: i64, prompt: &str, answer: &str) -> i64 {
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

async fn create_short_answer(app: &TestApp, deck_id: i64, prompt: &str) -> i64 {
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

async fn create_deck(app: &TestApp, name: &str, module_id: Option<i64>) -> i64 {
    let (_, deck) = app
        .post(
            "/api/decks",
            json!({ "name": name, "module_id": module_id, "description": "" }),
        )
        .await;
    deck["id"].as_i64().unwrap()
}

async fn start_sm2_session(app: &TestApp, deck_id: i64) -> i64 {
    let (status, session) = app
        .post("/api/sessions", json!({ "mode": "sm2", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(status, 201, "could not start an sm2 session: {session}");
    session["id"].as_i64().unwrap()
}

async fn set_due_at(app: &TestApp, card_id: i64, due_at: &str) {
    sqlx::query("UPDATE schedule SET due_at = ? WHERE card_id = ?")
        .bind(due_at)
        .bind(card_id)
        .execute(&app.pool)
        .await
        .unwrap();
}

async fn answer_self_graded(app: &TestApp, session_id: i64, card_id: i64, self_grade: &str) -> Value {
    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": self_grade }),
        )
        .await;
    assert_eq!(status, 200, "could not answer: {body}");
    body
}

async fn answer_typed(app: &TestApp, session_id: i64, card_id: i64, given: &str) -> Value {
    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": given }),
        )
        .await;
    assert_eq!(status, 200, "could not answer: {body}");
    body
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

fn card_stats_for(body: &Value, card_id: i64) -> Option<&Value> {
    body["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["card_id"].as_i64() == Some(card_id))
}

#[tokio::test]
async fn cards_with_no_reviews_are_omitted_rather_than_sent_at_zero() {
    let app = spawn_app().await;
    let (deck_id, card_ids) = deck_with_cards(&app, "Data Mining", 2).await;

    let session_id = insert_session(&app, "practice", deck_id).await;
    insert_review(&app, session_id, card_ids[0], false, "2026-08-20T10:00:00Z").await;

    let (_, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    assert_eq!(body["cards"].as_array().unwrap().len(), 1);
    assert_eq!(card_stats_for(&body, card_ids[0]).unwrap()["attempt_count"], 1);
    assert!(card_stats_for(&body, card_ids[1]).is_none());
}

#[tokio::test]
async fn archived_cards_are_omitted_from_the_card_stats() {
    let app = spawn_app().await;
    let (deck_id, card_ids) = deck_with_cards(&app, "Data Mining", 1).await;

    let session_id = insert_session(&app, "practice", deck_id).await;
    insert_review(&app, session_id, card_ids[0], false, "2026-08-20T10:00:00Z").await;
    app.post(&format!("/api/cards/{}/archive", card_ids[0]), json!({})).await;

    let (_, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    assert_eq!(body["cards"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn miss_rate_weights_recent_reviews_more_heavily() {
    let app = spawn_app().await;
    let (deck_id, card_ids) = deck_with_cards(&app, "Data Mining", 2).await;
    let session_id = insert_session(&app, "practice", deck_id).await;

    let recently_fixed = card_ids[0];
    let recently_broken = card_ids[1];

    let days = ["2026-08-20", "2026-08-21", "2026-08-22", "2026-08-23"];

    for (day_index, day) in days.iter().enumerate() {
        let answered_at = format!("{day}T10:00:00Z");
        insert_review(&app, session_id, recently_fixed, day_index >= 2, &answered_at).await;
        insert_review(&app, session_id, recently_broken, day_index < 2, &answered_at).await;
    }

    let (_, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    let fixed_miss_rate =
        card_stats_for(&body, recently_fixed).unwrap()["miss_rate"].as_f64().unwrap();
    let broken_miss_rate =
        card_stats_for(&body, recently_broken).unwrap()["miss_rate"].as_f64().unwrap();

    assert_eq!(card_stats_for(&body, recently_fixed).unwrap()["attempt_count"], 4);
    assert_eq!(card_stats_for(&body, recently_broken).unwrap()["attempt_count"], 4);
    assert!(
        broken_miss_rate > fixed_miss_rate,
        "recent misses must outweigh old ones: \
         recently_broken={broken_miss_rate}, recently_fixed={fixed_miss_rate}",
    );
}

#[tokio::test]
async fn only_the_ten_most_recent_reviews_feed_the_miss_rate() {
    let app = spawn_app().await;
    let (deck_id, card_ids) = deck_with_cards(&app, "Data Mining", 1).await;
    let session_id = insert_session(&app, "practice", deck_id).await;
    let card_id = card_ids[0];

    for review_index in 0..12 {
        let answered_at = format!("2026-08-{:02}T10:00:00Z", review_index + 1);
        insert_review(&app, session_id, card_id, true, &answered_at).await;
    }
    for review_index in 0..10 {
        let answered_at = format!("2026-09-{:02}T10:00:00Z", review_index + 1);
        insert_review(&app, session_id, card_id, false, &answered_at).await;
    }

    let (_, body) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    let entry = card_stats_for(&body, card_id).unwrap();
    assert_eq!(entry["attempt_count"], 22);
    assert_eq!(entry["miss_rate"].as_f64().unwrap(), 1.0);
}

#[tokio::test]
async fn sm2_accuracy_is_its_own_figure() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let first = create_flashcard(&app, deck_id, "one", "answer").await;
    let second = create_flashcard(&app, deck_id, "two", "answer").await;
    let third = create_flashcard(&app, deck_id, "three", "answer").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    answer_self_graded(&app, session_id, first, "good").await;
    answer_self_graded(&app, session_id, second, "again").await;

    let practice_session = insert_session(&app, "practice", deck_id).await;
    insert_review(&app, practice_session, third, true, "2026-08-21T10:00:00Z").await;

    let (_, stats) = app.get(&format!("/api/decks/{deck_id}/stats")).await;
    let summary = &stats["summary"];

    assert_eq!(summary["sm2_review_count"], 2, "practice reviews must not leak into sm2");
    assert_eq!(summary["sm2_accuracy"], 0.5);
    assert_eq!(summary["practice_accuracy"], 1.0, "sm2 must not leak into practice");
    assert!(summary["mock_accuracy"].is_null(), "sm2 must not leak into mock");
    assert_eq!(summary["practice_review_count"], 1);
    assert_eq!(summary["mock_review_count"], 0);
}

#[tokio::test]
async fn sm2_accuracy_is_null_and_not_zero_without_reviews() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "answer").await;

    let (_, stats) = app.get(&format!("/api/decks/{deck_id}/stats")).await;

    assert!(stats["summary"]["sm2_accuracy"].is_null());
    assert_eq!(stats["summary"]["sm2_review_count"], 0);
}

#[tokio::test]
async fn an_overridden_sm2_review_counts_as_correct() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_short_answer(&app, deck_id, "short").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let answered = answer_typed(&app, session_id, card_id, "wrong").await;
    let review_id = answered["review_id"].as_i64().unwrap();
    app.post(&format!("/api/reviews/{review_id}/override"), json!({})).await;

    let (_, stats) = app.get(&format!("/api/decks/{deck_id}/stats")).await;
    assert_eq!(stats["summary"]["sm2_accuracy"], 1.0);
}

#[tokio::test]
async fn the_due_count_excludes_archived_cards_and_reports_the_next_due_date() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let due_card = create_flashcard(&app, deck_id, "due", "answer").await;
    let later_card = create_flashcard(&app, deck_id, "later", "answer").await;
    let archived_card = create_flashcard(&app, deck_id, "archived", "answer").await;
    set_due_at(&app, due_card, "2020-01-01T00:00:00Z").await;
    set_due_at(&app, later_card, "2099-01-01T00:00:00Z").await;
    app.post(&format!("/api/cards/{archived_card}/archive"), json!({})).await;

    let (_, stats) = app.get(&format!("/api/decks/{deck_id}/stats")).await;
    let summary = &stats["summary"];

    assert_eq!(summary["due_count"], 1, "only the overdue, non-archived card: {summary}");
    assert_eq!(summary["next_due_at"], "2020-01-01T00:00:00Z");
}

#[tokio::test]
async fn a_card_with_no_schedule_row_still_counts_as_due() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let scheduled_card = create_flashcard(&app, deck_id, "scheduled", "answer").await;
    let unscheduled_card = create_flashcard(&app, deck_id, "unscheduled", "answer").await;
    set_due_at(&app, scheduled_card, "2099-01-01T00:00:00Z").await;

    sqlx::query("DELETE FROM schedule WHERE card_id = ?")
        .bind(unscheduled_card)
        .execute(&app.pool)
        .await
        .unwrap();

    let (_, stats) = app.get(&format!("/api/decks/{deck_id}/stats")).await;
    let summary = &stats["summary"];

    assert_eq!(
        summary["due_count"], 1,
        "the card missing a schedule row must count as due, not the not-yet-due card: {summary}",
    );
}
