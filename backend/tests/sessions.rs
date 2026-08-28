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

async fn create_multiple_choice(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "mc_single",
                "prompt_md": prompt,
                "explanation_md": "because the centroid moves",
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
                "explanation_md": "an explanation",
                "accepted": [
                    { "text": "k-means", "is_primary": true },
                    { "text": "lloyd's algorithm", "is_primary": false },
                ],
            }),
        )
        .await;
    card["id"].as_i64().unwrap()
}

async fn start_session(app: &common::TestApp, deck_id: i64) -> i64 {
    let (status, session) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(status, 201, "session creation failed: {session}");
    session["id"].as_i64().unwrap()
}

async fn answer_card(app: &common::TestApp, session_id: i64, card_id: i64) {
    sqlx::query("INSERT INTO reviews (card_id, session_id, correct) VALUES (?, ?, 1)")
        .bind(card_id)
        .bind(session_id)
        .execute(&app.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn next_never_returns_answer_data_for_any_kind() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "all kinds", None).await;
    create_multiple_choice(&app, deck_id, "which linkage").await;
    create_short_answer(&app, deck_id, "name the algorithm").await;
    create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    let mut kinds_seen: Vec<String> = Vec::new();

    for _ in 0..30 {
        let (status, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        assert_eq!(status, 200, "body was {body}");

        let card = body["card"].as_object().unwrap();
        let mut keys: Vec<&str> = card.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["choices", "id", "image_path", "kind", "prompt_md"],
            "the served card must carry exactly these keys and no others",
        );

        for choice in body["card"]["choices"].as_array().unwrap() {
            let mut choice_keys: Vec<&str> =
                choice.as_object().unwrap().keys().map(String::as_str).collect();
            choice_keys.sort_unstable();
            assert_eq!(choice_keys, vec!["id", "text_md"]);
        }

        let serialised = body.to_string();
        for forbidden in
            ["is_correct", "answer_md", "explanation_md", "accepted", "expected", "correct"]
        {
            assert!(
                !serialised.contains(forbidden),
                "a served card leaked {forbidden}: {serialised}",
            );
        }
        for answer_text in
            ["because the centroid moves", "k-means", "lloyd's algorithm", "an answer"]
        {
            assert!(
                !serialised.contains(answer_text),
                "a served card leaked the answer text {answer_text}: {serialised}",
            );
        }

        let kind = body["card"]["kind"].as_str().unwrap().to_string();
        if !kinds_seen.contains(&kind) {
            kinds_seen.push(kind);
        }
    }

    assert_eq!(kinds_seen.len(), 3, "all three kinds should have been served across 30 serves");
}

#[tokio::test]
async fn next_shuffles_the_choices() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "one card", None).await;
    create_multiple_choice(&app, deck_id, "which linkage").await;
    let session_id = start_session(&app, deck_id).await;

    let mut orders: Vec<Vec<i64>> = Vec::new();
    let mut identifier_sets: Vec<Vec<i64>> = Vec::new();

    for _ in 0..30 {
        let (_, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        let order: Vec<i64> = body["card"]["choices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|choice| choice["id"].as_i64().unwrap())
            .collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        if !identifier_sets.contains(&sorted) {
            identifier_sets.push(sorted);
        }
        if !orders.contains(&order) {
            orders.push(order);
        }
    }

    assert!(orders.len() >= 2, "choices must be shuffled per serve, saw only {orders:?}");
    assert_eq!(
        identifier_sets.len(),
        1,
        "shuffling must reorder the same choices, never change the set",
    );
}

#[tokio::test]
async fn next_serves_an_unseen_card_before_well_known_ones() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mostly known", None).await;
    let session_id_holder = create_flashcard(&app, deck_id, "seed").await;
    let mut known: Vec<i64> = vec![session_id_holder];
    for index in 0..4 {
        known.push(create_flashcard(&app, deck_id, &format!("known {index}")).await);
    }
    let unseen = create_flashcard(&app, deck_id, "never seen").await;
    let session_id = start_session(&app, deck_id).await;

    for card_id in &known {
        for _ in 0..3 {
            answer_card(&app, session_id, *card_id).await;
        }
    }

    let mut served_unseen = false;
    for _ in 0..20 {
        let (_, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        if body["card"]["id"].as_i64() == Some(unseen) {
            served_unseen = true;
            break;
        }
    }
    assert!(served_unseen, "a never-seen card must surface against well-known ones");
}

#[tokio::test]
async fn next_does_not_repeat_a_card_inside_the_window() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "nine cards", None).await;
    let mut card_ids: Vec<i64> = Vec::new();
    for index in 0..9 {
        card_ids.push(create_flashcard(&app, deck_id, &format!("card {index}")).await);
    }

    let (_, seeding) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;
    let seeding_session = seeding["id"].as_i64().unwrap();
    for card_id in &card_ids {
        sqlx::query(
            "INSERT INTO reviews (card_id, session_id, correct, answered_at)
             VALUES (?, ?, 1, '2026-08-01T10:00:00Z')",
        )
        .bind(card_id)
        .bind(seeding_session)
        .execute(&app.pool)
        .await
        .unwrap();
    }

    let session_id = start_session(&app, deck_id).await;

    let mut served: Vec<i64> = Vec::new();
    for _ in 0..9 {
        let (_, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        let card_id = body["card"]["id"].as_i64().unwrap();
        assert!(
            !served.contains(&card_id),
            "card {card_id} repeated inside the window; served so far {served:?}",
        );
        served.push(card_id);
        answer_card(&app, session_id, card_id).await;
    }
    assert_eq!(served.len(), 9, "all nine cards should have been served exactly once");
}

