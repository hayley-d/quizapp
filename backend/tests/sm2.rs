#![allow(dead_code)]

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

async fn next_card(app: &common::TestApp, session_id: i64) -> (u16, Value) {
    let (status, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    (status.as_u16(), body)
}

async fn answer_self_graded(
    app: &common::TestApp,
    session_id: i64,
    card_id: i64,
    self_grade: &str,
) -> Value {
    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": self_grade }),
        )
        .await;
    assert_eq!(status, 200, "could not answer: {body}");
    body
}

async fn answer_whatever_was_served(
    app: &common::TestApp,
    session_id: i64,
    card: &Value,
) -> Value {
    let card_id = card["id"].as_i64().unwrap();
    let body = match card["kind"].as_str().unwrap() {
        "flashcard" => json!({ "card_id": card_id, "self_grade": "good" }),
        "short_answer" => json!({ "card_id": card_id, "given": "anything" }),
        _ => json!({
            "card_id": card_id,
            "choice_id": card["choices"][0]["id"].as_i64().unwrap(),
        }),
    };
    let (status, answered) = app
        .post(&format!("/api/sessions/{session_id}/answer"), body)
        .await;
    assert_eq!(status, 200, "could not answer: {answered}");
    answered
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
    let live_card = create_flashcard(&app, deck_id, "live", "answer").await;
    let archived_card = create_flashcard(&app, deck_id, "archived", "answer").await;
    app.post(&format!("/api/cards/{archived_card}/archive"), json!({})).await;

    let (_, created) = app
        .post("/api/sessions", json!({ "mode": "sm2", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(created["target_count"], 1, "the archived card must not count as due");

    let session_id = created["id"].as_i64().unwrap();
    let (status, served) = next_card(&app, session_id).await;
    assert_eq!(status, 200, "{served}");
    assert_eq!(
        served["card"]["id"], live_card,
        "the archived card must never be served: {served}",
    );
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
    assert_eq!(
        body["fields"][0]["message"],
        "A spaced repetition session is whatever is due, so its length is not yours to set",
    );
}

#[tokio::test]
async fn the_most_overdue_card_is_served_first() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let recent = create_flashcard(&app, deck_id, "recent", "answer").await;
    let ancient = create_flashcard(&app, deck_id, "ancient", "answer").await;
    set_due_at(&app, recent, "2020-06-01T00:00:00Z").await;
    set_due_at(&app, ancient, "2019-01-01T00:00:00Z").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let (status, served) = next_card(&app, session_id).await;

    assert_eq!(status, 200, "{served}");
    assert_eq!(served["mode"], "sm2");
    assert_eq!(
        served["card"]["id"], ancient,
        "the card overdue since 2019 must come before the one overdue since 2020: {served}",
    );
}

#[tokio::test]
async fn cards_due_at_the_same_moment_are_ordered_by_id() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let first = create_flashcard(&app, deck_id, "first", "answer").await;
    let second = create_flashcard(&app, deck_id, "second", "answer").await;
    set_due_at(&app, first, "2020-01-01T00:00:00Z").await;
    set_due_at(&app, second, "2020-01-01T00:00:00Z").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let (_, served) = next_card(&app, session_id).await;

    assert_eq!(served["card"]["id"], first.min(second));
}

#[tokio::test]
async fn a_reload_serves_the_same_card() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "answer one").await;
    create_flashcard(&app, deck_id, "two", "answer two").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let (_, first_serve) = next_card(&app, session_id).await;
    let (_, second_serve) = next_card(&app, session_id).await;

    assert_eq!(
        first_serve["card"]["id"], second_serve["card"]["id"],
        "an unanswered serve wrote no review row, so the same card must come back",
    );
}

#[tokio::test]
async fn the_serve_carries_no_answer_content() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "flash", "the secret answer").await;
    create_multiple_choice(&app, deck_id, "choice").await;
    create_short_answer(&app, deck_id, "short").await;

    let session_id = start_sm2_session(&app, deck_id).await;

    for _ in 0..3 {
        let (status, served) = next_card(&app, session_id).await;
        assert_eq!(status, 200, "{served}");
        let card = &served["card"];
        assert!(card.get("answer_md").is_none(), "answer_md leaked: {card}");
        assert!(card.get("explanation_md").is_none(), "explanation_md leaked: {card}");
        assert!(card.get("accepted").is_none(), "accepted leaked: {card}");
        let rendered = card.to_string();
        assert!(!rendered.contains("is_correct"), "is_correct leaked: {card}");
        assert!(!rendered.contains("the secret answer"), "the answer leaked: {card}");

        answer_whatever_was_served(&app, session_id, card).await;
    }
}

