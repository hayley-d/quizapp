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
async fn accepts_sm2_mode_now_that_part_seven_has_landed() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "clustering", None).await;
    create_flashcard(&app, deck_id, "a").await;

    let (status, body) = app
        .post("/api/sessions", json!({ "mode": "sm2", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(status, 201, "sm2 must be accepted: {body}");
    assert_eq!(body["mode"], "sm2");
    assert_eq!(body["target_count"], 1, "target_count is the due count: {body}");
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

async fn create_text_answer(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "text_answer",
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
    create_text_answer(&app, deck_id, "name the algorithm").await;
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

        let mut envelope_keys: Vec<&str> =
            body.as_object().unwrap().keys().map(String::as_str).collect();
        envelope_keys.sort_unstable();
        assert_eq!(
            envelope_keys,
            vec![
                "answered_count",
                "card",
                "correct_count",
                "mastery_goal",
                "mastery_moved_up_count",
                "mode",
                "pool_count",
            ],
            "the serve envelope must carry exactly the card, the mode and the progress counts",
        );

        let serialised_card = body["card"].to_string();
        for forbidden in
            ["is_correct", "answer_md", "explanation_md", "accepted", "expected", "correct"]
        {
            assert!(
                !serialised_card.contains(forbidden),
                "a served card leaked {forbidden}: {serialised_card}",
            );
        }

        let serialised = body.to_string();
        for answer_text in
            ["because the centroid moves", "k-means", "lloyd's algorithm", "an answer"]
        {
            assert!(
                !serialised.contains(answer_text),
                "the serve response leaked the answer text {answer_text}: {serialised}",
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
async fn a_missed_card_is_served_again_immediately_until_it_is_answered_correctly() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "ten cards", None).await;
    for index in 0..10 {
        create_flashcard(&app, deck_id, &format!("card {index}")).await;
    }
    let session_id = start_session(&app, deck_id).await;

    let (_, first) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    let missed_card_id = first["card"]["id"].as_i64().unwrap();

    for attempt in 0..4 {
        let (status, answer) = app
            .post(
                &format!("/api/sessions/{session_id}/answer"),
                json!({ "card_id": missed_card_id, "self_grade": "again" }),
            )
            .await;
        assert_eq!(status, 200, "attempt {attempt} was rejected: {answer}");
        assert_eq!(answer["correct"], json!(false));

        let (_, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
        assert_eq!(
            body["card"]["id"].as_i64(),
            Some(missed_card_id),
            "a card answered wrong must come straight back, not after the window",
        );
    }

    let (status, answer) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": missed_card_id, "self_grade": "good" }),
        )
        .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["correct"], json!(true));

    let (_, body) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_ne!(
        body["card"]["id"].as_i64(),
        Some(missed_card_id),
        "once answered correctly the card must rejoin the ordinary rotation",
    );
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
    assert_eq!(after["correct_count"], 1);

    let another = after["card"]["id"].as_i64().unwrap();
    sqlx::query("INSERT INTO reviews (card_id, session_id, correct) VALUES (?, ?, 0)")
        .bind(another)
        .bind(session_id)
        .execute(&app.pool)
        .await
        .unwrap();

    let (_, final_progress) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(final_progress["answered_count"], 2);
    assert_eq!(
        final_progress["correct_count"], 1,
        "a wrong answer must raise answered without raising correct",
    );
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
    let text_answer = create_text_answer(&app, deck_id, "name the algorithm").await;
    let session_id = start_session(&app, deck_id).await;

    for card_id in [multiple_choice, text_answer] {
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

async fn correct_choice_id(app: &common::TestApp, card_id: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM choices WHERE card_id = ? AND is_correct = 1")
        .bind(card_id)
        .fetch_one(&app.pool)
        .await
        .unwrap()
}

async fn wrong_choice_id(app: &common::TestApp, card_id: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM choices WHERE card_id = ? AND is_correct = 0 ORDER BY id")
        .bind(card_id)
        .fetch_one(&app.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn grades_a_correct_multiple_choice_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mc", None).await;
    let card_id = create_multiple_choice(&app, deck_id, "which linkage").await;
    let session_id = start_session(&app, deck_id).await;
    let choice_id = correct_choice_id(&app, card_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "choice_id": choice_id }),
        )
        .await;

    assert_eq!(status, 200, "body was {body}");
    assert_eq!(body["correct"], true);
    assert_eq!(body["expected"], json!(["complete linkage"]));
    assert_eq!(body["explanation_md"], "because the centroid moves");
    assert_eq!(body["can_override"], false);
    assert!(body["review_id"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn grades_an_incorrect_multiple_choice_answer_and_returns_the_expected_choice() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mc", None).await;
    let card_id = create_multiple_choice(&app, deck_id, "which linkage").await;
    let session_id = start_session(&app, deck_id).await;
    let choice_id = wrong_choice_id(&app, card_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "choice_id": choice_id }),
        )
        .await;

    assert_eq!(status, 200);
    assert_eq!(body["correct"], false);
    assert_eq!(body["expected"], json!(["complete linkage"]));
    assert_eq!(
        body["can_override"], false,
        "a multiple-choice miss is not an unfair miss",
    );
}

#[tokio::test]
async fn stores_the_chosen_choice_text_for_a_multiple_choice_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mc", None).await;
    let card_id = create_multiple_choice(&app, deck_id, "which linkage").await;
    let session_id = start_session(&app, deck_id).await;
    let choice_id = wrong_choice_id(&app, card_id).await;

    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "choice_id": choice_id }),
    )
    .await;

    let stored: String = sqlx::query_scalar("SELECT given FROM reviews WHERE card_id = ?")
        .bind(card_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        stored, "single linkage",
        "the chosen wording is stored, not its id, so it survives the card being edited",
    );
}

