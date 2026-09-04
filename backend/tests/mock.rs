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

async fn create_text_answer(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "text_answer",
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

async fn start_mock_session(app: &common::TestApp, deck_id: i64) -> i64 {
    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "mock", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(status, 201, "could not start a mock session: {body}");
    body["id"].as_i64().unwrap()
}

async fn start_practice_session(app: &common::TestApp, deck_id: i64) -> i64 {
    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(status, 201, "could not start a practice session: {body}");
    body["id"].as_i64().unwrap()
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

#[tokio::test]
async fn creates_a_mock_session_with_the_pool_size_as_its_target() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;
    create_flashcard(&app, deck_id, "two", "an answer").await;
    create_flashcard(&app, deck_id, "three", "an answer").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "mock", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 201, "{body}");
    assert_eq!(body["mode"], "mock");
    assert_eq!(body["target_count"], 3);
    assert_eq!(body["pool_count"], 3);
    assert_eq!(body["answered_count"], 0);
    assert!(body["ended_at"].is_null());
}

#[tokio::test]
async fn a_mock_target_count_excludes_archived_cards() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;
    create_flashcard(&app, deck_id, "two", "an answer").await;
    let archived = create_flashcard(&app, deck_id, "three", "an answer").await;

    app.post(&format!("/api/cards/{archived}/archive"), json!({})).await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "mock", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 201, "{body}");
    assert_eq!(body["target_count"], 2);
}

#[tokio::test]
async fn a_practice_session_still_has_no_target_count() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 201, "{body}");
    assert!(body["target_count"].is_null(), "practice must not gain a target: {body}");
}

#[tokio::test]
async fn rejects_a_client_supplied_target_count_on_a_mock_session() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;

    let (status, body) = app
        .post(
            "/api/sessions",
            json!({ "mode": "mock", "deck_ids": [deck_id], "target_count": 5 }),
        )
        .await;

    assert_eq!(status, 422, "{body}");
    assert!(
        field_errors(&body).contains(&(
            "target_count".to_string(),
            "A mock test is the whole deck, so its length is not yours to set".to_string()
        )),
        "mock must get its own target_count message: {body}",
    );
    assert_eq!(app.count("SELECT COUNT(*) FROM sessions").await, 0);
}

#[tokio::test]
async fn refuses_to_create_a_mock_session_for_an_empty_deck() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "empty", None).await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "mock", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 422, "{body}");
    assert_eq!(app.count("SELECT COUNT(*) FROM sessions").await, 0);
}

#[tokio::test]
async fn refuses_a_mock_session_whose_only_card_is_archived() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let only_card = create_flashcard(&app, deck_id, "one", "an answer").await;
    app.post(&format!("/api/cards/{only_card}/archive"), json!({})).await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "mock", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 422, "{body}");
    assert_eq!(app.count("SELECT COUNT(*) FROM sessions").await, 0);
}

#[tokio::test]
async fn a_mock_session_accepts_a_module_wide_pool() {
    let app = common::spawn_app().await;
    let module_id = create_module(&app, "COS781").await;
    let first_deck = create_deck(&app, "clustering", Some(module_id)).await;
    let second_deck = create_deck(&app, "classification", Some(module_id)).await;
    create_flashcard(&app, first_deck, "one", "an answer").await;
    create_flashcard(&app, second_deck, "two", "an answer").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "mock", "module_id": module_id }))
        .await;

    assert_eq!(status, 201, "the API keeps its module capability: {body}");
    assert_eq!(body["target_count"], 2);
    assert_eq!(body["deck_ids"], json!([first_deck, second_deck]));
}

#[tokio::test]
async fn a_mock_session_accepts_several_decks() {
    let app = common::spawn_app().await;
    let first_deck = create_deck(&app, "clustering", None).await;
    let second_deck = create_deck(&app, "classification", None).await;
    create_flashcard(&app, first_deck, "one", "an answer").await;
    create_flashcard(&app, second_deck, "two", "an answer").await;

    let (status, body) = app
        .post(
            "/api/sessions",
            json!({ "mode": "mock", "deck_ids": [second_deck, first_deck] }),
        )
        .await;

    assert_eq!(status, 201, "{body}");
    assert_eq!(body["target_count"], 2);
}

#[tokio::test]
async fn an_unknown_mode_is_still_distinguished_from_a_not_yet_mode() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "cram", "deck_ids": [deck_id] }))
        .await;

    assert_eq!(status, 422);
    assert!(
        field_errors(&body).contains(&(
            "mode".to_string(),
            "mode must be practice, mock or sm2".to_string()
        )),
        "{body}",
    );
}