#[tokio::test]
async fn an_answered_card_is_not_served_again_and_the_session_runs_out() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "only", "answer").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let (_, served) = next_card(&app, session_id).await;
    let card_id = served["card"]["id"].as_i64().unwrap();
    answer_self_graded(&app, session_id, card_id, "good").await;

    let (status, body) = next_card(&app, session_id).await;
    assert_eq!(status, 409, "{body}");
}

#[tokio::test]
async fn a_correct_answer_schedules_the_card_a_day_out() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "answer").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let answered = answer_self_graded(&app, session_id, card_id, "good").await;
    let review_id = answered["review_id"].as_i64().unwrap();

    let (due_at, interval_days, ease, repetitions, lapses) = app.schedule_state_for(card_id).await;
    assert_eq!(repetitions, 1);
    assert_eq!(lapses, 0);
    assert_eq!(interval_days, 1.0);
    assert_eq!(ease, 2.5, "quality 4 leaves the ease exactly where it started");
    assert!(due_at.ends_with("T00:00:00Z"), "due_at must be midnight UTC: {due_at}");
    assert_eq!(
        due_at.len(),
        20,
        "due_at must be exactly YYYY-MM-DDT00:00:00Z with no leftover time-of-day: {due_at}",
    );

    let answered_at = app.answered_at_for_review(review_id).await;
    let expected_due_at = app.date_advanced_by_days(&answered_at, 1).await;
    assert_eq!(
        due_at, expected_due_at,
        "due_at must be exactly one day after the review's own answered_at: \
         answered_at={answered_at}",
    );
}

#[tokio::test]
async fn a_correct_multiple_choice_answer_schedules_the_card_a_day_out() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_multiple_choice(&app, deck_id, "which linkage merges the closest pair")
        .await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let (_, served) = next_card(&app, session_id).await;
    let card = &served["card"];
    let correct_choice_id = card["choices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|choice| choice["text_md"].as_str().unwrap() == "complete linkage")
        .unwrap()["id"]
        .as_i64()
        .unwrap();

    let (status, answered) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "choice_id": correct_choice_id }),
        )
        .await;
    assert_eq!(status, 200, "{answered}");
    assert_eq!(answered["correct"], json!(true), "{answered}");
    let review_id = answered["review_id"].as_i64().unwrap();

    let (due_at, interval_days, _, repetitions, lapses) = app.schedule_state_for(card_id).await;
    assert_eq!(repetitions, 1);
    assert_eq!(lapses, 0);
    assert_eq!(interval_days, 1.0);

    let answered_at = app.answered_at_for_review(review_id).await;
    let expected_due_at = app.date_advanced_by_days(&answered_at, 1).await;
    assert_eq!(
        due_at, expected_due_at,
        "due_at must be exactly one day after the review's own answered_at: \
         answered_at={answered_at}",
    );
}

#[tokio::test]
async fn a_lapse_resets_the_card_without_touching_the_ease() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "answer").await;

    let first_session = start_sm2_session(&app, deck_id).await;
    answer_self_graded(&app, first_session, card_id, "easy").await;
    let (_, _, ease_after_easy, _, _) = app.schedule_state_for(card_id).await;

    set_due_at(&app, card_id, "2020-01-01T00:00:00Z").await;
    let second_session = start_sm2_session(&app, deck_id).await;
    answer_self_graded(&app, second_session, card_id, "again").await;

    let (_, interval_days, ease, repetitions, lapses) = app.schedule_state_for(card_id).await;
    assert_eq!(repetitions, 0);
    assert_eq!(lapses, 1);
    assert_eq!(interval_days, 1.0);
    assert_eq!(ease, ease_after_easy, "a lapse must not move the ease factor");
}

#[tokio::test]
async fn a_practice_session_leaves_the_schedule_alone() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "answer").await;

    let (_, session) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;
    let session_id = session["id"].as_i64().unwrap();

    let before = app.schedule_state_for(card_id).await;
    answer_self_graded(&app, session_id, card_id, "good").await;
    let after = app.schedule_state_for(card_id).await;

    assert_eq!(before, after, "only sm2 mode may write the schedule");
}