#[tokio::test]
async fn rejects_a_choice_id_from_another_card() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mc", None).await;
    let first = create_multiple_choice(&app, deck_id, "first").await;
    let second = create_multiple_choice(&app, deck_id, "second").await;
    let session_id = start_session(&app, deck_id).await;
    let foreign_choice = correct_choice_id(&app, second).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": first, "choice_id": foreign_choice }),
        )
        .await;

    assert_eq!(status, 422, "a foreign option is a bad request, not a wrong answer");
    assert!(has_field(&body, "choice_id"));
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn grades_a_text_answer_by_normalised_match() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name the algorithm").await;
    let session_id = start_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "  K-Means!  " }),
        )
        .await;

    assert_eq!(status, 200, "body was {body}");
    assert_eq!(body["correct"], true, "normalisation must fold case and punctuation");
    assert_eq!(body["expected"], json!(["k-means", "lloyd's algorithm"]));
}

#[tokio::test]
async fn a_wrong_text_answer_can_be_overridden() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name the algorithm").await;
    let session_id = start_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "centroid clustering" }),
        )
        .await;

    assert_eq!(status, 200);
    assert_eq!(body["correct"], false);
    assert_eq!(body["can_override"], true);
}

#[tokio::test]
async fn stores_the_submitted_wording_verbatim_for_a_text_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name the algorithm").await;
    let session_id = start_session(&app, deck_id).await;

    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "given": "  Lloyd's Algorithm  " }),
    )
    .await;

    let stored: String = sqlx::query_scalar("SELECT given FROM reviews WHERE card_id = ?")
        .bind(card_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        stored, "Lloyd's Algorithm",
        "the raw wording is stored trimmed, never the normalised key -- the override needs it",
    );
}

#[tokio::test]
async fn a_punctuation_only_answer_is_incorrect_even_when_an_accepted_key_normalises_to_empty() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let (_, card) = app
        .post(
            "/api/cards",
            json!({
                "deck_id": deck_id,
                "kind": "text_answer",
                "prompt_md": "a dash card",
                "accepted": [{ "text": "---", "is_primary": true }],
            }),
        )
        .await;
    let card_id = card["id"].as_i64().unwrap();
    let session_id = start_session(&app, deck_id).await;

    let empty_key: String =
        sqlx::query_scalar("SELECT normalised FROM accepted WHERE card_id = ?")
            .bind(card_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(empty_key, "", "this card's accepted answer must normalise to empty to be a test");

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "!!!" }),
        )
        .await;

    assert_eq!(status, 200);
    assert_eq!(
        body["correct"], false,
        "a punctuation-only answer must not match an accepted row that normalised to empty",
    );
}