async fn record_review(app: &common::TestApp, session_id: i64, card_id: i64, correct: i64) {
    sqlx::query("INSERT INTO reviews (card_id, session_id, correct) VALUES (?, ?, ?)")
        .bind(card_id)
        .bind(session_id)
        .bind(correct)
        .execute(&app.pool)
        .await
        .unwrap();
}

async fn serve(app: &common::TestApp, session_id: i64) -> (u16, Value) {
    let (status, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    (status.as_u16(), body)
}

#[tokio::test]
async fn a_mock_serve_carries_the_mode_and_no_running_score() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (status, body) = serve(&app, session_id).await;
    assert_eq!(status, 200, "{body}");

    let mut keys: Vec<&str> = body.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["answered_count", "card", "mode", "pool_count", "started_at", "target_count"],
        "a mock serve must carry no running score: {body}",
    );
    assert_eq!(body["mode"], "mock");
    assert_eq!(body["target_count"], 1);
    assert!(body["started_at"].as_str().unwrap().ends_with('Z'), "{body}");
    assert!(body["correct_count"].is_null(), "correct_count leaks the verdict: {body}");
}

#[tokio::test]
async fn a_mock_serve_is_identical_across_reloads() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    for index in 0..6 {
        create_multiple_choice(&app, deck_id, &format!("question {index}")).await;
    }
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, first) = serve(&app, session_id).await;
    for attempt in 0..20 {
        let (_, again) = serve(&app, session_id).await;
        assert_eq!(again, first, "serve {attempt} differed from the first");
    }
}

#[tokio::test]
async fn a_mock_serve_keeps_its_choice_order_across_reloads() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_multiple_choice(&app, deck_id, "which linkage").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, first) = serve(&app, session_id).await;
    let first_order = first["card"]["choices"].clone();
    for _ in 0..20 {
        let (_, again) = serve(&app, session_id).await;
        assert_eq!(again["card"]["choices"], first_order, "choice order moved on reload");
    }
}

#[tokio::test]
async fn a_mock_test_serves_every_card_exactly_once() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let mut created = Vec::new();
    for index in 0..12 {
        created.push(create_flashcard(&app, deck_id, &format!("q{index}"), "an answer").await);
    }
    created.sort_unstable();
    let session_id = start_mock_session(&app, deck_id).await;

    let mut served = Vec::new();
    for _ in 0..12 {
        let (status, body) = serve(&app, session_id).await;
        assert_eq!(status, 200, "ran out early after {} cards: {body}", served.len());
        let card_id = body["card"]["id"].as_i64().unwrap();
        served.push(card_id);
        record_review(&app, session_id, card_id, 1).await;
    }

    let (status, body) = serve(&app, session_id).await;
    assert_eq!(status, 409, "the pool should be exhausted: {body}");

    let mut sorted = served.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, created, "every card exactly once");
}

#[tokio::test]
async fn a_mock_pool_count_does_not_shrink_as_cards_are_answered() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    for index in 0..5 {
        create_flashcard(&app, deck_id, &format!("q{index}"), "an answer").await;
    }
    let session_id = start_mock_session(&app, deck_id).await;

    for expected_answered in 0..5 {
        let (status, body) = serve(&app, session_id).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["pool_count"], 5, "pool_count must stay the whole pool: {body}");
        assert_eq!(body["answered_count"], expected_answered, "{body}");
        let card_id = body["card"]["id"].as_i64().unwrap();
        record_review(&app, session_id, card_id, 1).await;
    }
}

#[tokio::test]
async fn archiving_a_card_mid_mock_ends_the_run_early() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    for index in 0..4 {
        create_flashcard(&app, deck_id, &format!("q{index}"), "an answer").await;
    }
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, first) = serve(&app, session_id).await;
    let first_card = first["card"]["id"].as_i64().unwrap();
    record_review(&app, session_id, first_card, 1).await;

    let (_, second) = serve(&app, session_id).await;
    let doomed = second["card"]["id"].as_i64().unwrap();
    app.post(&format!("/api/cards/{doomed}/archive"), json!({})).await;

    let mut answered = 1;
    loop {
        let (status, body) = serve(&app, session_id).await;
        if status == 409 {
            break;
        }
        assert_eq!(status, 200, "{body}");
        let card_id = body["card"]["id"].as_i64().unwrap();
        assert_ne!(card_id, doomed, "an archived card was served");
        record_review(&app, session_id, card_id, 1).await;
        answered += 1;
        assert!(answered <= 4, "the run did not terminate");
    }

    assert_eq!(answered, 3, "a four-card mock with one archived should end at three");
}

