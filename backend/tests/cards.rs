mod common;

use axum::http::StatusCode;
use serde_json::{json, Value};

async fn deck(app: &common::TestApp, name: &str) -> i64 {
    let (_, d) = app.post("/api/decks", json!({ "name": name })).await;
    d["id"].as_i64().unwrap()
}

fn mc(deck_id: i64) -> Value {
    json!({
        "deck_id": deck_id, "kind": "mc_single",
        "prompt_md": "Which linkage merges the two closest points?",
        "choices": [
            { "text_md": "Single",   "is_correct": true  },
            { "text_md": "Complete", "is_correct": false }
        ]
    })
}

#[tokio::test]
async fn creates_an_mc_single_card() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, card) = app.post("/api/cards", mc(d)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(card["kind"], "mc_single");
    assert_eq!(card["archived"], false);
    assert!(card["answer_md"].is_null());
    assert_eq!(card["accepted"].as_array().unwrap().len(), 0);

    let choices = card["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0]["position"], 0, "position comes from array order");
    assert_eq!(choices[1]["position"], 1);
    assert_eq!(choices[0]["is_correct"], true);
}

#[tokio::test]
async fn creates_a_short_answer_card_and_normalises_accepted() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, card) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "short_answer",
            "prompt_md": "Name the partitioning algorithm.",
            "accepted": [
                { "text": "K-Means",   "is_primary": true  },
                { "text": "k means++", "is_primary": false }
            ]
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let accepted = card["accepted"].as_array().unwrap();
    assert_eq!(accepted[0]["text"], "K-Means", "the typed wording is preserved");
    assert_eq!(accepted[0]["normalised"], "k means", "the key is folded");
    assert_eq!(accepted[0]["is_primary"], true);
}

#[tokio::test]
async fn creates_a_flashcard() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, card) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "flashcard",
            "prompt_md": "Define support.",
            "answer_md": "The fraction of transactions containing the itemset."
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(card["kind"], "flashcard");
    assert_eq!(card["choices"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn mc_single_needs_two_choices_and_exactly_one_correct() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let too_few = json!({
        "deck_id": d, "kind": "mc_single", "prompt_md": "p",
        "choices": [ { "text_md": "Only", "is_correct": true } ]
    });
    let (status, body) = app.post("/api/cards", too_few).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "choices");

    let two_correct = json!({
        "deck_id": d, "kind": "mc_single", "prompt_md": "p",
        "choices": [ { "text_md": "A", "is_correct": true },
                     { "text_md": "B", "is_correct": true } ]
    });
    let (status, body) = app.post("/api/cards", two_correct).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "choices");

    let none_correct = json!({
        "deck_id": d, "kind": "mc_single", "prompt_md": "p",
        "choices": [ { "text_md": "A", "is_correct": false },
                     { "text_md": "B", "is_correct": false } ]
    });
    let (status, _) = app.post("/api/cards", none_correct).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn empty_choice_text_names_its_row() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "mc_single", "prompt_md": "p",
            "choices": [ { "text_md": "A", "is_correct": true },
                         { "text_md": "  ", "is_correct": false } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "choices[1].text_md",
               "the editor highlights the offending row, not the whole list");
}

#[tokio::test]
async fn short_answer_needs_one_primary() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "short_answer", "prompt_md": "p",
            "accepted": [ { "text": "a", "is_primary": true },
                          { "text": "b", "is_primary": true } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "accepted");
}

#[tokio::test]
async fn flashcard_needs_an_answer() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "flashcard", "prompt_md": "p", "answer_md": "   "
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "answer_md");
}

#[tokio::test]
async fn children_of_the_wrong_kind_are_rejected() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "flashcard", "prompt_md": "p", "answer_md": "a",
            "choices": [ { "text_md": "A", "is_correct": true } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "choices");
}

#[tokio::test]
async fn prompt_and_kind_are_validated() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let mut blank = mc(d);
    blank["prompt_md"] = json!("   ");
    let (status, body) = app.post("/api/cards", blank).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "prompt_md");

    let mut bad_kind = mc(d);
    bad_kind["kind"] = json!("essay");
    let (status, body) = app.post("/api/cards", bad_kind).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "kind");
}

#[tokio::test]
async fn unknown_deck_is_rejected_naming_deck_id() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/cards", mc(9999)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "deck_id");
}

#[tokio::test]
async fn every_created_card_gets_a_schedule_row() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let (_, card) = app.post("/api/cards", mc(d)).await;
    let id = card["id"].as_i64().unwrap();

    // Spec: "Schedule exists from day one" — one row per card at creation, so
    // SM-2 never needs a migration over hand-written cards.
    let row = app.schedule_for(id).await;
    assert_eq!(row.0, 1, "expected exactly one schedule row");
    assert!(!row.1.is_empty(), "due_at must be set");
}

#[tokio::test]
async fn a_rejected_create_leaves_nothing_behind() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, _) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "mc_single", "prompt_md": "p",
            "choices": [ { "text_md": "A", "is_correct": true },
                         { "text_md": "B", "is_correct": true } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // GET /api/cards?deck_id= does not exist until Task 4; assert directly
    // via the pool for now. Task 4 switches this to the HTTP call.
    assert_eq!(app.count("SELECT COUNT(*) FROM cards").await, 0, "no partial card row");
    assert_eq!(app.count("SELECT COUNT(*) FROM choices").await, 0);
    assert_eq!(app.count("SELECT COUNT(*) FROM schedule").await, 0);
}

#[tokio::test]
async fn get_returns_the_full_card_and_404s_on_unknown() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", mc(d)).await;
    let id = created["id"].as_i64().unwrap();

    let (status, card) = app.get(&format!("/api/cards/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(card["id"], id);
    assert_eq!(card["choices"].as_array().unwrap().len(), 2);
    assert_eq!(card["choices"][0]["text_md"], "Single", "children in position order");

    let (status, _) = app.get("/api/cards/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
