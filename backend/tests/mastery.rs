use serde_json::{json, Value};

mod common;

async fn create_deck(app: &common::TestApp, name: &str) -> i64 {
    let (_, deck) = app
        .post("/api/decks", json!({ "name": name, "module_id": null, "description": "" }))
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

async fn start_practice(app: &common::TestApp, deck_id: i64, mastery_goal: Option<i64>) -> Value {
    let (_, session) = app
        .post(
            "/api/sessions",
            json!({ "mode": "practice", "deck_ids": [deck_id], "mastery_goal": mastery_goal }),
        )
        .await;
    session
}

async fn backdate_session_start(app: &common::TestApp, session_id: i64, started_at: &str) {
    sqlx::query("UPDATE sessions SET started_at = ? WHERE id = ?")
        .bind(started_at)
        .bind(session_id)
        .execute(&app.pool)
        .await
        .unwrap();
}

async fn insert_review(
    app: &common::TestApp,
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

async fn finish(app: &common::TestApp, session_id: i64) -> Value {
    let (_, summary) = app.post(&format!("/api/sessions/{session_id}/finish"), json!({})).await;
    summary
}

fn movement_for(summary: &Value, card_id: i64) -> Option<&Value> {
    summary["mastery_movements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|movement| movement["card_id"].as_i64() == Some(card_id))
}

#[tokio::test]
async fn a_practice_session_stores_and_echoes_its_goal() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    create_flashcard(&app, deck_id, "one").await;
    create_flashcard(&app, deck_id, "two").await;

    let session = start_practice(&app, deck_id, Some(2)).await;
    assert_eq!(session["mastery_goal"], 2);

    let session_id = session["id"].as_i64().unwrap();
    let (_, served) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(served["mastery_goal"], 2);
    assert_eq!(served["mastery_moved_up_count"], 0);
}

#[tokio::test]
async fn a_session_created_without_a_goal_reports_a_null_one() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    create_flashcard(&app, deck_id, "one").await;

    let session = start_practice(&app, deck_id, None).await;
    assert!(session["mastery_goal"].is_null());

    let session_id = session["id"].as_i64().unwrap();
    let (_, served) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert!(served["mastery_goal"].is_null());
}

#[tokio::test]
async fn a_mock_test_refuses_a_goal() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    create_flashcard(&app, deck_id, "one").await;

    let (status, body) = app
        .post(
            "/api/sessions",
            json!({ "mode": "mock", "deck_ids": [deck_id], "mastery_goal": 2 }),
        )
        .await;

    assert_eq!(status, 422, "{body}");
    let fields: Vec<&str> = body["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["field"].as_str().unwrap())
        .collect();
    assert!(fields.contains(&"mastery_goal"), "{body}");
}

#[tokio::test]
async fn a_goal_must_be_positive_and_no_larger_than_the_deck() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    create_flashcard(&app, deck_id, "one").await;

    for rejected_goal in [0, -3, 99] {
        let (status, body) = app
            .post(
                "/api/sessions",
                json!({
                    "mode": "practice",
                    "deck_ids": [deck_id],
                    "mastery_goal": rejected_goal,
                }),
            )
            .await;
        assert_eq!(status, 422, "a goal of {rejected_goal} must be refused: {body}");
    }
}

#[tokio::test]
async fn answering_a_card_correctly_twice_walks_it_up_the_ladder() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;

    let session = start_practice(&app, deck_id, Some(1)).await;
    let session_id = session["id"].as_i64().unwrap();

    let (_, first) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "good" }),
        )
        .await;
    assert_eq!(first["level_before"], "unseen");
    assert_eq!(first["level_after"], "learning");
    assert_eq!(first["mastery_direction"], "up");
    assert_eq!(first["mastery_moved_up_count"], 1);

    let (_, second) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "good" }),
        )
        .await;
    assert_eq!(
        second["level_after"], "solid",
        "a second correct answer takes it to solid, and the card is still only one card",
    );
    assert_eq!(
        second["mastery_moved_up_count"], 1,
        "the goal counts cards that moved, not rungs climbed",
    );
}

#[tokio::test]
async fn the_moved_up_count_survives_a_reload() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;
    create_flashcard(&app, deck_id, "two").await;

    let session = start_practice(&app, deck_id, Some(2)).await;
    let session_id = session["id"].as_i64().unwrap();
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "self_grade": "good" }),
    )
    .await;

    let (_, first_serve) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    let (_, second_serve) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(
        first_serve["mastery_moved_up_count"], second_serve["mastery_moved_up_count"],
        "session state lives only in reviews, so re-serving must not change the count",
    );
    assert_eq!(first_serve["mastery_moved_up_count"], 1);
}