#[tokio::test]
async fn archiving_a_card_mid_mock_does_not_reorder_the_remaining_cards() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let mut created = Vec::new();
    for index in 0..8 {
        created.push(create_flashcard(&app, deck_id, &format!("q{index}"), "an answer").await);
    }
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, first) = serve(&app, session_id).await;
    let head = first["card"]["id"].as_i64().unwrap();
    record_review(&app, session_id, head, 1).await;

    let (_, second) = serve(&app, session_id).await;
    let next_up = second["card"]["id"].as_i64().unwrap();

    let survivor = created
        .iter()
        .copied()
        .find(|card_id| *card_id != head && *card_id != next_up)
        .unwrap();
    app.post(&format!("/api/cards/{survivor}/archive"), json!({})).await;

    let (status, after) = serve(&app, session_id).await;
    assert_eq!(status, 200);
    assert_eq!(
        after["card"]["id"].as_i64().unwrap(),
        next_up,
        "archiving an unrelated card must not change whose turn it is",
    );
}

#[tokio::test]
async fn two_mock_sessions_on_the_same_deck_get_different_orders() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    for index in 0..10 {
        create_flashcard(&app, deck_id, &format!("q{index}"), "an answer").await;
    }

    let mut heads = Vec::new();
    for _ in 0..6 {
        let session_id = start_mock_session(&app, deck_id).await;
        let (_, body) = serve(&app, session_id).await;
        heads.push(body["card"]["id"].as_i64().unwrap());
    }

    let distinct: std::collections::HashSet<i64> = heads.iter().copied().collect();
    assert!(distinct.len() > 1, "every session opened on the same card: {heads:?}");
}

#[tokio::test]
async fn a_mock_serve_never_returns_answer_content_for_any_kind() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "flash", "the secret answer").await;
    create_multiple_choice(&app, deck_id, "choice").await;
    create_text_answer(&app, deck_id, "short").await;
    create_flashcard(&app, deck_id, "listy", "- alpha ultrasecret\n- beta ultrasecret").await;
    let session_id = start_mock_session(&app, deck_id).await;

    for _ in 0..4 {
        let (status, body) = serve(&app, session_id).await;
        assert_eq!(status, 200, "{body}");

        let mut card_keys: Vec<&str> =
            body["card"].as_object().unwrap().keys().map(String::as_str).collect();
        card_keys.sort_unstable();
        assert_eq!(
            card_keys,
            vec!["answer_points", "choices", "id", "image_path", "kind", "prompt_md"],
            "{body}",
        );

        let serialised = body.to_string();
        for forbidden in [
            "is_correct",
            "answer_md",
            "explanation_md",
            "accepted",
            "expected",
            "correct",
            "the secret answer",
            "lloyd",
            "ultrasecret",
        ] {
            assert!(!serialised.contains(forbidden), "{forbidden} leaked: {serialised}");
        }

        if !body["card"]["answer_points"].is_null() {
            let cues = &body["card"]["answer_points"]["cues"];
            assert!(
                cues["behind_the_hint"].as_array().unwrap().is_empty(),
                "a card the learner has never seen must not be hiding a further hint: {body}",
            );
            assert_eq!(
                body["card"]["answer_points"]["total"].as_i64().unwrap(),
                2,
                "the point count is the only thing a serve may say about the list: {body}",
            );
        }

        let card_id = body["card"]["id"].as_i64().unwrap();
        record_review(&app, session_id, card_id, 1).await;
    }
}

#[tokio::test]
async fn a_finished_mock_session_conflicts_on_next() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    app.post(&format!("/api/sessions/{session_id}/finish"), json!({})).await;

    let (status, body) = serve(&app, session_id).await;
    assert_eq!(status, 409, "{body}");
}

#[tokio::test]
async fn a_practice_serve_still_carries_its_running_score() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_practice_session(&app, deck_id).await;

    let (status, body) = serve(&app, session_id).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["mode"], "practice");
    assert_eq!(body["correct_count"], 0, "practice keeps its score: {body}");
}

#[tokio::test]
async fn a_mock_session_refuses_to_reveal_a_flashcard() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy", "the secret answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/reveal"),
            json!({ "card_id": card_id }),
        )
        .await;

    assert_eq!(status, 409, "{body}");
    assert_eq!(body["message"], "A mock test does not reveal answers");

    let serialised = body.to_string();
    for forbidden in ["the secret answer", "answer_md", "explanation_md"] {
        assert!(!serialised.contains(forbidden), "{forbidden} leaked: {serialised}");
    }
}

#[tokio::test]
async fn a_mock_reveal_refusal_does_not_disclose_the_card_kind() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let flashcard = create_flashcard(&app, deck_id, "flash", "an answer").await;
    let choice_card = create_multiple_choice(&app, deck_id, "choice").await;
    let short_card = create_text_answer(&app, deck_id, "short").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let mut responses = Vec::new();
    for card_id in [flashcard, choice_card, short_card] {
        let (status, body) = app
            .post(
                &format!("/api/sessions/{session_id}/reveal"),
                json!({ "card_id": card_id }),
            )
            .await;
        responses.push((status.as_u16(), body["message"].as_str().unwrap().to_string()));
    }

    assert_eq!(
        responses[0], responses[1],
        "a flashcard and a multiple-choice card must refuse identically",
    );
    assert_eq!(
        responses[0], responses[2],
        "a flashcard and a text-answer card must refuse identically",
    );
}