#[tokio::test]
async fn an_sm2_flashcard_still_reveals_and_self_grades() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "the answer").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let (status, revealed) = app
        .post(
            &format!("/api/sessions/{session_id}/reveal"),
            json!({ "card_id": card_id }),
        )
        .await;

    assert_eq!(status, 200, "sm2 must keep the practice reveal: {revealed}");
    assert_eq!(revealed["answer_md"], "the answer");

    let answered = answer_self_graded(&app, session_id, card_id, "good").await;
    assert_eq!(answered["mode"], "sm2");
    assert_eq!(answered["correct"], true);
}

#[tokio::test]
async fn a_failed_schedule_write_rolls_back_the_review() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "answer").await;
    let session_id = start_sm2_session(&app, deck_id).await;

    sqlx::query("DROP TABLE schedule").execute(&app.pool).await.unwrap();

    let (status, _) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "good" }),
        )
        .await;

    assert_eq!(status, 500);
    assert_eq!(
        app.count("SELECT COUNT(*) FROM reviews").await,
        0,
        "the review must roll back with the schedule write",
    );
}

async fn answer_typed(
    app: &common::TestApp,
    session_id: i64,
    card_id: i64,
    given: &str,
) -> Value {
    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": given }),
        )
        .await;
    assert_eq!(status, 200, "could not answer: {body}");
    body
}

#[tokio::test]
async fn an_override_recomputes_the_schedule_rather_than_leaving_the_lapse() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_short_answer(&app, deck_id, "short").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let answered = answer_typed(&app, session_id, card_id, "hopelessly wrong").await;
    assert_eq!(answered["correct"], false);

    let (_, _, _, repetitions_after_miss, lapses_after_miss) =
        app.schedule_state_for(card_id).await;
    assert_eq!(repetitions_after_miss, 0);
    assert_eq!(lapses_after_miss, 1);

    let review_id = answered["review_id"].as_i64().unwrap();
    let (status, body) = app
        .post(&format!("/api/reviews/{review_id}/override"), json!({}))
        .await;
    assert_eq!(status, 200, "{body}");

    let (_, interval_days, _, repetitions, lapses) = app.schedule_state_for(card_id).await;
    assert_eq!(repetitions, 1, "the replay must treat the override as a correct answer");
    assert_eq!(lapses, 0, "the lapse must be replayed away, not left behind");
    assert_eq!(interval_days, 1.0);
}

#[tokio::test]
async fn the_replay_ignores_reviews_from_other_modes() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_short_answer(&app, deck_id, "short").await;

    let (_, practice) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;
    let practice_id = practice["id"].as_i64().unwrap();
    answer_typed(&app, practice_id, card_id, "wrong in practice").await;
    answer_typed(&app, practice_id, card_id, "wrong again in practice").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let answered = answer_typed(&app, session_id, card_id, "wrong in sm2").await;
    let review_id = answered["review_id"].as_i64().unwrap();
    app.post(&format!("/api/reviews/{review_id}/override"), json!({})).await;

    let (_, _, _, repetitions, lapses) = app.schedule_state_for(card_id).await;
    assert_eq!(repetitions, 1, "only the one sm2 review may feed the replay");
    assert_eq!(lapses, 0);
}

#[tokio::test]
async fn overriding_a_practice_review_leaves_the_schedule_alone() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_short_answer(&app, deck_id, "short").await;

    let (_, practice) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;
    let practice_id = practice["id"].as_i64().unwrap();
    let answered = answer_typed(&app, practice_id, card_id, "wrong").await;
    let review_id = answered["review_id"].as_i64().unwrap();

    let before = app.schedule_for(card_id).await;
    app.post(&format!("/api/reviews/{review_id}/override"), json!({})).await;
    let after = app.schedule_for(card_id).await;

    assert_eq!(before, after, "a practice override must not reach the schedule");
}

#[tokio::test]
async fn an_sm2_flashcard_override_is_still_refused() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "the answer").await;

    let session_id = start_sm2_session(&app, deck_id).await;
    let answered = answer_self_graded(&app, session_id, card_id, "again").await;
    let review_id = answered["review_id"].as_i64().unwrap();

    let (status, body) = app
        .post(&format!("/api/reviews/{review_id}/override"), json!({}))
        .await;

    assert_eq!(status, 409, "a self-grade is the student's own verdict: {body}");
}