#[tokio::test]
async fn rejects_a_blank_text_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name the algorithm").await;
    let session_id = start_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": "   " }),
        )
        .await;

    assert_eq!(status, 422);
    assert!(field_errors(&body)
        .contains(&("given".to_string(), "Type an answer".to_string())));
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn a_flashcard_self_grade_of_again_is_incorrect_and_the_rest_are_correct() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    for (grade, expected_correct) in
        [("again", false), ("hard", true), ("good", true), ("easy", true)]
    {
        let (status, body) = app
            .post(
                &format!("/api/sessions/{session_id}/answer"),
                json!({ "card_id": card_id, "self_grade": grade }),
            )
            .await;
        assert_eq!(status, 200, "grade {grade} failed: {body}");
        assert_eq!(body["correct"], expected_correct, "grade {grade} graded wrongly");
        assert_eq!(body["expected"], json!(["an answer"]));
        assert_eq!(body["can_override"], false, "a flashcard grader can simply grade again");
    }
}

#[tokio::test]
async fn a_flashcard_self_grade_is_persisted() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "self_grade": "hard" }),
    )
    .await;

    let stored: String = sqlx::query_scalar("SELECT self_grade FROM reviews WHERE card_id = ?")
        .bind(card_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(stored, "hard", "the four-level grade must survive for SM-2 in part 7");

    let given: Option<String> = sqlx::query_scalar("SELECT given FROM reviews WHERE card_id = ?")
        .bind(card_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(given, None, "a flashcard stores its signal in self_grade, not given");
}

#[tokio::test]
async fn rejects_an_unknown_self_grade() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "medium" }),
        )
        .await;

    assert_eq!(status, 422);
    assert!(has_field(&body, "self_grade"));
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn rejects_the_wrong_answer_field_for_each_kind() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "all kinds", None).await;
    let multiple_choice = create_multiple_choice(&app, deck_id, "which linkage").await;
    let text_answer = create_text_answer(&app, deck_id, "name it").await;
    let flashcard = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;
    let choice_id = correct_choice_id(&app, multiple_choice).await;

    let cases = [
        (multiple_choice, json!({ "card_id": multiple_choice, "given": "text" }), "given"),
        (text_answer, json!({ "card_id": text_answer, "choice_id": choice_id }), "choice_id"),
        (flashcard, json!({ "card_id": flashcard, "given": "text" }), "given"),
        (
            multiple_choice,
            json!({ "card_id": multiple_choice, "choice_id": choice_id, "self_grade": "good" }),
            "self_grade",
        ),
    ];

    for (_, body_json, expected_field) in cases {
        let (status, body) =
            app.post(&format!("/api/sessions/{session_id}/answer"), body_json).await;
        assert_eq!(status, 422, "expected a rejection, got {body}");
        assert!(
            has_field(&body, expected_field),
            "expected an error on {expected_field}, got {body}",
        );
    }
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn rejects_a_negative_ms() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "good", "ms": -1 }),
        )
        .await;

    assert_eq!(status, 422);
    assert!(has_field(&body, "ms"));
}

#[tokio::test]
async fn stores_the_elapsed_milliseconds() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "self_grade": "good", "ms": 4200 }),
    )
    .await;

    let stored: i64 = sqlx::query_scalar("SELECT ms FROM reviews WHERE card_id = ?")
        .bind(card_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(stored, 4200);
}

#[tokio::test]
async fn rejects_a_card_from_another_deck() {
    let app = common::spawn_app().await;
    let inside = create_deck(&app, "inside", None).await;
    let outside = create_deck(&app, "outside", None).await;
    create_flashcard(&app, inside, "in").await;
    let foreign = create_flashcard(&app, outside, "out").await;
    let session_id = start_session(&app, inside).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": foreign, "self_grade": "good" }),
        )
        .await;

    assert_eq!(status, 422, "the card pool is the trust boundary");
    assert!(has_field(&body, "card_id"));
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn rejects_an_archived_card() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "mixed", None).await;
    create_flashcard(&app, deck_id, "live").await;
    let archived = create_flashcard(&app, deck_id, "archived").await;
    let session_id = start_session(&app, deck_id).await;
    app.post(&format!("/api/cards/{archived}/archive"), json!({})).await;

    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": archived, "self_grade": "good" }),
        )
        .await;

    assert_eq!(status, 422);
    assert!(has_field(&body, "card_id"));
}

#[tokio::test]
async fn answering_writes_exactly_one_review_row() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    app.post(
        &format!("/api/sessions/{session_id}/answer"),
        json!({ "card_id": card_id, "self_grade": "good" }),
    )
    .await;

    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 1);
}