#[tokio::test]
async fn a_refused_mock_reveal_writes_nothing() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    app.post(
        &format!("/api/sessions/{session_id}/reveal"),
        json!({ "card_id": card_id }),
    )
    .await;

    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn a_practice_session_still_reveals_a_flashcard() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy", "a measure of disorder").await;
    let session_id = start_practice_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/reveal"),
            json!({ "card_id": card_id }),
        )
        .await;

    assert_eq!(status, 200, "{body}");
    assert_eq!(body["answer_md"], "a measure of disorder");
}

async fn answer_typed(
    app: &common::TestApp,
    session_id: i64,
    card_id: i64,
    given: &str,
) -> (u16, Value) {
    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": given }),
        )
        .await;
    (status.as_u16(), body)
}

async fn correctness_of(app: &common::TestApp, card_id: i64) -> i64 {
    app.count(&format!("SELECT correct FROM reviews WHERE card_id = {card_id}")).await
}

#[tokio::test]
async fn a_mock_flashcard_is_typed_and_auto_graded() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy", "a measure of disorder").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (status, body) = answer_typed(&app, session_id, card_id, "A Measure Of Disorder!").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(correctness_of(&app, card_id).await, 1, "an exact answer must grade correct");
}

#[tokio::test]
async fn a_mock_flashcard_grades_a_wrong_answer_wrong() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy", "a measure of disorder").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (status, _) = answer_typed(&app, session_id, card_id, "bananas").await;
    assert_eq!(status, 200);
    assert_eq!(correctness_of(&app, card_id).await, 0);
}

#[tokio::test]
async fn a_mock_flashcard_absorbs_a_small_typo() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "name it", "hierarchical clustering").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (status, _) = answer_typed(&app, session_id, card_id, "hierarchical clusterng").await;
    assert_eq!(status, 200);
    assert_eq!(correctness_of(&app, card_id).await, 1, "one typo should be forgiven");
}

#[tokio::test]
async fn a_mock_flashcard_rejects_a_self_grade() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "good" }),
        )
        .await;

    assert_eq!(status, 422, "{body}");
    assert!(
        field_errors(&body).contains(&(
            "self_grade".to_string(),
            "A mock test grades flashcards automatically".to_string()
        )),
        "{body}",
    );
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn a_mock_flashcard_requires_typed_text() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id }),
        )
        .await;
    assert_eq!(status, 422, "{body}");
    assert!(
        field_errors(&body)
            .contains(&("given".to_string(), "This field is required".to_string())),
        "{body}",
    );

    let (status, body) = answer_typed(&app, session_id, card_id, "   ").await;
    assert_eq!(status, 422, "{body}");
    assert!(
        field_errors(&body).contains(&("given".to_string(), "Type an answer".to_string())),
        "{body}",
    );

    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn a_mock_flashcard_stores_the_wording_and_no_self_grade() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    answer_typed(&app, session_id, card_id, "  K-MEANS!  ").await;

    let stored: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT given, self_grade FROM reviews WHERE card_id = ?")
            .bind(card_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(stored.0.as_deref(), Some("K-MEANS!"), "the raw trimmed wording is stored");
    assert!(stored.1.is_none(), "a mock flashcard has no self_grade");
}

#[tokio::test]
async fn a_mock_multiple_choice_still_rejects_typed_text() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_multiple_choice(&app, deck_id, "which linkage").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (status, body) = answer_typed(&app, session_id, card_id, "complete linkage").await;
    assert_eq!(status, 422, "{body}");
    assert!(
        field_errors(&body).contains(&(
            "given".to_string(),
            "Only a text-answer or flashcard takes typed text".to_string()
        )),
        "{body}",
    );
}

#[tokio::test]
async fn a_mock_answer_response_carries_no_verdict() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let flashcard = create_flashcard(&app, deck_id, "flash", "the secret answer").await;
    let choice_card = create_multiple_choice(&app, deck_id, "choice").await;
    let short_card = create_text_answer(&app, deck_id, "short").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, choices) = app.get(&format!("/api/cards/{choice_card}")).await;
    let choice_id = choices["choices"][0]["id"].as_i64().unwrap();

    let bodies = vec![
        answer_typed(&app, session_id, flashcard, "something wrong").await.1,
        app.post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": choice_card, "choice_id": choice_id }),
        )
        .await
        .1,
        answer_typed(&app, session_id, short_card, "k-means").await.1,
    ];

    for body in &bodies {
        let mut keys: Vec<&str> = body.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["answered_count", "mode", "pool_count"],
            "a mock answer must carry no verdict: {body}",
        );

        let serialised = body.to_string();
        for forbidden in [
            "correct",
            "expected",
            "explanation_md",
            "can_override",
            "review_id",
            "the secret answer",
            "complete linkage",
            "lloyd",
        ] {
            assert!(!serialised.contains(forbidden), "{forbidden} leaked: {serialised}");
        }
    }
}