#[tokio::test]
async fn a_session_reports_only_its_own_contribution_when_two_sessions_overlap() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;

    let earlier = start_practice(&app, deck_id, None).await;
    let earlier_id = earlier["id"].as_i64().unwrap();
    backdate_session_start(&app, earlier_id, "2026-08-30T09:00:00Z").await;

    let later = start_practice(&app, deck_id, None).await;
    let later_id = later["id"].as_i64().unwrap();
    backdate_session_start(&app, later_id, "2026-08-30T10:00:00Z").await;

    insert_review(&app, earlier_id, card_id, true, "2026-08-30T11:00:00Z").await;

    let later_summary = finish(&app, later_id).await;
    assert!(
        movement_for(&later_summary, card_id).is_none(),
        "the overlapping session's own review must not be credited to this one: {later_summary}",
    );
    assert_eq!(later_summary["mastery_moved_up_count"], 0);

    let earlier_summary = finish(&app, earlier_id).await;
    assert_eq!(
        movement_for(&earlier_summary, card_id).unwrap()["level_after"],
        "learning",
        "the session that actually wrote the review owns the movement: {earlier_summary}",
    );
    assert_eq!(earlier_summary["mastery_moved_up_count"], 1);
}

#[tokio::test]
async fn movement_is_measured_from_the_session_start_not_from_now() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;

    let warm_up = start_practice(&app, deck_id, None).await;
    let warm_up_id = warm_up["id"].as_i64().unwrap();
    backdate_session_start(&app, warm_up_id, "2026-08-29T09:00:00Z").await;
    insert_review(&app, warm_up_id, card_id, true, "2026-08-29T09:05:00Z").await;
    insert_review(&app, warm_up_id, card_id, true, "2026-08-29T09:10:00Z").await;

    let session = start_practice(&app, deck_id, None).await;
    let session_id = session["id"].as_i64().unwrap();
    backdate_session_start(&app, session_id, "2026-08-30T09:00:00Z").await;
    insert_review(&app, session_id, card_id, true, "2026-08-30T09:05:00Z").await;

    let summary = finish(&app, session_id).await;
    let movement = movement_for(&summary, card_id).unwrap();
    assert_eq!(
        movement["level_before"], "solid",
        "yesterday's review belongs to the before picture, not to this session: {summary}",
    );
    assert_eq!(movement["level_after"], "mastered");
    assert_eq!(movement["direction"], "up");
}

#[tokio::test]
async fn a_card_that_did_not_change_level_is_reported_as_unchanged() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;

    let warm_up = start_practice(&app, deck_id, None).await;
    let warm_up_id = warm_up["id"].as_i64().unwrap();
    backdate_session_start(&app, warm_up_id, "2026-08-29T09:00:00Z").await;
    insert_review(&app, warm_up_id, card_id, false, "2026-08-29T09:05:00Z").await;

    let session = start_practice(&app, deck_id, None).await;
    let session_id = session["id"].as_i64().unwrap();
    backdate_session_start(&app, session_id, "2026-08-30T09:00:00Z").await;
    insert_review(&app, session_id, card_id, false, "2026-08-30T09:05:00Z").await;

    let summary = finish(&app, session_id).await;
    let movement = movement_for(&summary, card_id).unwrap();
    assert_eq!(movement["level_before"], "shaky");
    assert_eq!(movement["level_after"], "shaky");
    assert_eq!(movement["direction"], "unchanged");
    assert_eq!(summary["mastery_moved_up_count"], 0);
    assert_eq!(summary["mastery_moved_down_count"], 0);
}

#[tokio::test]
async fn a_card_can_move_down_and_is_counted_separately() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;

    let warm_up = start_practice(&app, deck_id, None).await;
    let warm_up_id = warm_up["id"].as_i64().unwrap();
    backdate_session_start(&app, warm_up_id, "2026-08-29T09:00:00Z").await;
    insert_review(&app, warm_up_id, card_id, true, "2026-08-29T09:05:00Z").await;
    insert_review(&app, warm_up_id, card_id, true, "2026-08-29T09:10:00Z").await;
    insert_review(&app, warm_up_id, card_id, true, "2026-08-29T23:05:00Z").await;

    let session = start_practice(&app, deck_id, None).await;
    let session_id = session["id"].as_i64().unwrap();
    backdate_session_start(&app, session_id, "2026-08-30T09:00:00Z").await;
    insert_review(&app, session_id, card_id, false, "2026-08-30T09:05:00Z").await;

    let summary = finish(&app, session_id).await;
    let movement = movement_for(&summary, card_id).unwrap();
    assert_eq!(movement["level_before"], "mastered");
    assert_eq!(movement["direction"], "down");
    assert_eq!(summary["mastery_moved_down_count"], 1);
    assert_eq!(summary["mastery_moved_up_count"], 0);
}