#[tokio::test]
async fn answering_does_not_touch_the_schedule_table() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    let before: (String, f64, f64, i64, i64) = sqlx::query_as(
        "SELECT due_at, interval_days, ease, reps, lapses FROM schedule WHERE card_id = ?",
    )
    .bind(card_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    for grade in ["again", "good", "easy"] {
        app.post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": grade }),
        )
        .await;
    }

    let after: (String, f64, f64, i64, i64) = sqlx::query_as(
        "SELECT due_at, interval_days, ease, reps, lapses FROM schedule WHERE card_id = ?",
    )
    .bind(card_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    assert_eq!(before, after, "practice mode must ignore the schedule table entirely");
}

#[tokio::test]
async fn answering_on_a_finished_session_conflicts() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    sqlx::query("UPDATE sessions SET ended_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id = ?")
        .bind(session_id)
        .execute(&app.pool)
        .await
        .unwrap();

    let (status, _) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "good" }),
        )
        .await;
    assert_eq!(status, 409);
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 0);
}

#[tokio::test]
async fn answering_on_an_unknown_session_is_not_found() {
    let app = common::spawn_app().await;
    let (status, _) = app
        .post("/api/sessions/9999/answer", json!({ "card_id": 1, "self_grade": "good" }))
        .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn can_override_is_true_only_for_an_incorrect_text_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "all kinds", None).await;
    let multiple_choice = create_multiple_choice(&app, deck_id, "which linkage").await;
    let text_answer = create_text_answer(&app, deck_id, "name it").await;
    let flashcard = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;
    let wrong_choice = wrong_choice_id(&app, multiple_choice).await;

    let cases = [
        (
            "a correct text answer",
            json!({ "card_id": text_answer, "given": "k-means" }),
            false,
        ),
        (
            "an incorrect text answer",
            json!({ "card_id": text_answer, "given": "something else" }),
            true,
        ),
        (
            "an incorrect multiple choice",
            json!({ "card_id": multiple_choice, "choice_id": wrong_choice }),
            false,
        ),
        (
            "a flashcard graded again",
            json!({ "card_id": flashcard, "self_grade": "again" }),
            false,
        ),
    ];

    for (description, request, expected) in cases {
        let (status, body) =
            app.post(&format!("/api/sessions/{session_id}/answer"), request).await;
        assert_eq!(status, 200, "{description} failed: {body}");
        assert_eq!(
            body["can_override"], expected,
            "{description} should report can_override = {expected}",
        );
    }
}

async fn answer_short(app: &common::TestApp, session_id: i64, card_id: i64, given: &str) -> Value {
    let (status, body) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "given": given }),
        )
        .await;
    assert_eq!(status, 200, "answer failed: {body}");
    body
}

#[tokio::test]
async fn overriding_flips_the_targeted_review_and_adds_an_accepted_row() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let answered = answer_short(&app, session_id, card_id, "Centroid Clustering").await;
    let review_id = answered["review_id"].as_i64().unwrap();
    let accepted_before = app.count("SELECT COUNT(*) FROM accepted").await;

    let (status, body) = app
        .post(&format!("/api/reviews/{review_id}/override"), json!({}))
        .await;

    assert_eq!(status, 200, "body was {body}");
    assert_eq!(body["correct"], true);
    assert_eq!(body["overridden"], true);
    assert_eq!(body["accepted_added"], true);

    let flags: (bool, bool) =
        sqlx::query_as("SELECT correct, overridden FROM reviews WHERE id = ?")
            .bind(review_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(flags, (true, true));

    assert_eq!(app.count("SELECT COUNT(*) FROM accepted").await, accepted_before + 1);

    let added: (String, String, bool) = sqlx::query_as(
        "SELECT text, normalised, is_primary FROM accepted WHERE card_id = ? ORDER BY id DESC",
    )
    .bind(card_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(added.0, "Centroid Clustering", "the student's own wording is accepted");
    assert_eq!(added.1, "centroid clustering");
    assert!(!added.2, "an override must never create a second primary wording");
}

#[tokio::test]
async fn the_overridden_wording_is_accepted_on_the_next_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let first = answer_short(&app, session_id, card_id, "centroid clustering").await;
    assert_eq!(first["correct"], false);
    let review_id = first["review_id"].as_i64().unwrap();

    app.post(&format!("/api/reviews/{review_id}/override"), json!({})).await;

    let second = answer_short(&app, session_id, card_id, "Centroid  Clustering!").await;
    assert_eq!(
        second["correct"], true,
        "the override must teach the card, not just fix one row",
    );
}