#[tokio::test]
async fn a_mock_card_cannot_be_answered_twice() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    create_flashcard(&app, deck_id, "two", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (first, _) = answer_typed(&app, session_id, card_id, "an answer").await;
    assert_eq!(first, 200);

    let (second, body) = answer_typed(&app, session_id, card_id, "an answer").await;
    assert_eq!(second, 409, "{body}");
    assert_eq!(body["message"], "That card has already been answered in this mock test");
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 1);
}

#[tokio::test]
async fn a_practice_card_can_still_be_answered_twice() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_practice_session(&app, deck_id).await;

    for _ in 0..2 {
        let (status, body) = answer_typed(&app, session_id, card_id, "k-means").await;
        assert_eq!(status, 200, "practice must keep repeating cards: {body}");
    }
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 2);
}

#[tokio::test]
async fn a_practice_flashcard_is_still_self_graded() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_practice_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "hard" }),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["correct"], true);

    let stored: Option<String> =
        sqlx::query_scalar("SELECT self_grade FROM reviews WHERE card_id = ?")
            .bind(card_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(stored.as_deref(), Some("hard"), "part 7 depends on this column");
}

#[tokio::test]
async fn a_practice_flashcard_still_rejects_typed_text() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_practice_session(&app, deck_id).await;

    let (status, body) = answer_typed(&app, session_id, card_id, "an answer").await;
    assert_eq!(status, 422, "{body}");
    assert!(
        field_errors(&body).contains(&(
            "given".to_string(),
            "Only a text-answer card takes typed text".to_string()
        )),
        "practice keeps its own wording: {body}",
    );
}

#[tokio::test]
async fn answering_a_mock_card_does_not_touch_the_schedule_table() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let before = app.schedule_state_for(card_id).await;
    answer_typed(&app, session_id, card_id, "an answer").await;
    let after = app.schedule_state_for(card_id).await;

    assert_eq!(before, after, "mock mode must leave the sm-2 schedule alone");
}

async fn submit(app: &common::TestApp, session_id: i64) {
    let (status, body) = app.post(&format!("/api/sessions/{session_id}/finish"), json!({})).await;
    assert_eq!(status, 200, "could not submit: {body}");
}

async fn results(app: &common::TestApp, session_id: i64) -> (u16, Value) {
    let (status, body) = app.get(&format!("/api/sessions/{session_id}/results")).await;
    (status.as_u16(), body)
}

#[tokio::test]
async fn results_are_refused_while_the_session_is_active() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let flashcard = create_flashcard(&app, deck_id, "flash", "the secret answer").await;
    let choice_card = create_multiple_choice(&app, deck_id, "choice").await;
    let short_card = create_text_answer(&app, deck_id, "short").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, choices) = app.get(&format!("/api/cards/{choice_card}")).await;
    let choice_id = choices["choices"][0]["id"].as_i64().unwrap();
    answer_typed(&app, session_id, flashcard, "wrong").await;
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": choice_card, "choice_id": choice_id }),
    )
    .await;
    answer_typed(&app, session_id, short_card, "nonsense").await;

    let (status, body) = results(&app, session_id).await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["message"], "This session has not been submitted yet");

    let serialised = body.to_string();
    for forbidden in [
        "the secret answer",
        "complete linkage",
        "lloyd",
        "k-means",
        "expected",
        "answer_md",
        "explanation_md",
        "questions",
    ] {
        assert!(!serialised.contains(forbidden), "{forbidden} leaked: {serialised}");
    }
}

#[tokio::test]
async fn results_on_an_active_practice_session_are_also_refused() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_practice_session(&app, deck_id).await;

    let (status, body) = results(&app, session_id).await;
    assert_eq!(status, 409, "one rule for both modes: {body}");
}

