use serde_json::{json, Value};

mod common;

const SEVEN_REQUIREMENTS: &str = "\
1. Ability to mine a variety of pattern types
2. Mining should be interactive
3. Mine patterns at varying granularity levels
4. Allow users to provide a priori knowledge hints
5. Should associate quality measures with patterns
6. Presentation and visualisation is often very important
7. Noisy and incomplete data must be handled";

async fn create_deck(app: &common::TestApp, name: &str) -> i64 {
    let (_, deck) = app
        .post("/api/decks", json!({ "name": name, "module_id": null, "description": "" }))
        .await;
    deck["id"].as_i64().unwrap()
}

async fn create_list_card(app: &common::TestApp, deck_id: i64, prompt: &str, answer: &str) -> i64 {
    let (status, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "text_answer",
                "prompt_md": prompt,
                "accepted": [{ "text": answer, "is_primary": true }],
            }),
        )
        .await;
    assert_eq!(status, 201, "{card}");
    card["id"].as_i64().unwrap()
}

async fn start_practice(app: &common::TestApp, deck_id: i64) -> i64 {
    let (_, session) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;
    session["id"].as_i64().unwrap()
}

async fn serve(app: &common::TestApp, session_id: i64) -> Value {
    let (status, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(status, 200, "{body}");
    body
}

async fn reveal(app: &common::TestApp, session_id: i64, card_id: i64, given: &str) -> Value {
    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/reveal"),
            json!({ "card_id": card_id, "given": given }),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    body
}

async fn tick(
    app: &common::TestApp,
    session_id: i64,
    card_id: i64,
    recalled_point_keys: Vec<String>,
) -> (axum::http::StatusCode, Value) {
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "recalled_point_keys": recalled_point_keys }),
    )
    .await
}

fn point_keys(reveal_body: &Value) -> Vec<String> {
    reveal_body["answer_points"]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| point["key"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn a_seven_point_answer_is_served_as_seven_points_with_first_word_cues() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    create_list_card(&app, deck_id, "list the 7 requirements", SEVEN_REQUIREMENTS).await;
    let session_id = start_practice(&app, deck_id).await;

    let body = serve(&app, session_id).await;
    let points = &body["card"]["answer_points"];
    assert_eq!(points["total"].as_i64().unwrap(), 7);
    assert_eq!(points["full_total"].as_i64().unwrap(), 7);
    assert!(!points["focused"].as_bool().unwrap());
    assert_eq!(
        points["cues"]["tier"].as_str().unwrap(),
        "word",
        "a card with no history opens on the strongest scaffold",
    );
    assert_eq!(points["cues"]["visible"].as_array().unwrap().len(), 7);
    assert!(
        !body.to_string().contains("granularity"),
        "a serve must never carry the point text: {body}",
    );
}

#[tokio::test]
async fn a_single_line_answer_is_still_an_ordinary_text_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "what is k-means", "k-means clustering").await;
    let session_id = start_practice(&app, deck_id).await;

    let body = serve(&app, session_id).await;
    assert!(body["card"]["answer_points"].is_null(), "{body}");

    let (status, revealed) = app
        .post(
            &format!("/api/sessions/{session_id}/reveal"),
            json!({ "card_id": card_id }),
        )
        .await;
    assert_eq!(status, 409, "a plain text answer is still not revealable: {revealed}");

    let (status, answered) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "k-means clustering" }),
        )
        .await;
    assert_eq!(status, 200, "{answered}");
    assert!(answered["correct"].as_bool().unwrap());
}

#[tokio::test]
async fn typing_the_points_pre_ticks_the_checklist_whatever_the_order_or_spelling() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "the 4 v's", "1. Volume\n2. Velocity\n3. Variety\n4. Veracity").await;
    let session_id = start_practice(&app, deck_id).await;

    let revealed = reveal(&app, session_id, card_id, "veracity\nVOLUME!\nvelocty").await;
    let matched: Vec<bool> = revealed["answer_points"]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| point["matched_what_you_typed"].as_bool().unwrap())
        .collect();
    assert_eq!(
        matched,
        vec![true, true, false, true],
        "order must not matter, casing must not matter, one typo must be forgiven: {revealed}",
    );
}