#[tokio::test]
async fn a_mock_test_reports_movement_even_though_it_takes_no_goal() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;

    let (_, session) = app
        .post("/api/sessions", json!({ "mode": "mock", "deck_ids": [deck_id] }))
        .await;
    let session_id = session["id"].as_i64().unwrap();
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "given": "an answer" }),
    )
    .await;

    let summary = finish(&app, session_id).await;
    assert!(summary["mastery_goal"].is_null());
    assert_eq!(movement_for(&summary, card_id).unwrap()["level_after"], "learning");
    assert_eq!(summary["mastery_moved_up_count"], 1);
}

#[tokio::test]
async fn finish_and_results_report_the_same_movement() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;

    let session = start_practice(&app, deck_id, Some(1)).await;
    let session_id = session["id"].as_i64().unwrap();
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "self_grade": "good" }),
    )
    .await;

    let summary = finish(&app, session_id).await;
    let (_, results) = app.get(&format!("/api/sessions/{session_id}/results")).await;
    assert_eq!(summary["mastery_movements"], results["summary"]["mastery_movements"]);
    assert_eq!(
        summary["mastery_moved_up_count"],
        results["summary"]["mastery_moved_up_count"],
    );
}

#[tokio::test]
async fn overriding_a_typed_answer_lifts_the_level_and_the_goal_counter() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "text_answer",
                "prompt_md": "What is MapReduce?",
                "accepted": [{ "text": "a programming model", "is_primary": true }],
            }),
        )
        .await;
    let card_id = card["id"].as_i64().unwrap();

    let session = start_practice(&app, deck_id, Some(1)).await;
    let session_id = session["id"].as_i64().unwrap();
    let (_, answer) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "a model for programming" }),
        )
        .await;
    assert_eq!(answer["correct"], false);
    assert_eq!(answer["mastery_moved_up_count"], 0);

    let review_id = answer["review_id"].as_i64().unwrap();
    let (status, overridden) = app
        .post(&format!("/api/reviews/{review_id}/override"), json!({}))
        .await;
    assert_eq!(status, 200, "{overridden}");
    assert_eq!(overridden["level_after"], "learning");
    assert_eq!(overridden["mastery_direction"], "up");
    assert_eq!(overridden["mastery_moved_up_count"], 1);

    let (_, served) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(
        served["mastery_moved_up_count"], 1,
        "the runner's counter and the override response must not be able to disagree",
    );
}

#[tokio::test]
async fn overriding_a_mock_flashcard_also_reports_the_level_it_reached() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;

    let (_, session) = app
        .post("/api/sessions", json!({ "mode": "mock", "deck_ids": [deck_id] }))
        .await;
    let session_id = session["id"].as_i64().unwrap();
    let (_, answer) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "something else entirely" }),
        )
        .await;
    finish(&app, session_id).await;

    let (_, results) = app.get(&format!("/api/sessions/{session_id}/results")).await;
    let review_id = results["questions"].as_array().unwrap()[0]["review_id"].as_i64().unwrap();
    let _ = answer;

    let (status, overridden) = app
        .post(&format!("/api/reviews/{review_id}/override"), json!({}))
        .await;
    assert_eq!(status, 200, "{overridden}");
    assert_eq!(
        overridden["level_after"], "learning",
        "the flashcard branch returns early and must still report movement: {overridden}",
    );
    assert_eq!(overridden["mastery_direction"], "up");
}

#[tokio::test]
async fn a_card_archived_after_the_session_leaves_the_movement_list() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Data Mining").await;
    let card_id = create_flashcard(&app, deck_id, "one").await;
    create_flashcard(&app, deck_id, "two").await;

    let session = start_practice(&app, deck_id, None).await;
    let session_id = session["id"].as_i64().unwrap();
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "self_grade": "good" }),
    )
    .await;

    let before_archiving = finish(&app, session_id).await;
    assert!(movement_for(&before_archiving, card_id).is_some());

    app.post(&format!("/api/cards/{card_id}/archive"), json!({})).await;

    let (_, after_archiving) = app.get(&format!("/api/sessions/{session_id}/results")).await;
    assert!(
        movement_for(&after_archiving["summary"], card_id).is_none(),
        "an archived card leaves the ladder, matching the deck stats rule: {after_archiving}",
    );
}