#[tokio::test]
async fn the_no_repeat_window_survives_a_reload() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "ten cards", None).await;
    for index in 0..10 {
        create_flashcard(&app, deck_id, &format!("card {index}")).await;
    }
    let session_id = start_session(&app, deck_id).await;

    let mut answered: Vec<i64> = Vec::new();
    for _ in 0..3 {
        let (_, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        let card_id = body["card"]["id"].as_i64().unwrap();
        answered.push(card_id);
        answer_card(&app, session_id, card_id).await;
    }

    for _ in 0..15 {
        let (_, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        let card_id = body["card"]["id"].as_i64().unwrap();
        assert!(
            !answered.contains(&card_id),
            "card {card_id} was answered before the reload and must still be excluded",
        );
    }
}

#[tokio::test]
async fn a_three_card_deck_still_serves_a_card() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "three cards", None).await;
    for index in 0..3 {
        create_flashcard(&app, deck_id, &format!("card {index}")).await;
    }
    let session_id = start_session(&app, deck_id).await;

    for round in 0..40 {
        let (status, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        assert_eq!(status, 200, "round {round} failed: {body}");
        answer_card(&app, session_id, body["card"]["id"].as_i64().unwrap()).await;
    }
}

#[tokio::test]
async fn a_one_card_deck_serves_the_same_card_repeatedly() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "one card", None).await;
    let card_id = create_flashcard(&app, deck_id, "the only card").await;
    let session_id = start_session(&app, deck_id).await;

    for _ in 0..10 {
        let (status, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        assert_eq!(status, 200);
        assert_eq!(body["card"]["id"].as_i64(), Some(card_id));
        answer_card(&app, session_id, card_id).await;
    }
}

#[tokio::test]
async fn next_conflicts_when_every_pool_card_is_archived_mid_session() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "vanishing", None).await;
    let card_id = create_flashcard(&app, deck_id, "the only card").await;
    let session_id = start_session(&app, deck_id).await;

    app.post(&format!("/api/cards/{card_id}/archive"), json!({})).await;

    let (status, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(status, 409, "an emptied pool must conflict, not crash: {body}");
    assert_eq!(body["error"], "conflict");
}

#[tokio::test]
async fn next_on_an_unknown_session_is_not_found() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/sessions/9999/next").await;
    assert_eq!(status, 404);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn next_reports_the_progress_counts_from_reviews() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "counting", None).await;
    for index in 0..5 {
        create_flashcard(&app, deck_id, &format!("card {index}")).await;
    }
    let session_id = start_session(&app, deck_id).await;

    let (_, before) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(before["pool_count"], 5);
    assert_eq!(before["answered_count"], 0);

    answer_card(&app, session_id, before["card"]["id"].as_i64().unwrap()).await;

    let (_, after) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(after["answered_count"], 1, "progress must come from reviews, not the client");
}

#[tokio::test]
async fn revealing_a_flashcard_returns_its_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flashcards", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    let (status, body) = app
        .post(&format!("/api/sessions/{session_id}/reveal"), json!({ "card_id": card_id }))
        .await;

    assert_eq!(status, 200, "body was {body}");
    assert_eq!(body["card_id"], card_id);
    assert_eq!(body["answer_md"], "an answer");
}

#[tokio::test]
async fn refuses_to_reveal_a_graded_card() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "graded kinds", None).await;
    let multiple_choice = create_multiple_choice(&app, deck_id, "which linkage").await;
    let short_answer = create_short_answer(&app, deck_id, "name the algorithm").await;
    let session_id = start_session(&app, deck_id).await;

    for card_id in [multiple_choice, short_answer] {
        let (status, body) = app
            .post(&format!("/api/sessions/{session_id}/reveal"), json!({ "card_id": card_id }))
            .await;
        assert_eq!(status, 409, "reveal must not become a key oracle: {body}");
        let serialised = body.to_string();
        for forbidden in ["is_correct", "complete linkage", "k-means", "because the centroid"] {
            assert!(!serialised.contains(forbidden), "refusal leaked {forbidden}: {serialised}");
        }
    }
}

#[tokio::test]
async fn refuses_to_reveal_a_card_outside_the_session() {
    let app = common::spawn_app().await;
    let inside = create_deck(&app, "inside", None).await;
    let outside = create_deck(&app, "outside", None).await;
    create_flashcard(&app, inside, "in").await;
    let foreign = create_flashcard(&app, outside, "out").await;
    let session_id = start_session(&app, inside).await;

    let (status, body) = app
        .post(&format!("/api/sessions/{session_id}/reveal"), json!({ "card_id": foreign }))
        .await;

    assert_eq!(status, 422);
    assert!(has_field(&body, "card_id"));
}

#[tokio::test]
async fn revealing_writes_nothing() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flashcards", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    app.post(&format!("/api/sessions/{session_id}/reveal"), json!({ "card_id": card_id }))
        .await;

    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn next_and_reveal_on_a_finished_session_conflict() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "finished", None).await;
    let card_id = create_flashcard(&app, deck_id, "a card").await;
    let session_id = start_session(&app, deck_id).await;

    sqlx::query("UPDATE sessions SET ended_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id = ?")
        .bind(session_id)
        .execute(&app.pool)
        .await
        .unwrap();

    let (next_status, next_body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(next_status, 409, "body was {next_body}");
    assert_eq!(next_body["error"], "conflict");

    let (reveal_status, _) = app
        .post(&format!("/api/sessions/{session_id}/reveal"), json!({ "card_id": card_id }))
        .await;
    assert_eq!(reveal_status, 409);
}

#[tokio::test]
async fn reveal_on_an_unknown_session_is_not_found() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/sessions/9999/reveal", json!({ "card_id": 1 })).await;
    assert_eq!(status, 404);
    assert_eq!(body["error"], "not_found");
}