#[tokio::test]
async fn a_partial_score_grades_hard_and_comes_back_as_a_focus_repetition() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "list the 7 requirements", SEVEN_REQUIREMENTS).await;
    let session_id = start_practice(&app, deck_id).await;

    let revealed = reveal(&app, session_id, card_id, "").await;
    let keys = point_keys(&revealed);
    assert_eq!(keys.len(), 7);

    let (status, answered) = tick(&app, session_id, card_id, keys[..5].to_vec()).await;
    assert_eq!(status, 200, "{answered}");
    assert!(
        answered["correct"].as_bool().unwrap(),
        "five of seven is a pass, not a failure: {answered}",
    );
    assert!(
        !answered["can_override"].as_bool().unwrap(),
        "ticking the checklist replaces the override entirely: {answered}",
    );

    let body = serve(&app, session_id).await;
    assert_eq!(
        body["card"]["id"].as_i64().unwrap(),
        card_id,
        "a card with points left over must be served again: {body}",
    );
    let points = &body["card"]["answer_points"];
    assert!(points["focused"].as_bool().unwrap(), "{body}");
    assert_eq!(points["total"].as_i64().unwrap(), 2, "only the missed points are asked for");
    assert_eq!(points["full_total"].as_i64().unwrap(), 7);
    assert_eq!(points["cues"]["visible"].as_array().unwrap().len(), 2);

    let focused = reveal(&app, session_id, card_id, "").await;
    let focused_keys = point_keys(&focused);
    assert_eq!(focused_keys, keys[5..].to_vec(), "the focus rep asks for exactly what was missed");
    assert!(focused["answer_points"]["focused"].as_bool().unwrap());

    let (status, resolved) = tick(&app, session_id, card_id, focused_keys).await;
    assert_eq!(status, 200, "{resolved}");
    assert!(resolved["correct"].as_bool().unwrap());
}

#[tokio::test]
async fn a_full_recall_resolves_the_card_and_a_half_recall_fails_it() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "the 4 v's", "1. Volume\n2. Velocity\n3. Variety\n4. Veracity").await;
    let session_id = start_practice(&app, deck_id).await;

    let keys = point_keys(&reveal(&app, session_id, card_id, "").await);

    let (_, answered) = tick(&app, session_id, card_id, keys[..2].to_vec()).await;
    assert!(
        !answered["correct"].as_bool().unwrap(),
        "exactly half recalled is a failed repetition: {answered}",
    );

    let (_, answered) = tick(&app, session_id, card_id, keys[2..].to_vec()).await;
    assert!(answered["correct"].as_bool().unwrap(), "{answered}");

    let self_grades = sqlx::query_scalar!(
        r#"SELECT self_grade AS "self_grade!: String" FROM reviews
           WHERE card_id = ? ORDER BY id"#,
        card_id,
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        self_grades,
        vec!["again", "easy"],
        "the score is the grade: half is again, and a clean sweep with no hints is easy",
    );
}

#[tokio::test]
async fn a_hint_costs_the_easy_grade_but_not_the_pass() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "the 2 types", "- alpha one\n- beta two").await;
    let session_id = start_practice(&app, deck_id).await;

    let keys = point_keys(&reveal(&app, session_id, card_id, "").await);
    let (status, answered) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "recalled_point_keys": keys, "hints_used": true }),
        )
        .await;
    assert_eq!(status, 200, "{answered}");
    assert!(answered["correct"].as_bool().unwrap());

    let self_grade = sqlx::query_scalar!(
        r#"SELECT self_grade AS "self_grade!: String" FROM reviews WHERE card_id = ?"#,
        card_id,
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(self_grade, "good", "a hinted clean sweep is good, not easy");
}

#[tokio::test]
async fn a_point_the_card_never_offered_is_refused() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "the 2 types", "- alpha one\n- beta two").await;
    let session_id = start_practice(&app, deck_id).await;

    let (status, body) = tick(&app, session_id, card_id, vec!["gamma three".to_string()]).await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(body["fields"][0]["field"].as_str().unwrap(), "recalled_point_keys");
}

#[tokio::test]
async fn a_multi_point_card_refuses_the_grades_and_fields_of_other_kinds() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "the 2 types", "- alpha one\n- beta two").await;
    let session_id = start_practice(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "good" }),
        )
        .await;
    assert_eq!(status, 422, "{body}");

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "alpha one" }),
        )
        .await;
    assert_eq!(status, 422, "a multi-point card is scored by its ticks, not by matching: {body}");
    assert_eq!(body["fields"][0]["field"].as_str().unwrap(), "recalled_point_keys");
}

#[tokio::test]
async fn ordinary_cards_still_refuse_point_ticks() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "what is k-means", "k-means clustering").await;
    let session_id = start_practice(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "k-means", "recalled_point_keys": [] }),
        )
        .await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(body["fields"][0]["field"].as_str().unwrap(), "recalled_point_keys");
}

#[tokio::test]
async fn a_multi_point_review_can_never_be_overridden() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "the 4 v's", "1. Volume\n2. Velocity\n3. Variety\n4. Veracity").await;
    let session_id = start_practice(&app, deck_id).await;

    let keys = point_keys(&reveal(&app, session_id, card_id, "").await);
    let (_, answered) = tick(&app, session_id, card_id, keys[..1].to_vec()).await;
    assert!(!answered["correct"].as_bool().unwrap());
    let review_id = answered["review_id"].as_i64().unwrap();

    let (status, body) = app.post(&format!("/api/reviews/{review_id}/override"), json!({})).await;
    assert_eq!(status, 409, "{body}");

    let accepted_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!: i64" FROM accepted WHERE card_id = ?"#,
        card_id,
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        accepted_count, 1,
        "a refused override must not teach the card a new wording",
    );
}