#[tokio::test]
async fn results_on_an_unknown_session_are_not_found() {
    let app = common::spawn_app().await;
    let (status, _) = results(&app, 9999).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn results_list_every_question_in_answer_order() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    for index in 0..12 {
        create_flashcard(&app, deck_id, &format!("q{index}"), "an answer").await;
    }
    let session_id = start_mock_session(&app, deck_id).await;

    let mut served = Vec::new();
    for _ in 0..12 {
        let (_, body) = serve(&app, session_id).await;
        let card_id = body["card"]["id"].as_i64().unwrap();
        served.push(card_id);
        answer_typed(&app, session_id, card_id, "an answer").await;
    }
    submit(&app, session_id).await;

    let (status, body) = results(&app, session_id).await;
    assert_eq!(status, 200, "{body}");

    let listed: Vec<i64> = body["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|question| question["card_id"].as_i64().unwrap())
        .collect();
    assert_eq!(listed, served, "results must be in answer order");
}

#[tokio::test]
async fn results_keep_answer_order_when_timestamps_tie() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let first = create_flashcard(&app, deck_id, "one", "an answer").await;
    let second = create_flashcard(&app, deck_id, "two", "an answer").await;
    let third = create_flashcard(&app, deck_id, "three", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    for card_id in [third, first, second] {
        sqlx::query(
            "INSERT INTO reviews (card_id, session_id, correct, answered_at)
             VALUES (?, ?, 1, '2026-08-28T10:00:00Z')",
        )
        .bind(card_id)
        .bind(session_id)
        .execute(&app.pool)
        .await
        .unwrap();
    }
    submit(&app, session_id).await;

    let (_, body) = results(&app, session_id).await;
    let listed: Vec<i64> = body["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|question| question["card_id"].as_i64().unwrap())
        .collect();
    assert_eq!(
        listed,
        vec![third, first, second],
        "with identical timestamps the review id must break the tie in insertion order",
    );
}

#[tokio::test]
async fn results_carry_the_expected_answer_for_each_kind() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let flashcard = create_flashcard(&app, deck_id, "flash", "a measure of disorder").await;
    let choice_card = create_multiple_choice(&app, deck_id, "choice").await;
    let short_card = create_text_answer(&app, deck_id, "short").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, choices) = app.get(&format!("/api/cards/{choice_card}")).await;
    let choice_id = choices["choices"][0]["id"].as_i64().unwrap();
    answer_typed(&app, session_id, flashcard, "wrong").await;
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": choice_card, "choice_id": choice_id }),
    )
    .await;
    answer_typed(&app, session_id, short_card, "nonsense").await;
    submit(&app, session_id).await;

    let (_, body) = results(&app, session_id).await;
    let questions = body["questions"].as_array().unwrap();

    let by_kind = |kind: &str| -> Value {
        questions
            .iter()
            .find(|question| question["kind"] == kind)
            .unwrap()
            .clone()
    };

    assert_eq!(by_kind("flashcard")["expected"], json!(["a measure of disorder"]));
    assert_eq!(by_kind("mc_single")["expected"], json!(["complete linkage"]));
    assert_eq!(
        by_kind("text_answer")["expected"],
        json!(["k-means", "lloyd's algorithm"]),
        "accepted wordings, primary first",
    );

    let flashcard_question = by_kind("flashcard");
    assert_eq!(flashcard_question["prompt_md"], "flash");
    assert_eq!(flashcard_question["given"], "wrong");
    assert_eq!(flashcard_question["correct"], false);
}

#[tokio::test]
async fn results_report_correctness_per_question() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let right = create_flashcard(&app, deck_id, "right", "an answer").await;
    let wrong = create_flashcard(&app, deck_id, "wrong", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    answer_typed(&app, session_id, right, "an answer").await;
    answer_typed(&app, session_id, wrong, "bananas").await;
    submit(&app, session_id).await;

    let (_, body) = results(&app, session_id).await;
    let questions = body["questions"].as_array().unwrap();
    let verdict_of = |card_id: i64| -> bool {
        questions
            .iter()
            .find(|question| question["card_id"] == card_id)
            .unwrap()["correct"]
            .as_bool()
            .unwrap()
    };
    assert!(verdict_of(right));
    assert!(!verdict_of(wrong));

    assert_eq!(body["summary"]["answered_count"], 2);
    assert_eq!(body["summary"]["correct_count"], 1);
}

#[tokio::test]
async fn results_survive_a_reload() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;
    answer_typed(&app, session_id, card_id, "an answer").await;
    submit(&app, session_id).await;

    let (_, first) = results(&app, session_id).await;
    let (_, again) = results(&app, session_id).await;
    assert_eq!(first, again, "a results reload must be byte-identical");
}

#[tokio::test]
async fn results_include_a_practice_flashcards_self_grade() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_practice_session(&app, deck_id).await;

    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "self_grade": "hard" }),
    )
    .await;
    submit(&app, session_id).await;

    let (_, body) = results(&app, session_id).await;
    let question = &body["questions"][0];
    assert_eq!(question["self_grade"], "hard");
    assert!(question["given"].is_null(), "a self-graded flashcard stores no typed text");
}