#[tokio::test]
async fn overriding_does_not_add_a_duplicate_normalised_accepted_row() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let first = answer_short(&app, session_id, card_id, "centroid clustering").await;
    let second = answer_short(&app, session_id, card_id, "Centroid-Clustering!").await;
    let accepted_before = app.count("SELECT COUNT(*) FROM accepted").await;

    let (_, first_body) = app
        .post(
            &format!("/api/reviews/{}/override", first["review_id"].as_i64().unwrap()),
            json!({}),
        )
        .await;
    assert_eq!(first_body["accepted_added"], true);

    let (status, second_body) = app
        .post(
            &format!("/api/reviews/{}/override", second["review_id"].as_i64().unwrap()),
            json!({}),
        )
        .await;

    assert_eq!(status, 200);
    assert_eq!(
        second_body["accepted_added"], false,
        "a wording that normalises to an existing key must add nothing",
    );
    assert_eq!(
        app.count("SELECT COUNT(*) FROM accepted").await,
        accepted_before + 1,
        "two overrides of the same normalised key must add exactly one accepted row",
    );
}

#[tokio::test]
async fn overriding_leaves_other_reviews_of_the_same_card_alone() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let first = answer_short(&app, session_id, card_id, "centroid clustering").await;
    let second = answer_short(&app, session_id, card_id, "centroid clustering").await;

    app.post(
        &format!("/api/reviews/{}/override", first["review_id"].as_i64().unwrap()),
        json!({}),
    )
    .await;

    let untouched: (bool, bool) =
        sqlx::query_as("SELECT correct, overridden FROM reviews WHERE id = ?")
            .bind(second["review_id"].as_i64().unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        untouched,
        (false, false),
        "reviews are a record of what happened; a bulk flip would rewrite history",
    );
}

#[tokio::test]
async fn overriding_inserts_no_new_review_row() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let answered = answer_short(&app, session_id, card_id, "centroid clustering").await;
    let before = app.count("SELECT COUNT(*) FROM reviews").await;

    app.post(
        &format!("/api/reviews/{}/override", answered["review_id"].as_i64().unwrap()),
        json!({}),
    )
    .await;

    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, before);
}

#[tokio::test]
async fn refuses_to_override_a_multiple_choice_or_flashcard_review() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "graded", None).await;
    let multiple_choice = create_multiple_choice(&app, deck_id, "which linkage").await;
    let flashcard = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;
    let wrong_choice = wrong_choice_id(&app, multiple_choice).await;

    let (_, mc_answer) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": multiple_choice, "choice_id": wrong_choice }),
        )
        .await;
    let (_, flash_answer) = app
        .post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": flashcard, "self_grade": "again" }),
        )
        .await;

    for review in [&mc_answer, &flash_answer] {
        let review_id = review["review_id"].as_i64().unwrap();
        let (status, body) = app
            .post(&format!("/api/reviews/{review_id}/override"), json!({}))
            .await;
        assert_eq!(status, 409, "body was {body}");
    }
    assert_eq!(app.count("SELECT COUNT(*) FROM accepted").await, 0);
}

#[tokio::test]
async fn refuses_to_override_an_already_correct_review_and_adds_nothing() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let answered = answer_short(&app, session_id, card_id, "centroid clustering").await;
    let review_id = answered["review_id"].as_i64().unwrap();

    app.post(&format!("/api/reviews/{review_id}/override"), json!({})).await;
    let accepted_after_first = app.count("SELECT COUNT(*) FROM accepted").await;

    let (status, body) = app
        .post(&format!("/api/reviews/{review_id}/override"), json!({}))
        .await;

    assert_eq!(status, 409, "a second override must conflict: {body}");
    assert_eq!(
        app.count("SELECT COUNT(*) FROM accepted").await,
        accepted_after_first,
        "a refused override must write nothing",
    );
}

