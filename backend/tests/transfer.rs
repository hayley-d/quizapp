mod common;

use axum::http::StatusCode;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use common::{spawn_app, TestApp};
use serde_json::{json, Value};

fn png_bytes(filler: usize) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.resize(8 + filler, 0xAB);
    bytes
}

async fn create_module(app: &TestApp, name: &str) -> i64 {
    let (status, body) = app.post("/api/modules", json!({ "name": name })).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().expect("module id")
}

async fn create_deck(app: &TestApp, module_id: Option<i64>, name: &str) -> i64 {
    let (status, body) = app
        .post(
            "/api/decks",
            json!({ "name": name, "module_id": module_id, "description": "notes" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().expect("deck id")
}

async fn create_card(app: &TestApp, body: Value) -> i64 {
    let (status, created) = app.post("/api/cards", body).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    created["id"].as_i64().expect("card id")
}

async fn create_one_of_each_kind(app: &TestApp, deck_id: i64) {
    create_card(
        app,
        json!({
            "deck_id": deck_id,
            "kind": "mc_single",
            "prompt_md": "Which receptor does propranolol block?",
            "explanation_md": "Non-selective, so $\\beta_1$ and $\\beta_2$.",
            "choices": [
                { "text_md": "$\\beta_1$ only", "is_correct": false },
                { "text_md": "$\\beta_1$ and $\\beta_2$", "is_correct": true },
                { "text_md": "$\\alpha_1$ only", "is_correct": false }
            ]
        }),
    )
    .await;

    create_card(
        app,
        json!({
            "deck_id": deck_id,
            "kind": "text_answer",
            "prompt_md": "Name the enzyme inhibited by aspirin",
            "accepted": [
                { "text": "cyclooxygenase", "is_primary": true },
                { "text": "COX", "is_primary": false }
            ]
        }),
    )
    .await;

    create_card(
        app,
        json!({
            "deck_id": deck_id,
            "kind": "flashcard",
            "prompt_md": "Half-life of amiodarone",
            "answer_md": "About 58 days",
        }),
    )
    .await;
}

async fn export_deck(app: &TestApp, deck_id: i64) -> Value {
    let (status, body) = app.get(&format!("/api/decks/{deck_id}/export")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn import(app: &TestApp, file: Value) -> (StatusCode, Value) {
    app.post("/api/import", file).await
}

async fn import_expecting_success(app: &TestApp, file: Value) -> Value {
    let (status, body) = import(app, file).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

fn strip_volatile(mut file: Value) -> Value {
    file.as_object_mut().expect("object").remove("exported_at");
    file
}

#[tokio::test]
async fn a_deck_of_every_card_kind_survives_an_export_and_import_unchanged() {
    let app = spawn_app().await;
    let module_id = create_module(&app, "Pharmacology").await;
    let deck_id = create_deck(&app, Some(module_id), "Beta blockers").await;
    create_one_of_each_kind(&app, deck_id).await;

    let exported = export_deck(&app, deck_id).await;
    assert_eq!(exported["format"], "quizapp-transfer");
    assert_eq!(exported["format_version"], 1);
    assert_eq!(exported["decks"][0]["module_name"], "Pharmacology");
    assert_eq!(exported["decks"][0]["name"], "Beta blockers");
    assert_eq!(exported["decks"][0]["description"], "notes");

    let result = import_expecting_success(&app, exported.clone()).await;
    let imported_deck_id = result["decks"][0]["id"].as_i64().expect("imported deck id");
    assert_eq!(result["decks"][0]["card_count"], 3);

    let mut round_tripped = strip_volatile(export_deck(&app, imported_deck_id).await);
    let mut original = strip_volatile(exported);

    round_tripped["decks"][0]["name"] = json!("Beta blockers");
    assert_eq!(round_tripped, original.take());
}

#[tokio::test]
async fn card_order_and_the_archived_flag_survive_the_round_trip() {
    let app = spawn_app().await;
    let deck_id = create_deck(&app, None, "Ordering").await;

    for prompt in ["first", "second", "third", "fourth"] {
        create_card(
            &app,
            json!({
                "deck_id": deck_id, "kind": "flashcard",
                "prompt_md": prompt, "answer_md": "answer",
            }),
        )
        .await;
    }
    let archived_id = create_card(
        &app,
        json!({
            "deck_id": deck_id, "kind": "flashcard",
            "prompt_md": "fifth", "answer_md": "answer",
        }),
    )
    .await;
    let (status, body) = app.post(&format!("/api/cards/{archived_id}/archive"), json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let exported = export_deck(&app, deck_id).await;
    let result = import_expecting_success(&app, exported).await;
    let imported_deck_id = result["decks"][0]["id"].as_i64().expect("imported deck id");

    let round_tripped = export_deck(&app, imported_deck_id).await;
    let cards = round_tripped["decks"][0]["cards"].as_array().expect("cards");

    let prompts: Vec<&str> =
        cards.iter().map(|card| card["prompt_md"].as_str().expect("prompt")).collect();
    assert_eq!(prompts, ["first", "second", "third", "fourth", "fifth"]);
    assert_eq!(cards[4]["archived"], true);
    assert_eq!(cards[0]["archived"], false);
}

#[tokio::test]
async fn an_image_travels_as_base64_and_is_written_once_however_often_it_is_imported() {
    let app = spawn_app().await;
    let deck_id = create_deck(&app, None, "Diagrams").await;

    let bytes = png_bytes(64);
    let (status, uploaded) = app.post_file("/api/images", "file", "diagram.png", &bytes).await;
    assert_eq!(status, StatusCode::CREATED, "{uploaded}");
    let image_path = uploaded["path"].as_str().expect("image path").to_string();

    create_card(
        &app,
        json!({
            "deck_id": deck_id, "kind": "text_answer",
            "prompt_md": "Label the highlighted structure",
            "image_path": image_path,
            "accepted": [{ "text": "hippocampus", "is_primary": true }],
        }),
    )
    .await;

    let exported = export_deck(&app, deck_id).await;
    let encoded = exported["decks"][0]["cards"][0]["image_base64"]
        .as_str()
        .expect("image_base64 present");
    assert_eq!(BASE64.decode(encoded).expect("decodes"), bytes);
    assert!(exported["decks"][0]["cards"][0].get("image_path").is_none());

    assert_eq!(app.image_count().await, 1);

    let first = import_expecting_success(&app, exported.clone()).await;
    assert_eq!(first["image_count"], 1);
    assert_eq!(app.image_count().await, 1, "a content-addressed image must not duplicate");

    let imported_deck_id = first["decks"][0]["id"].as_i64().expect("imported deck id");
    let (status, cards) = app.get(&format!("/api/cards?deck_id={imported_deck_id}")).await;
    assert_eq!(status, StatusCode::OK, "{cards}");
    assert_eq!(cards[0]["image_path"], image_path.as_str());

    import_expecting_success(&app, exported).await;
    assert_eq!(app.image_count().await, 1);
}

#[tokio::test]
async fn an_import_carries_no_progress_and_leaves_the_deck_unstudied() {
    let app = spawn_app().await;
    let deck_id = create_deck(&app, None, "Fresh").await;
    create_one_of_each_kind(&app, deck_id).await;

    let (status, session) = app
        .post("/api/sessions", json!({ "mode": "practice", "deck_ids": [deck_id] }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    let session_id = session["id"].as_i64().expect("session id");
    let (status, next) = app.get(&format!("/api/sessions/{session_id}/next")).await;
    assert_eq!(status, StatusCode::OK, "{next}");
    let card_id = next["card"]["id"].as_i64().expect("card id");
    let answer = match next["card"]["kind"].as_str().expect("kind") {
        "mc_single" => json!({ "card_id": card_id, "choice_id": next["card"]["choices"][0]["id"] }),
        "flashcard" => json!({ "card_id": card_id, "self_grade": "again" }),
        _ => json!({ "card_id": card_id, "given": "wrong" }),
    };
    let (status, answered) =
        app.post(&format!("/api/sessions/{session_id}/answer"), answer).await;
    assert_eq!(status, StatusCode::OK, "{answered}");
    assert!(app.count("SELECT COUNT(*) FROM reviews").await > 0);

    let exported = export_deck(&app, deck_id).await;
    for absent in ["reviews", "schedule", "sessions"] {
        assert!(exported.get(absent).is_none(), "export must not carry {absent}");
    }
    for card in exported["decks"][0]["cards"].as_array().expect("cards") {
        for absent in ["reviews", "schedule", "id", "position", "created_at"] {
            assert!(card.get(absent).is_none(), "a card must not carry {absent}");
        }
    }

    let reviews_before = app.count("SELECT COUNT(*) FROM reviews").await;
    let result = import_expecting_success(&app, exported).await;
    let imported_deck_id = result["decks"][0]["id"].as_i64().expect("imported deck id");

    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, reviews_before);

    let (status, stats) = app.get(&format!("/api/decks/{imported_deck_id}/stats")).await;
    assert_eq!(status, StatusCode::OK, "{stats}");
    assert_eq!(stats["summary"]["mastery_counts"]["unseen"], 3);

    let schedule_rows = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM schedule WHERE card_id IN (SELECT id FROM cards WHERE deck_id = ?)",
    )
    .bind(imported_deck_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(schedule_rows, 3, "imported cards must be schedulable like hand-created ones");
}

#[tokio::test]
async fn a_name_already_taken_is_suffixed_and_the_existing_deck_is_untouched() {
    let app = spawn_app().await;
    let module_id = create_module(&app, "Pharmacology").await;
    let deck_id = create_deck(&app, Some(module_id), "Beta blockers").await;
    create_one_of_each_kind(&app, deck_id).await;

    let exported = export_deck(&app, deck_id).await;

    let first = import_expecting_success(&app, exported.clone()).await;
    assert_eq!(first["decks"][0]["name"], "Beta blockers (2)");
    assert_eq!(first["decks"][0]["original_name"], "Beta blockers");

    let second = import_expecting_success(&app, exported).await;
    assert_eq!(second["decks"][0]["name"], "Beta blockers (3)");

    let (status, original) = app.get(&format!("/api/decks/{deck_id}")).await;
    assert_eq!(status, StatusCode::OK, "{original}");
    assert_eq!(original["name"], "Beta blockers");
    assert_eq!(original["card_count"], 3);
    assert_eq!(app.count("SELECT COUNT(*) FROM modules").await, 1);
}

#[tokio::test]
async fn two_decks_of_the_same_name_in_one_file_do_not_collide() {
    let app = spawn_app().await;

    let file = json!({
        "format": "quizapp-transfer",
        "format_version": 1,
        "decks": [
            { "module_name": null, "name": "Revision", "description": "", "cards": [] },
            { "module_name": null, "name": "Revision", "description": "", "cards": [] },
            { "module_name": null, "name": "Revision", "description": "", "cards": [] }
        ]
    });

    let result = import_expecting_success(&app, file).await;
    let names: Vec<&str> = result["decks"]
        .as_array()
        .expect("decks")
        .iter()
        .map(|deck| deck["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["Revision", "Revision (2)", "Revision (3)"]);
}

#[tokio::test]
async fn a_module_is_matched_by_name_created_when_missing_and_null_imports_unfiled() {
    let app = spawn_app().await;
    let existing_module_id = create_module(&app, "Pharmacology").await;

    let file = json!({
        "format": "quizapp-transfer",
        "format_version": 1,
        "decks": [
            { "module_name": "Pharmacology", "name": "Matched", "description": "", "cards": [] },
            { "module_name": "Neurology", "name": "Created", "description": "", "cards": [] },
            { "module_name": null, "name": "Unfiled", "description": "", "cards": [] }
        ]
    });

    let result = import_expecting_success(&app, file).await;

    let (status, decks) = app.get("/api/decks").await;
    assert_eq!(status, StatusCode::OK, "{decks}");

    let find = |name: &str| -> Value {
        decks
            .as_array()
            .expect("decks")
            .iter()
            .find(|deck| deck["name"] == name)
            .expect("deck present")
            .clone()
    };

    assert_eq!(find("Matched")["module_id"], existing_module_id);
    assert_eq!(find("Created")["module_name"], "Neurology");
    assert_eq!(find("Unfiled")["module_id"], Value::Null);
    assert_eq!(find("Unfiled")["module_name"], Value::Null);

    assert_eq!(app.count("SELECT COUNT(*) FROM modules").await, 2);
    assert_eq!(result["decks"].as_array().expect("decks").len(), 3);
}

#[tokio::test]
async fn a_module_export_carries_every_deck_in_it() {
    let app = spawn_app().await;
    let module_id = create_module(&app, "Pharmacology").await;
    let first_deck_id = create_deck(&app, Some(module_id), "Beta blockers").await;
    create_deck(&app, Some(module_id), "Diuretics").await;
    create_deck(&app, None, "Unrelated").await;
    create_one_of_each_kind(&app, first_deck_id).await;

    let (status, exported) = app.get(&format!("/api/modules/{module_id}/export")).await;
    assert_eq!(status, StatusCode::OK, "{exported}");

    let names: Vec<&str> = exported["decks"]
        .as_array()
        .expect("decks")
        .iter()
        .map(|deck| deck["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["Beta blockers", "Diuretics"]);
    assert_eq!(exported["decks"][0]["cards"].as_array().expect("cards").len(), 3);

    let result = import_expecting_success(&app, exported).await;
    assert_eq!(result["decks"].as_array().expect("decks").len(), 2);
    assert_eq!(result["decks"][0]["name"], "Beta blockers (2)");
    assert_eq!(app.count("SELECT COUNT(*) FROM modules").await, 1);
}

#[tokio::test]
async fn one_invalid_card_rejects_the_whole_file_and_writes_nothing() {
    let app = spawn_app().await;
    let decks_before = app.count("SELECT COUNT(*) FROM decks").await;

    let file = json!({
        "format": "quizapp-transfer",
        "format_version": 1,
        "decks": [{
            "module_name": "Brand new module",
            "name": "Half good",
            "description": "",
            "cards": [
                {
                    "kind": "flashcard", "prompt_md": "fine", "answer_md": "fine",
                    "choices": [], "accepted": []
                },
                {
                    "kind": "mc_single", "prompt_md": "only one option",
                    "choices": [{ "text_md": "lonely", "is_correct": true }],
                    "accepted": []
                }
            ]
        }]
    });

    let (status, body) = import(&app, file).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["fields"][0]["field"], "decks[0].cards[1].choices");

    assert_eq!(app.count("SELECT COUNT(*) FROM decks").await, decks_before);
    assert_eq!(app.count("SELECT COUNT(*) FROM cards").await, 0);
    assert_eq!(app.count("SELECT COUNT(*) FROM modules").await, 0);
}

#[tokio::test]
async fn every_invalid_card_is_reported_at_once() {
    let app = spawn_app().await;

    let file = json!({
        "format": "quizapp-transfer",
        "format_version": 1,
        "decks": [
            {
                "name": "First",
                "cards": [{ "kind": "flashcard", "prompt_md": "no answer" }]
            },
            {
                "name": "Second",
                "cards": [
                    { "kind": "flashcard", "prompt_md": "fine", "answer_md": "fine" },
                    { "kind": "text_answer", "prompt_md": "no accepted answers" }
                ]
            }
        ]
    });

    let (status, body) = import(&app, file).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    let reported: Vec<&str> = body["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .map(|field| field["field"].as_str().expect("field name"))
        .collect();
    assert!(reported.contains(&"decks[0].cards[0].answer_md"), "{reported:?}");
    assert!(reported.contains(&"decks[1].cards[1].accepted"), "{reported:?}");
}

#[tokio::test]
async fn a_deck_with_a_blank_name_is_rejected() {
    let app = spawn_app().await;

    let file = json!({
        "format": "quizapp-transfer",
        "format_version": 1,
        "decks": [{ "name": "   ", "cards": [] }]
    });

    let (status, body) = import(&app, file).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["fields"][0]["field"], "decks[0].name");
}

#[tokio::test]
async fn a_file_from_another_format_or_version_is_refused() {
    let app = spawn_app().await;

    let (status, body) = import(
        &app,
        json!({ "format": "anki", "format_version": 1, "decks": [{ "name": "x", "cards": [] }] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["fields"][0]["field"], "format");

    let (status, body) = import(
        &app,
        json!({
            "format": "quizapp-transfer", "format_version": 2,
            "decks": [{ "name": "x", "cards": [] }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["fields"][0]["field"], "format_version");

    let (status, body) =
        import(&app, json!({ "format": "quizapp-transfer", "format_version": 1, "decks": [] }))
            .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["fields"][0]["field"], "decks");
}

#[tokio::test]
async fn an_unknown_key_anywhere_in_the_file_is_refused() {
    let app = spawn_app().await;

    let (status, body) = import(
        &app,
        json!({
            "format": "quizapp-transfer", "format_version": 1,
            "decks": [{ "name": "x", "cards": [] }],
            "reviews": []
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    let (status, body) = import(
        &app,
        json!({
            "format": "quizapp-transfer", "format_version": 1,
            "decks": [{ "name": "x", "cards": [
                { "kind": "flashcard", "prompt_md": "p", "answer_md": "a", "position": 3 }
            ] }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

#[tokio::test]
async fn image_bytes_that_are_not_a_readable_image_are_refused() {
    let app = spawn_app().await;

    let not_an_image = BASE64.encode(b"just some notes about k-means");
    let (status, body) = import(
        &app,
        json!({
            "format": "quizapp-transfer", "format_version": 1,
            "decks": [{ "name": "x", "cards": [{
                "kind": "flashcard", "prompt_md": "p", "answer_md": "a",
                "image_base64": not_an_image
            }] }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["fields"][0]["field"], "decks[0].cards[0].image_base64");

    let (status, body) = import(
        &app,
        json!({
            "format": "quizapp-transfer", "format_version": 1,
            "decks": [{ "name": "x", "cards": [{
                "kind": "flashcard", "prompt_md": "p", "answer_md": "a",
                "image_base64": "this is not base64 at all!!"
            }] }]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["fields"][0]["field"], "decks[0].cards[0].image_base64");

    assert_eq!(app.image_count().await, 0);
    assert_eq!(app.count("SELECT COUNT(*) FROM decks").await, 0);
}

#[tokio::test]
async fn base64_wrapped_over_several_lines_still_decodes() {
    let app = spawn_app().await;
    let bytes = png_bytes(120);
    let wrapped = BASE64
        .encode(&bytes)
        .as_bytes()
        .chunks(40)
        .map(|chunk| String::from_utf8(chunk.to_vec()).expect("ascii"))
        .collect::<Vec<String>>()
        .join("\n");

    let result = import_expecting_success(
        &app,
        json!({
            "format": "quizapp-transfer", "format_version": 1,
            "decks": [{ "name": "Hand written", "cards": [{
                "kind": "flashcard", "prompt_md": "p", "answer_md": "a",
                "image_base64": wrapped
            }] }]
        }),
    )
    .await;

    assert_eq!(result["image_count"], 1);
    assert_eq!(app.image_count().await, 1);

    let deck_id = result["decks"][0]["id"].as_i64().expect("deck id");
    let (status, cards) = app.get(&format!("/api/cards?deck_id={deck_id}")).await;
    assert_eq!(status, StatusCode::OK, "{cards}");
    let stored = cards[0]["image_path"].as_str().expect("image path");
    assert_eq!(
        std::fs::read(app.images_directory.join(stored.trim_start_matches("images/")))
            .expect("stored image"),
        bytes
    );
}

#[tokio::test]
async fn a_minimal_hand_written_file_imports() {
    let app = spawn_app().await;

    let result = import_expecting_success(
        &app,
        json!({
            "format": "quizapp-transfer",
            "format_version": 1,
            "decks": [{
                "name": "Written by hand",
                "cards": [
                    { "kind": "flashcard", "prompt_md": "What is $e$?", "answer_md": "2.718..." },
                    {
                        "kind": "text_answer", "prompt_md": "Capital of Peru",
                        "accepted": [{ "text": "Lima", "is_primary": true }]
                    }
                ]
            }]
        }),
    )
    .await;

    assert_eq!(result["decks"][0]["name"], "Written by hand");
    assert_eq!(result["decks"][0]["card_count"], 2);
    assert_eq!(result["image_count"], 0);
}

#[tokio::test]
async fn export_names_the_file_after_a_slug_of_the_deck_or_module() {
    let app = spawn_app().await;
    let module_id = create_module(&app, "Clinical Pharmacology!").await;
    let deck_id = create_deck(&app, Some(module_id), "Beta blockers: $\\beta_1$").await;

    assert_eq!(
        app.header_value(&format!("/api/decks/{deck_id}/export"), "content-disposition").await,
        Some("attachment; filename=\"beta-blockers-beta-1.quizapp.json\"".to_string()),
    );
    assert_eq!(
        app.header_value(&format!("/api/modules/{module_id}/export"), "content-disposition").await,
        Some("attachment; filename=\"clinical-pharmacology.quizapp.json\"".to_string()),
    );
}

#[tokio::test]
async fn exporting_something_that_does_not_exist_is_a_404() {
    let app = spawn_app().await;

    let (status, body) = app.get("/api/decks/999/export").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");

    let (status, body) = app.get("/api/modules/999/export").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn an_empty_module_exports_a_file_with_no_decks_which_import_then_refuses() {
    let app = spawn_app().await;
    let module_id = create_module(&app, "Empty").await;

    let (status, exported) = app.get(&format!("/api/modules/{module_id}/export")).await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert_eq!(exported["decks"].as_array().expect("decks").len(), 0);

    let (status, body) = import(&app, exported).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["fields"][0]["field"], "decks");
}

#[tokio::test]
async fn the_multi_point_mode_round_trips_through_an_export_and_import() {
    let app = spawn_app().await;
    let deck_id = create_deck(&app, None, "mining").await;
    for (prompt, mode) in [("auto", "auto"), ("forced on", "on"), ("forced off", "off")] {
        create_card(
            &app,
            json!({
                "deck_id": deck_id,
                "kind": "text_answer",
                "prompt_md": prompt,
                "accepted": [{ "text": "1. Volume\n2. Velocity", "is_primary": true }],
                "multi_point_mode": mode,
            }),
        )
        .await;
    }

    let exported = export_deck(&app, deck_id).await;
    let modes: Vec<&str> = exported["decks"][0]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| card["multi_point_mode"].as_str().unwrap())
        .collect();
    assert_eq!(modes, ["auto", "on", "off"], "{exported}");

    let imported = import_expecting_success(&app, exported).await;
    let new_deck_id = imported["decks"][0]["id"].as_i64().unwrap();
    let stored = sqlx::query_scalar!(
        r#"SELECT multi_point_mode AS "multi_point_mode!: String" FROM cards
           WHERE deck_id = ? ORDER BY position"#,
        new_deck_id,
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    assert_eq!(stored, ["auto", "on", "off"]);
}

#[tokio::test]
async fn a_file_exported_before_multi_point_answers_existed_still_imports() {
    let app = spawn_app().await;
    let file = json!({
        "format": "quizapp-transfer",
        "format_version": 1,
        "exported_at": "2026-01-01T00:00:00Z",
        "decks": [{
            "name": "mining",
            "description": "",
            "cards": [{
                "kind": "text_answer",
                "prompt_md": "the 2 v's",
                "archived": false,
                "accepted": [{ "text": "1. Volume\n2. Velocity", "is_primary": true }],
            }],
        }],
    });

    let imported = import_expecting_success(&app, file).await;
    let deck_id = imported["decks"][0]["id"].as_i64().unwrap();
    let mode = sqlx::query_scalar!(
        r#"SELECT multi_point_mode AS "multi_point_mode!: String" FROM cards WHERE deck_id = ?"#,
        deck_id,
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        mode, "auto",
        "a card written before the field existed must land on the automatic behaviour",
    );
}