#[tokio::test]
async fn results_can_override_is_true_only_where_the_override_applies() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let flashcard = create_flashcard(&app, deck_id, "flash", "a measure of disorder").await;
    let choice_card = create_multiple_choice(&app, deck_id, "choice").await;
    let short_card = create_text_answer(&app, deck_id, "short").await;
    let right_card = create_flashcard(&app, deck_id, "right", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, choices) = app.get(&format!("/api/cards/{choice_card}")).await;
    let wrong_choice = choices["choices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|choice| choice["is_correct"] == false)
        .unwrap()["id"]
        .as_i64()
        .unwrap();

    answer_typed(&app, session_id, flashcard, "bananas").await;
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": choice_card, "choice_id": wrong_choice }),
    )
    .await;
    answer_typed(&app, session_id, short_card, "nonsense").await;
    answer_typed(&app, session_id, right_card, "an answer").await;
    submit(&app, session_id).await;

    let (_, body) = results(&app, session_id).await;
    let questions = body["questions"].as_array().unwrap();
    let can_override_of = |card_id: i64| -> bool {
        questions
            .iter()
            .find(|question| question["card_id"] == card_id)
            .unwrap()["can_override"]
            .as_bool()
            .unwrap()
    };

    assert!(can_override_of(flashcard), "a wrong mock flashcard is overridable");
    assert!(can_override_of(short_card), "a wrong text answer is overridable");
    assert!(!can_override_of(choice_card), "a multiple-choice answer is never overridable");
    assert!(!can_override_of(right_card), "a correct answer is not overridable");
}

#[tokio::test]
async fn results_of_a_session_with_no_answers_are_empty_with_a_null_accuracy() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "one", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;
    submit(&app, session_id).await;

    let (status, body) = results(&app, session_id).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["questions"].as_array().unwrap().len(), 0);
    assert!(body["summary"]["accuracy"].is_null());
}

#[tokio::test]
async fn results_put_the_primary_accepted_wording_first_even_when_it_was_added_last() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "text_answer",
                "prompt_md": "name the algorithm",
                "accepted": [
                    { "text": "lloyd's algorithm", "is_primary": false },
                    { "text": "k-means", "is_primary": true },
                ],
            }),
        )
        .await;
    let card_id = card["id"].as_i64().unwrap();
    let session_id = start_mock_session(&app, deck_id).await;

    answer_typed(&app, session_id, card_id, "nonsense").await;
    submit(&app, session_id).await;

    let (_, body) = results(&app, session_id).await;
    assert_eq!(
        body["questions"][0]["expected"],
        json!(["k-means", "lloyd's algorithm"]),
        "the primary wording must lead even though it has the higher id",
    );
}

async fn review_id_for(app: &common::TestApp, card_id: i64) -> i64 {
    app.count(&format!("SELECT id FROM reviews WHERE card_id = {card_id}")).await
}

async fn override_review(app: &common::TestApp, review_id: i64) -> (u16, Value) {
    let (status, body) = app.post(&format!("/api/reviews/{review_id}/override"), json!({})).await;
    (status.as_u16(), body)
}

#[tokio::test]
async fn a_wrong_mock_flashcard_can_be_overridden_after_submitting() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "flash", "a measure of disorder").await;
    let session_id = start_mock_session(&app, deck_id).await;

    answer_typed(&app, session_id, card_id, "disorder in a set, roughly").await;
    submit(&app, session_id).await;
    let review_id = review_id_for(&app, card_id).await;

    let (status, body) = override_review(&app, review_id).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["correct"], true);
    assert_eq!(body["overridden"], true);
    assert_eq!(body["accepted_added"], false, "a flashcard has no accepted list");
    assert_eq!(body["expected"], json!(["a measure of disorder"]));

    assert_eq!(correctness_of(&app, card_id).await, 1);
    assert_eq!(
        app.count("SELECT COUNT(*) FROM reviews WHERE overridden = 1").await,
        1,
    );
}

#[tokio::test]
async fn overriding_a_mock_flashcard_adds_no_accepted_row() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "flash", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    answer_typed(&app, session_id, card_id, "bananas").await;
    submit(&app, session_id).await;
    let before = app.count("SELECT COUNT(*) FROM accepted").await;

    override_review(&app, review_id_for(&app, card_id).await).await;

    assert_eq!(app.count("SELECT COUNT(*) FROM accepted").await, before);
    assert_eq!(before, 0, "a flashcard deck has no accepted rows to begin with");
}