#[tokio::test]
async fn refuses_to_override_a_review_with_no_usable_wording() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let answered = answer_short(&app, session_id, card_id, "!!!").await;
    assert_eq!(answered["correct"], false);
    let review_id = answered["review_id"].as_i64().unwrap();

    let (status, body) = app
        .post(&format!("/api/reviews/{review_id}/override"), json!({}))
        .await;

    assert_eq!(status, 409, "body was {body}");
    let empty_keys = app
        .count("SELECT COUNT(*) FROM accepted WHERE normalised = ''")
        .await;
    assert_eq!(empty_keys, 0, "an override must never store an empty comparison key");
}

#[tokio::test]
async fn overriding_after_the_session_finished_still_works() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let answered = answer_short(&app, session_id, card_id, "centroid clustering").await;
    app.post(&format!("/api/sessions/{session_id}/finish"), json!({})).await;

    let (status, _) = app
        .post(
            &format!("/api/reviews/{}/override", answered["review_id"].as_i64().unwrap()),
            json!({}),
        )
        .await;
    assert_eq!(status, 200, "an unfair miss can be corrected after the session ends");
}

#[tokio::test]
async fn overriding_an_unknown_review_is_not_found() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/reviews/9999/override", json!({})).await;
    assert_eq!(status, 404);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn finishing_sets_ended_at_and_returns_the_summary() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    answer_short(&app, session_id, card_id, "k-means").await;
    answer_short(&app, session_id, card_id, "wrong").await;

    let (status, body) = app
        .post(&format!("/api/sessions/{session_id}/finish"), json!({}))
        .await;

    assert_eq!(status, 200, "body was {body}");
    assert_eq!(body["answered_count"], 2);
    assert_eq!(body["correct_count"], 1);
    assert_eq!(body["overridden_count"], 0);
    assert_eq!(body["distinct_card_count"], 1);
    assert_eq!(body["accuracy"], 0.5);
    assert!(body["ended_at"].as_str().unwrap().ends_with('Z'));
}

#[tokio::test]
async fn finishing_twice_returns_the_same_ended_at() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let (first_status, first) = app
        .post(&format!("/api/sessions/{session_id}/finish"), json!({}))
        .await;
    let (second_status, second) = app
        .post(&format!("/api/sessions/{session_id}/finish"), json!({}))
        .await;

    assert_eq!(first_status, 200);
    assert_eq!(second_status, 200, "a double-posted finish must not error");
    assert_eq!(
        first["ended_at"], second["ended_at"],
        "finishing twice must not move the original end time",
    );
}

#[tokio::test]
async fn accuracy_is_null_for_a_session_with_no_answers() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let (_, body) = app
        .post(&format!("/api/sessions/{session_id}/finish"), json!({}))
        .await;

    assert_eq!(body["answered_count"], 0);
    assert_eq!(
        body["accuracy"],
        Value::Null,
        "zero accuracy would claim every answer was wrong; there were none",
    );
}

#[tokio::test]
async fn the_summary_counts_an_override_as_correct() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "short", None).await;
    let card_id = create_text_answer(&app, deck_id, "name it").await;
    let session_id = start_session(&app, deck_id).await;

    let answered = answer_short(&app, session_id, card_id, "centroid clustering").await;
    app.post(
        &format!("/api/reviews/{}/override", answered["review_id"].as_i64().unwrap()),
        json!({}),
    )
    .await;

    let (_, body) = app
        .post(&format!("/api/sessions/{session_id}/finish"), json!({}))
        .await;

    assert_eq!(body["answered_count"], 1);
    assert_eq!(body["correct_count"], 1, "an overridden miss counts as correct");
    assert_eq!(body["overridden_count"], 1);
    assert_eq!(body["accuracy"], 1.0);
}

#[tokio::test]
async fn the_summary_totals_the_elapsed_milliseconds() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "flash", None).await;
    let card_id = create_flashcard(&app, deck_id, "define entropy").await;
    let session_id = start_session(&app, deck_id).await;

    for milliseconds in [1000, 2500] {
        app.post(
            &format!("/api/sessions/{session_id}/answer"),
            json!({ "card_id": card_id, "self_grade": "good", "ms": milliseconds }),
        )
        .await;
    }

    let (_, body) = app
        .post(&format!("/api/sessions/{session_id}/finish"), json!({}))
        .await;
    assert_eq!(body["total_ms"], 3500);
}

#[tokio::test]
async fn finishing_an_unknown_session_is_not_found() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/sessions/9999/finish", json!({})).await;
    assert_eq!(status, 404);
    assert_eq!(body["error"], "not_found");
}