#[tokio::test]
async fn the_author_can_force_a_list_off_and_a_plain_answer_on() {
    let app = common::spawn_app().await;
    let forced_off_deck = create_deck(&app, "prose").await;
    let forced_on_deck = create_deck(&app, "unmarked list").await;

    let (status, forced_off) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": forced_off_deck,
                "kind": "text_answer",
                "prompt_md": "prose",
                "accepted": [{ "text": "1. Volume\n2. Velocity", "is_primary": true }],
                "multi_point_mode": "off",
            }),
        )
        .await;
    assert_eq!(status, 201, "{forced_off}");

    let (status, forced_on) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": forced_on_deck,
                "kind": "text_answer",
                "prompt_md": "unmarked list",
                "accepted": [{ "text": "Volume\nVelocity", "is_primary": true }],
                "multi_point_mode": "on",
            }),
        )
        .await;
    assert_eq!(status, 201, "{forced_on}");

    let off_session = start_practice(&app, forced_off_deck).await;
    let served = serve(&app, off_session).await;
    assert!(
        served["card"]["answer_points"].is_null(),
        "an answer forced off stays one opaque string: {served}",
    );

    let on_session = start_practice(&app, forced_on_deck).await;
    let served = serve(&app, on_session).await;
    assert_eq!(
        served["card"]["answer_points"]["total"].as_i64().unwrap(),
        2,
        "an answer forced on splits its unmarked lines: {served}",
    );

    let card_id = forced_on["id"].as_i64().unwrap();
    let keys = point_keys(&reveal(&app, on_session, card_id, "Velocity").await);
    assert_eq!(keys, vec!["volume".to_string(), "velocity".to_string()]);
}

#[tokio::test]
async fn forcing_a_one_point_answer_on_is_rejected() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;

    let (status, body) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "text_answer",
                "prompt_md": "one thing",
                "accepted": [{ "text": "k-means", "is_primary": true }],
                "multi_point_mode": "on",
            }),
        )
        .await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(body["fields"][0]["field"].as_str().unwrap(), "multi_point_mode");
}

#[tokio::test]
async fn the_results_screen_reports_the_score_and_what_was_missed() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mining").await;
    let card_id = create_list_card(&app, deck_id, "the 4 v's", "1. Volume\n2. Velocity\n3. Variety\n4. Veracity").await;
    let session_id = start_practice(&app, deck_id).await;

    let keys = point_keys(&reveal(&app, session_id, card_id, "").await);
    tick(&app, session_id, card_id, keys[..3].to_vec()).await;
    app.post(&format!("/api/sessions/{session_id}/finish"), json!({})).await;

    let (status, results) = app.get(&format!("/api/sessions/{session_id}/results")).await;
    assert_eq!(status, 200, "{results}");
    let question = &results["questions"][0];
    assert_eq!(question["answer_points"]["recalled"].as_i64().unwrap(), 3);
    assert_eq!(question["answer_points"]["total"].as_i64().unwrap(), 4);
    assert_eq!(
        question["answer_points"]["missed"].as_array().unwrap(),
        &vec![Value::from("Veracity")],
    );
    assert!(!question["can_override"].as_bool().unwrap());
}

#[tokio::test]
async fn the_editor_can_preview_how_an_answer_will_split() {
    let app = common::spawn_app().await;

    let (status, preview) = app
        .post(
            "/api/cards/answer-points-preview",
            json!({ "source": SEVEN_REQUIREMENTS }),
        )
        .await;
    assert_eq!(status, 200, "{preview}");
    assert!(preview["multi_point"].as_bool().unwrap());
    assert_eq!(preview["points"].as_array().unwrap().len(), 7);
    assert_eq!(
        preview["points"][1]["text"].as_str().unwrap(),
        "Mining should be interactive",
    );
    assert!(preview["notes"].as_array().unwrap().is_empty());

    let (_, prose) = app
        .post(
            "/api/cards/answer-points-preview",
            json!({ "source": "One paragraph.\nAnother paragraph." }),
        )
        .await;
    assert!(
        !prose["multi_point"].as_bool().unwrap(),
        "unmarked prose must preview as an ordinary answer: {prose}",
    );

    let (_, forced) = app
        .post(
            "/api/cards/answer-points-preview",
            json!({ "source": "One paragraph.\nAnother paragraph.", "multi_point_mode": "on" }),
        )
        .await;
    assert_eq!(forced["points"].as_array().unwrap().len(), 2, "{forced}");

    let (_, with_notes) = app
        .post(
            "/api/cards/answer-points-preview",
            json!({ "source": "A data warehouse.\n1. A unified schema.\n2. One site.\n3. Pre-processed." }),
        )
        .await;
    assert_eq!(with_notes["points"].as_array().unwrap().len(), 3);
    assert_eq!(
        with_notes["notes"].as_array().unwrap(),
        &vec![Value::from("A data warehouse.")],
        "prose around the list previews as an unscored note: {with_notes}",
    );
}