#[tokio::test]
async fn overriding_is_refused_while_a_mock_test_is_unsubmitted() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let flashcard = create_flashcard(&app, deck_id, "flash", "the secret answer").await;
    let short_card = create_text_answer(&app, deck_id, "short").await;
    let right_card = create_flashcard(&app, deck_id, "right", "an answer").await;
    let choice_card = create_multiple_choice(&app, deck_id, "choice").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, card) = app.get(&format!("/api/cards/{choice_card}")).await;
    let wrong_choice = card["choices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|choice| choice["is_correct"] == false)
        .unwrap()["id"]
        .as_i64()
        .unwrap();

    answer_typed(&app, session_id, flashcard, "bananas").await;
    answer_typed(&app, session_id, short_card, "nonsense").await;
    answer_typed(&app, session_id, right_card, "an answer").await;
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": choice_card, "choice_id": wrong_choice }),
    )
    .await;

    let mut refusals = Vec::new();
    for card_id in [flashcard, short_card, right_card, choice_card] {
        let (status, body) = override_review(&app, review_id_for(&app, card_id).await).await;
        assert_eq!(status, 409, "a live mock must refuse: {body}");

        let serialised = body.to_string();
        for forbidden in ["the secret answer", "expected", "k-means", "lloyd"] {
            assert!(!serialised.contains(forbidden), "{forbidden} leaked: {serialised}");
        }
        refusals.push(body["message"].as_str().unwrap().to_string());
    }

    assert_eq!(
        refusals[0], refusals[1],
        "a flashcard and a text answer must refuse identically during a live mock",
    );
    assert_eq!(
        refusals[0], refusals[2],
        "a wrong and an already-correct answer must refuse identically during a live mock",
    );
    assert_eq!(
        refusals[0], refusals[3],
        "a multiple-choice answer must refuse identically too, or the message leaks the kind",
    );
    assert_eq!(refusals[0], "Submit the mock test before overriding an answer");
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews WHERE overridden = 1").await, 0);
}

#[tokio::test]
async fn a_practice_flashcard_still_cannot_be_overridden() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "flash", "an answer").await;
    let session_id = start_practice_session(&app, deck_id).await;

    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "self_grade": "again" }),
    )
    .await;

    let (status, body) = override_review(&app, review_id_for(&app, card_id).await).await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["message"], "Grade the flashcard again instead of overriding it");
}

#[tokio::test]
async fn a_mock_multiple_choice_still_cannot_be_overridden() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_multiple_choice(&app, deck_id, "which linkage").await;
    let session_id = start_mock_session(&app, deck_id).await;

    let (_, card) = app.get(&format!("/api/cards/{card_id}")).await;
    let wrong_choice = card["choices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|choice| choice["is_correct"] == false)
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "choice_id": wrong_choice }),
    )
    .await;
    submit(&app, session_id).await;

    let (status, body) = override_review(&app, review_id_for(&app, card_id).await).await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["message"], "A multiple-choice answer cannot be overridden");
}

#[tokio::test]
async fn overriding_a_mock_text_answer_still_teaches_the_card() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_mock_session(&app, deck_id).await;

    answer_typed(&app, session_id, card_id, "Lloyd Algorithm").await;
    submit(&app, session_id).await;

    let (status, body) = override_review(&app, review_id_for(&app, card_id).await).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["accepted_added"], true, "a text answer teaches the card");
    assert!(
        body["expected"].as_array().unwrap().len() == 3,
        "the new wording joins the accepted list: {body}",
    );

    let second_session = start_mock_session(&app, deck_id).await;
    answer_typed(&app, second_session, card_id, "  lloyd  algorithm!  ").await;
    let graded: i64 = sqlx::query_scalar(
        "SELECT correct FROM reviews WHERE session_id = ? AND card_id = ?",
    )
    .bind(second_session)
    .bind(card_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(graded, 1, "the accepted wording must grade correct next time");
}

#[tokio::test]
async fn the_mock_results_count_an_override_as_correct() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "flash", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    answer_typed(&app, session_id, card_id, "bananas").await;
    submit(&app, session_id).await;

    let (_, before) = results(&app, session_id).await;
    assert_eq!(before["summary"]["correct_count"], 0);

    override_review(&app, review_id_for(&app, card_id).await).await;

    let (_, after) = results(&app, session_id).await;
    assert_eq!(after["summary"]["correct_count"], 1);
    assert_eq!(after["summary"]["overridden_count"], 1);
    assert_eq!(after["questions"][0]["correct"], true);
    assert_eq!(after["questions"][0]["overridden"], true);
    assert_eq!(
        after["questions"][0]["can_override"], false,
        "an overridden question offers no second override",
    );
}

#[tokio::test]
async fn a_mock_flashcard_with_no_typed_text_cannot_be_overridden() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    let card_id = create_flashcard(&app, deck_id, "flash", "an answer").await;
    let session_id = start_mock_session(&app, deck_id).await;

    sqlx::query("INSERT INTO reviews (card_id, session_id, given, correct) VALUES (?, ?, '!!!', 0)")
        .bind(card_id)
        .bind(session_id)
        .execute(&app.pool)
        .await
        .unwrap();
    submit(&app, session_id).await;

    let (status, body) = override_review(&app, review_id_for(&app, card_id).await).await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["message"], "There is no answer to accept");
}
