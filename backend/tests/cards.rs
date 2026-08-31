mod common;

use axum::http::StatusCode;
use serde_json::{json, Value};

async fn create_deck(app: &common::TestApp, name: &str) -> i64 {
    let (_, created) = app.post("/api/decks", json!({ "name": name })).await;
    created["id"].as_i64().unwrap()
}

fn multiple_choice_card(deck_id: i64) -> Value {
    json!({
        "deck_id": deck_id, "kind": "mc_single",
        "prompt_md": "Which linkage merges the two closest points?",
        "choices": [
            { "text_md": "Single",   "is_correct": true  },
            { "text_md": "Complete", "is_correct": false }
        ]
    })
}

fn short_answer_card(deck_id: i64) -> Value {
    json!({
        "deck_id": deck_id, "kind": "short_answer",
        "prompt_md": "Name the linkage that merges the two closest points",
        "accepted": [{ "text": "single", "is_primary": true }]
    })
}

async fn create_flashcard(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (status, created) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard",
            "prompt_md": prompt, "answer_md": "an answer",
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "helper card create failed: {created}");
    created["id"].as_i64().unwrap()
}

async fn record_review(app: &common::TestApp, deck_id: i64, card_id: i64) -> i64 {
    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (mode, deck_ids) VALUES ('practice', ?) RETURNING id",
    )
    .bind(format!("[{deck_id}]"))
    .fetch_one(&app.pool)
    .await
    .unwrap();

    sqlx::query_scalar(
        "INSERT INTO reviews (card_id, session_id, correct) VALUES (?, ?, 1) RETURNING id",
    )
    .bind(card_id)
    .bind(session_id)
    .fetch_one(&app.pool)
    .await
    .unwrap()
}

async fn card_ids_in_order(app: &common::TestApp, deck_id: i64) -> Vec<i64> {
    let (_, rows) = app.get(&format!("/api/cards?deck_id={deck_id}&archived=all")).await;
    rows.as_array()
        .unwrap()
        .iter()
        .map(|card| card["id"].as_i64().unwrap())
        .collect()
}

async fn card_positions(app: &common::TestApp, deck_id: i64) -> Vec<i64> {
    let (_, rows) = app.get(&format!("/api/cards?deck_id={deck_id}&archived=all")).await;
    rows.as_array()
        .unwrap()
        .iter()
        .map(|card| card["position"].as_i64().unwrap())
        .collect()
}

#[tokio::test]
async fn creates_an_mc_single_card() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, card) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
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
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, card) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "short_answer",
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
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, card) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard",
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
    let deck_id = create_deck(&app, "Test 1").await;

    let too_few = json!({
        "deck_id": deck_id, "kind": "mc_single", "prompt_md": "p",
        "choices": [ { "text_md": "Only", "is_correct": true } ]
    });
    let (status, body) = app.post("/api/cards", too_few).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "choices");

    let two_correct = json!({
        "deck_id": deck_id, "kind": "mc_single", "prompt_md": "p",
        "choices": [ { "text_md": "A", "is_correct": true },
                     { "text_md": "B", "is_correct": true } ]
    });
    let (status, body) = app.post("/api/cards", two_correct).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "choices");

    let none_correct = json!({
        "deck_id": deck_id, "kind": "mc_single", "prompt_md": "p",
        "choices": [ { "text_md": "A", "is_correct": false },
                     { "text_md": "B", "is_correct": false } ]
    });
    let (status, _) = app.post("/api/cards", none_correct).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn empty_choice_text_names_its_row() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "mc_single", "prompt_md": "p",
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
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "short_answer", "prompt_md": "p",
            "accepted": [ { "text": "a", "is_primary": true },
                          { "text": "b", "is_primary": true } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "accepted");
}

#[tokio::test]
async fn short_answer_needs_at_least_one_accepted_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "short_answer", "prompt_md": "p",
            "accepted": []
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "accepted");
}

#[tokio::test]
async fn empty_accepted_text_names_its_row() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "short_answer", "prompt_md": "p",
            "accepted": [ { "text": "a", "is_primary": true },
                          { "text": "   ", "is_primary": false } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "accepted[1].text",
               "the editor highlights the offending row, not the whole list");
}

#[tokio::test]
async fn flashcard_needs_an_answer() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard", "prompt_md": "p", "answer_md": "   "
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "answer_md");
}

#[tokio::test]
async fn children_of_the_wrong_kind_are_rejected() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard", "prompt_md": "p", "answer_md": "a",
            "choices": [ { "text_md": "A", "is_correct": true } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "choices");
}

#[tokio::test]
async fn prompt_and_kind_are_validated() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let mut blank = multiple_choice_card(deck_id);
    blank["prompt_md"] = json!("   ");
    let (status, body) = app.post("/api/cards", blank).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "prompt_md");

    let mut bad_kind = multiple_choice_card(deck_id);
    bad_kind["kind"] = json!("essay");
    let (status, body) = app.post("/api/cards", bad_kind).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "kind");
}

#[tokio::test]
async fn unknown_deck_is_rejected_naming_deck_id() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/cards", multiple_choice_card(9999)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "deck_id");
}

#[tokio::test]
async fn every_created_card_gets_a_schedule_row() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, card) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = card["id"].as_i64().unwrap();

    let (schedule_row_count, due_at) = app.schedule_for(id).await;
    assert_eq!(schedule_row_count, 1, "expected exactly one schedule row");
    assert!(!due_at.is_empty(), "due_at must be set");
}

#[tokio::test]
async fn a_rejected_create_writes_nothing() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, _) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "mc_single", "prompt_md": "p",
            "choices": [ { "text_md": "A", "is_correct": true },
                         { "text_md": "B", "is_correct": true } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, list) = app.get(&format!("/api/cards?deck_id={deck_id}")).await;
    assert_eq!(list.as_array().unwrap().len(), 0, "no partial card row");
    assert_eq!(app.count("SELECT COUNT(*) FROM choices").await, 0);
    assert_eq!(app.count("SELECT COUNT(*) FROM schedule").await, 0);
}

#[tokio::test]
async fn get_returns_the_full_card_and_404s_on_unknown() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = created["id"].as_i64().unwrap();

    let (status, card) = app.get(&format!("/api/cards/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(card["id"], id);
    assert_eq!(card["choices"].as_array().unwrap().len(), 2);

    let (status, _) = app.get("/api/cards/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn choices_come_back_in_position_order_not_id_order() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = created["id"].as_i64().unwrap();

    sqlx::query("INSERT INTO choices (card_id, text_md, is_correct, position) VALUES (?, ?, ?, ?)")
        .bind(id).bind("Inserted last, sorts first").bind(false).bind(-1)
        .execute(&app.pool)
        .await
        .unwrap();

    let (status, card) = app.get(&format!("/api/cards/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    let choices = card["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 3);
    assert_eq!(choices[0]["text_md"], "Inserted last, sorts first",
               "highest id, lowest position: proves ordering is by position, not id");
}

#[tokio::test]
async fn lists_only_the_requested_deck_in_authoring_order() {
    let app = common::spawn_app().await;
    let deck_a_id = create_deck(&app, "Deck A").await;
    let deck_b_id = create_deck(&app, "Deck B").await;

    for prompt in ["first", "second", "third"] {
        let mut card = multiple_choice_card(deck_a_id);
        card["prompt_md"] = json!(prompt);
        app.post("/api/cards", card).await;
    }
    app.post("/api/cards", multiple_choice_card(deck_b_id)).await;

    let (status, list) = app.get(&format!("/api/cards?deck_id={deck_a_id}")).await;
    assert_eq!(status, StatusCode::OK);
    let rows = list.as_array().unwrap();
    assert_eq!(rows.len(), 3, "deck B's card must not leak in");

    assert_eq!(rows[0]["prompt_md"], "first");
    assert_eq!(rows[1]["prompt_md"], "second");
    assert_eq!(rows[2]["prompt_md"], "third");

    assert!(rows[0].get("choices").is_none(), "the list carries no children");
    assert!(rows[0].get("accepted").is_none());
}

#[tokio::test]
async fn absent_deck_id_lists_every_deck() {
    let app = common::spawn_app().await;
    let deck_a_id = create_deck(&app, "Deck A").await;
    let deck_b_id = create_deck(&app, "Deck B").await;
    app.post("/api/cards", multiple_choice_card(deck_a_id)).await;
    app.post("/api/cards", multiple_choice_card(deck_b_id)).await;

    let (_, list) = app.get("/api/cards").await;
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn kind_filter_selects_one_kind() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    app.post("/api/cards", multiple_choice_card(deck_id)).await;
    app.post("/api/cards", json!({
        "deck_id": deck_id, "kind": "flashcard", "prompt_md": "p", "answer_md": "a"
    })).await;

    let (_, all) = app.get(&format!("/api/cards?deck_id={deck_id}")).await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    let (_, only) = app.get(&format!("/api/cards?deck_id={deck_id}&kind=flashcard")).await;
    let rows = only.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["kind"], "flashcard");

    let (_, explicit_all) = app.get(&format!("/api/cards?deck_id={deck_id}&kind=all")).await;
    assert_eq!(explicit_all.as_array().unwrap().len(), 2, "kind=all equals absent");
}

#[tokio::test]
async fn bad_query_values_are_rejected_on_their_own_field() {
    let app = common::spawn_app().await;

    let (status, body) = app.get("/api/cards?kind=essay").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "kind");

    let (status, body) = app.get("/api/cards?archived=maybe").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "archived");

    let (status, body) = app.get("/api/cards?deck_id=abc").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "deck_id");
}

#[tokio::test]
async fn archived_filter_defaults_to_excluding_and_all_returns_both() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (_, live) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let live_id = live["id"].as_i64().unwrap();
    let (_, archived) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let archived_id = archived["id"].as_i64().unwrap();

    sqlx::query("UPDATE cards SET archived = 1 WHERE id = ?")
        .bind(archived_id)
        .execute(&app.pool)
        .await
        .unwrap();

    let (_, default_list) = app.get(&format!("/api/cards?deck_id={deck_id}")).await;
    let rows = default_list.as_array().unwrap();
    assert_eq!(rows.len(), 1, "default excludes the archived card");
    assert_eq!(rows[0]["id"], live_id);

    let (_, only_archived) = app.get(&format!("/api/cards?deck_id={deck_id}&archived=true")).await;
    let rows = only_archived.as_array().unwrap();
    assert_eq!(rows.len(), 1, "archived=true returns only the archived card");
    assert_eq!(rows[0]["id"], archived_id);

    let (_, both) = app.get(&format!("/api/cards?deck_id={deck_id}&archived=all")).await;
    assert_eq!(both.as_array().unwrap().len(), 2, "archived=all returns both");
}

#[tokio::test]
async fn patch_replaces_content_and_bumps_updated_at() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = created["id"].as_i64().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let (status, updated) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "mc_single", "prompt_md": "Reworded prompt",
            "explanation_md": "Now explained.",
            "choices": [ { "text_md": "Average", "is_correct": true },
                         { "text_md": "Ward",    "is_correct": false },
                         { "text_md": "Single",  "is_correct": false } ]
        }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["prompt_md"], "Reworded prompt");
    assert_eq!(updated["explanation_md"], "Now explained.");
    assert_eq!(updated["choices"].as_array().unwrap().len(), 3);
    assert_eq!(updated["choices"][0]["text_md"], "Average");
    assert_eq!(updated["choices"][0]["position"], 0, "positions are reassigned");
    assert_eq!(app.count("SELECT COUNT(*) FROM choices").await, 3,
               "the old two rows are gone, not orphaned");
    assert_ne!(updated["updated_at"], created["updated_at"], "updated_at is bumped");
}

#[tokio::test]
async fn changing_kind_clears_the_other_kind_s_children() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = created["id"].as_i64().unwrap();

    let (status, flash) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "flashcard", "prompt_md": "p", "answer_md": "Single linkage"
        }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(flash["kind"], "flashcard");
    assert_eq!(flash["choices"].as_array().unwrap().len(), 0);
    assert_eq!(app.count("SELECT COUNT(*) FROM choices").await, 0,
               "orphaned choices would resurface if the kind changed back");

    let (_, short) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "short_answer", "prompt_md": "p",
            "accepted": [ { "text": "Single-Linkage", "is_primary": true } ]
        }))
        .await;
    assert_eq!(short["accepted"][0]["normalised"], "single linkage");
    assert!(short["answer_md"].is_null(), "the flashcard answer is cleared");
    assert_eq!(app.count("SELECT COUNT(*) FROM accepted").await, 1,
               "exactly the new accepted row, none orphaned");

    let (status, back_to_flash) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "flashcard", "prompt_md": "p", "answer_md": "Single linkage"
        }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(back_to_flash["accepted"].as_array().unwrap().len(), 0);
    assert_eq!(app.count("SELECT COUNT(*) FROM accepted").await, 0,
               "orphaned accepted rows would resurface if the kind changed back");
}

#[tokio::test]
async fn a_rejected_patch_changes_nothing() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = created["id"].as_i64().unwrap();

    let (status, _) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "mc_single", "prompt_md": "Reworded",
            "choices": [ { "text_md": "A", "is_correct": true },
                         { "text_md": "B", "is_correct": true } ]
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, after) = app.get(&format!("/api/cards/{id}")).await;
    assert_eq!(after["prompt_md"], created["prompt_md"], "prompt untouched");
    assert_eq!(after["updated_at"], created["updated_at"], "updated_at untouched");
    assert_eq!(after["choices"].as_array().unwrap().len(), 2);
    assert_eq!(after["choices"][0]["text_md"], "Single", "children untouched");
    assert_eq!(after["choices"][1]["text_md"], "Complete", "children untouched");
    assert_eq!(app.count("SELECT COUNT(*) FROM choices").await, 2,
               "no new rows were ever written");
}

#[tokio::test]
async fn patch_unknown_card_is_404() {
    let app = common::spawn_app().await;
    let (status, _) = app
        .patch("/api/cards/9999", json!({
            "kind": "flashcard", "prompt_md": "p", "answer_md": "a"
        }))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(app.count("SELECT COUNT(*) FROM cards").await, 0,
               "nothing was written for an unknown id");
}

#[tokio::test]
async fn archive_hides_the_card_and_unarchive_restores_it() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = created["id"].as_i64().unwrap();

    let (status, archived) = app.post(&format!("/api/cards/{id}/archive"), json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(archived["archived"], true);
    assert_eq!(app.count(&format!("SELECT COUNT(*) FROM cards WHERE id = {id}")).await, 1,
               "archived, not deleted");

    let (_, default_list) = app.get(&format!("/api/cards?deck_id={deck_id}")).await;
    assert_eq!(default_list.as_array().unwrap().len(), 0);

    let (_, archived_list) = app
        .get(&format!("/api/cards?deck_id={deck_id}&archived=true")).await;
    assert_eq!(archived_list.as_array().unwrap().len(), 1);

    let (_, all) = app.get(&format!("/api/cards?deck_id={deck_id}&archived=all")).await;
    assert_eq!(all.as_array().unwrap().len(), 1);

    let (status, _) = app.post(&format!("/api/cards/{id}/archive"), json!({})).await;
    assert_eq!(status, StatusCode::OK);

    let (_, restored) = app.post(&format!("/api/cards/{id}/unarchive"), json!({})).await;
    assert_eq!(restored["archived"], false);
    let (_, back) = app.get(&format!("/api/cards?deck_id={deck_id}")).await;
    assert_eq!(back.as_array().unwrap().len(), 1);
    assert_eq!(app.count(&format!("SELECT COUNT(*) FROM cards WHERE id = {id}")).await, 1,
               "still exists after restore");
}

#[tokio::test]
async fn archiving_an_unknown_card_is_404() {
    let app = common::spawn_app().await;
    let (status, _) = app.post("/api/cards/9999/archive", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = app.post("/api/cards/9999/unarchive", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn archived_cards_do_not_count_toward_a_deck_s_card_count() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = created["id"].as_i64().unwrap();

    let (_, before) = app.get("/api/decks?module_id=all").await;
    assert_eq!(before[0]["card_count"], 1);

    app.post(&format!("/api/cards/{id}/archive"), json!({})).await;
    let (_, after) = app.get("/api/decks?module_id=all").await;
    assert_eq!(after[0]["card_count"], 0, "the decks query already filters archived = 0");
}

#[tokio::test]
async fn a_deck_s_card_count_reflects_created_cards() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let (_, decks) = app.get("/api/decks").await;
    assert_eq!(decks[0]["card_count"], 1);
}

#[tokio::test]
async fn archiving_never_touches_reviews_history() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = created["id"].as_i64().unwrap();

    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO sessions (mode, deck_ids) VALUES ('practice', '[]') RETURNING id"
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO reviews (card_id, session_id, correct) VALUES (?, ?, 1)")
        .bind(id)
        .bind(session_id)
        .execute(&app.pool)
        .await
        .unwrap();

    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 1);
    app.post(&format!("/api/cards/{id}/archive"), json!({})).await;
    assert_eq!(app.count("SELECT COUNT(*) FROM reviews").await, 1, "archive leaves reviews alone");
    app.post(&format!("/api/cards/{id}/unarchive"), json!({})).await;
    assert_eq!(
        app.count("SELECT COUNT(*) FROM reviews").await, 1,
        "unarchive leaves reviews alone",
    );
}

const UPLOADED: &str = "images/0123456789abcdef.png";

#[tokio::test]
async fn image_path_round_trips_through_create_and_get() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Clustering").await;

    let (status, card) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "short_answer",
            "prompt_md": "Name the linkage shown in the dendrogram.",
            "image_path": UPLOADED,
            "accepted": [{ "text": "single linkage", "is_primary": true }]
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(card["image_path"], UPLOADED);

    let (_, fetched) = app.get(&format!("/api/cards/{}", card["id"].as_i64().unwrap())).await;
    assert_eq!(fetched["image_path"], UPLOADED, "the stored path survives a re-read");
}

#[tokio::test]
async fn a_patch_omitting_image_path_clears_it() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Clustering").await;

    let (_, card) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard", "prompt_md": "Define linkage.",
            "answer_md": "How inter-cluster distance is measured.",
            "image_path": UPLOADED
        }))
        .await;
    let id = card["id"].as_i64().unwrap();
    assert_eq!(card["image_path"], UPLOADED);

    let (status, patched) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "flashcard", "prompt_md": "Define linkage.",
            "answer_md": "How inter-cluster distance is measured."
        }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(patched["image_path"].is_null(), "an omitted image_path must clear it");
}

#[tokio::test]
async fn a_patch_can_set_an_image_on_a_card_that_had_none() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Clustering").await;
    let (_, card) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let id = card["id"].as_i64().unwrap();
    assert!(card["image_path"].is_null());

    let (status, patched) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "mc_single",
            "prompt_md": "Which linkage merges the two closest points?",
            "image_path": "images/fedcba9876543210.webp",
            "choices": [
                { "text_md": "Single",   "is_correct": true  },
                { "text_md": "Complete", "is_correct": false }
            ]
        }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patched["image_path"], "images/fedcba9876543210.webp");
}

#[tokio::test]
async fn rejects_an_image_path_this_server_did_not_issue() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Clustering").await;

    let rejected = [
        ("../../etc/passwd", "escapes the directory"),
        ("images/../test.db", "traverses out of images/"),
        ("images/short.png", "stem is not 16 characters"),
        ("images/abcdef.png", "hex but only 6 characters"),
        ("images/0123456789abcdef00.png", "hex but 18 characters"),
        ("images/0123456789abcdeg.png", "g is not hex"),
        ("images/0123456789ABCDEF.png", "uppercase hex is not what we emit"),
        ("images/0123456789abcdef.gif", "gif is not an accepted type"),
        ("images/0123456789abcdef", "no extension"),
        ("http://example.com/x.png", "not a local path at all"),
        ("uploads/0123456789abcdef.png", "wrong directory"),
    ];

    for (path, why) in rejected {
        let (status, body) = app
            .post("/api/cards", json!({
                "deck_id": deck_id, "kind": "flashcard", "prompt_md": "Q", "answer_md": "A",
                "image_path": path
            }))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path} ({why}) was accepted");
        assert!(
            body["fields"].as_array().unwrap().iter()
                .any(|field| field["field"] == "image_path"),
            "{path} ({why}) was rejected but not against image_path: {body}",
        );
    }

    assert_eq!(app.count("SELECT COUNT(*) FROM cards").await, 0, "none were written");
}

#[tokio::test]
async fn an_empty_image_path_is_treated_as_absent() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Clustering").await;

    let (status, card) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard", "prompt_md": "Q", "answer_md": "A",
            "image_path": ""
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "an empty string is 'no image', not an error");
    assert!(card["image_path"].is_null());
}

#[tokio::test]
async fn create_appends_at_the_end_of_the_deck() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let first_card_id = create_flashcard(&app, deck_id, "first").await;
    let second_card_id = create_flashcard(&app, deck_id, "second").await;
    let third_card_id = create_flashcard(&app, deck_id, "third").await;

    assert_eq!(
        card_ids_in_order(&app, deck_id).await,
        vec![first_card_id, second_card_id, third_card_id],
    );
    assert_eq!(card_positions(&app, deck_id).await, vec![0, 1, 2], "0-based and dense");
}

#[tokio::test]
async fn positions_are_per_deck() {
    let app = common::spawn_app().await;
    let first_deck_id = create_deck(&app, "Deck one").await;
    let second_deck_id = create_deck(&app, "Deck two").await;

    create_flashcard(&app, first_deck_id, "one").await;
    create_flashcard(&app, second_deck_id, "two").await;
    create_flashcard(&app, first_deck_id, "three").await;

    assert_eq!(card_positions(&app, first_deck_id).await, vec![0, 1]);
    assert_eq!(
        card_positions(&app, second_deck_id).await, vec![0],
        "a second deck starts again at 0",
    );
}

#[tokio::test]
async fn a_client_supplied_position_is_rejected() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let (status, _) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard",
            "prompt_md": "q", "answer_md": "a", "position": 7,
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn archiving_does_not_renumber_positions() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;

    let first_card_id = create_flashcard(&app, deck_id, "first").await;
    let second_card_id = create_flashcard(&app, deck_id, "second").await;
    let third_card_id = create_flashcard(&app, deck_id, "third").await;

    app.post(&format!("/api/cards/{second_card_id}/archive"), json!({})).await;
    assert_eq!(
        card_ids_in_order(&app, deck_id).await,
        vec![first_card_id, second_card_id, third_card_id],
        "b keeps its slot while archived",
    );
    assert_eq!(card_positions(&app, deck_id).await, vec![0, 1, 2]);

    app.post(&format!("/api/cards/{second_card_id}/unarchive"), json!({})).await;
    assert_eq!(
        card_ids_in_order(&app, deck_id).await,
        vec![first_card_id, second_card_id, third_card_id],
        "and returns to it",
    );
}

#[tokio::test]
async fn moves_a_card_to_the_front() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let first_card_id = create_flashcard(&app, deck_id, "first").await;
    let second_card_id = create_flashcard(&app, deck_id, "second").await;
    let third_card_id = create_flashcard(&app, deck_id, "third").await;

    let (status, _) = app
        .post(
            &format!("/api/cards/{third_card_id}/move"),
            json!({ "before": first_card_id }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        card_ids_in_order(&app, deck_id).await,
        vec![third_card_id, first_card_id, second_card_id],
    );
    assert_eq!(card_positions(&app, deck_id).await, vec![0, 1, 2]);
}

#[tokio::test]
async fn moves_a_card_to_the_middle() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let first_card_id = create_flashcard(&app, deck_id, "first").await;
    let second_card_id = create_flashcard(&app, deck_id, "second").await;
    let third_card_id = create_flashcard(&app, deck_id, "third").await;

    app.post(&format!("/api/cards/{first_card_id}/move"), json!({ "before": third_card_id })).await;
    assert_eq!(
        card_ids_in_order(&app, deck_id).await,
        vec![second_card_id, first_card_id, third_card_id],
    );
}

#[tokio::test]
async fn a_null_before_moves_a_card_to_the_end() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let first_card_id = create_flashcard(&app, deck_id, "first").await;
    let second_card_id = create_flashcard(&app, deck_id, "second").await;
    let third_card_id = create_flashcard(&app, deck_id, "third").await;

    let (status, moved) = app
        .post(&format!("/api/cards/{first_card_id}/move"), json!({ "before": null }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moved["position"], 2, "the response carries the new position");
    assert_eq!(
        card_ids_in_order(&app, deck_id).await,
        vec![second_card_id, third_card_id, first_card_id],
    );
}

#[tokio::test]
async fn positions_stay_dense_across_a_sequence_of_moves() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let first_card_id = create_flashcard(&app, deck_id, "first").await;
    let second_card_id = create_flashcard(&app, deck_id, "second").await;
    let third_card_id = create_flashcard(&app, deck_id, "third").await;
    let fourth_card_id = create_flashcard(&app, deck_id, "fourth").await;

    app.post(
        &format!("/api/cards/{fourth_card_id}/move"),
        json!({ "before": first_card_id }),
    ).await;
    app.post(&format!("/api/cards/{first_card_id}/move"), json!({ "before": null })).await;
    app.post(
        &format!("/api/cards/{third_card_id}/move"),
        json!({ "before": second_card_id }),
    ).await;

    assert_eq!(
        card_ids_in_order(&app, deck_id).await,
        vec![fourth_card_id, third_card_id, second_card_id, first_card_id],
    );
    assert_eq!(card_positions(&app, deck_id).await, vec![0, 1, 2, 3]);
}

#[tokio::test]
async fn moving_before_a_card_in_another_deck_is_rejected() {
    let app = common::spawn_app().await;
    let first_deck_id = create_deck(&app, "Deck one").await;
    let second_deck_id = create_deck(&app, "Deck two").await;
    let first_card_id = create_flashcard(&app, first_deck_id, "mine").await;
    let second_card_id = create_flashcard(&app, first_deck_id, "also mine").await;
    let outsider = create_flashcard(&app, second_deck_id, "elsewhere").await;

    let (status, body) = app
        .post(&format!("/api/cards/{first_card_id}/move"), json!({ "before": outsider }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "before");
    assert_eq!(
        card_ids_in_order(&app, first_deck_id).await,
        vec![first_card_id, second_card_id],
        "a rejected move writes nothing",
    );
}

#[tokio::test]
async fn moving_before_a_nonexistent_card_is_rejected() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let first_card_id = create_flashcard(&app, deck_id, "only").await;

    let (status, body) = app
        .post(&format!("/api/cards/{first_card_id}/move"), json!({ "before": 99_999 }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "before");
}

#[tokio::test]
async fn moving_a_card_before_itself_is_rejected() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let first_card_id = create_flashcard(&app, deck_id, "only").await;

    let (status, body) = app
        .post(
            &format!("/api/cards/{first_card_id}/move"),
            json!({ "before": first_card_id }),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "before");
}

#[tokio::test]
async fn moving_a_nonexistent_card_is_404() {
    let app = common::spawn_app().await;
    let (status, _) = app.post("/api/cards/99999/move", json!({ "before": null })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_move_does_not_bump_updated_at() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let first_card_id = create_flashcard(&app, deck_id, "first").await;
    let second_card_id = create_flashcard(&app, deck_id, "second").await;

    let (_, before_move) = app.get(&format!("/api/cards/{first_card_id}")).await;

    let (_, moved) = app
        .post(&format!("/api/cards/{first_card_id}/move"), json!({ "before": null }))
        .await;
    assert_eq!(moved["updated_at"], before_move["updated_at"]);
    assert_eq!(card_ids_in_order(&app, deck_id).await, vec![second_card_id, first_card_id]);
}

#[tokio::test]
async fn a_move_keeps_interleaved_archived_cards_in_place() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let first_card_id = create_flashcard(&app, deck_id, "visible one").await;
    let hidden = create_flashcard(&app, deck_id, "archived").await;
    let third_card_id = create_flashcard(&app, deck_id, "visible two").await;
    app.post(&format!("/api/cards/{hidden}/archive"), json!({})).await;

    app.post(&format!("/api/cards/{third_card_id}/move"), json!({ "before": first_card_id })).await;
    assert_eq!(card_ids_in_order(&app, deck_id).await, vec![third_card_id, first_card_id, hidden]);
    assert_eq!(card_positions(&app, deck_id).await, vec![0, 1, 2]);
}

#[tokio::test]
async fn deleting_a_card_row_cascades_to_its_reviews() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let card_id = create_flashcard(&app, deck_id, "what is k-means").await;
    let review_id = record_review(&app, deck_id, card_id).await;

    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM reviews WHERE id = {review_id}")).await,
        1,
        "the review must exist before the delete, or the cascade proves nothing",
    );

    sqlx::query("DELETE FROM cards WHERE id = ?")
        .bind(card_id)
        .execute(&app.pool)
        .await
        .expect("the delete must succeed rather than be refused by a foreign key");

    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM reviews WHERE id = {review_id}")).await,
        0,
        "the card's review must be gone with it",
    );
}

#[tokio::test]
async fn deletes_a_multiple_choice_card_with_its_choices_schedule_and_reviews() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (_, card) = app.post("/api/cards", multiple_choice_card(deck_id)).await;
    let card_id = card["id"].as_i64().unwrap();
    let survivor_id = create_flashcard(&app, deck_id, "untouched").await;
    record_review(&app, deck_id, card_id).await;
    sqlx::query("UPDATE schedule SET due_at = '2026-01-01T00:00:00Z' WHERE card_id = ?")
        .bind(card_id)
        .execute(&app.pool)
        .await
        .unwrap();

    for (table, expected) in [("choices", 2), ("schedule", 1), ("reviews", 1)] {
        assert_eq!(
            app.count(&format!("SELECT COUNT(*) FROM {table} WHERE card_id = {card_id}")).await,
            expected,
            "{table} must be populated before the delete, or its assertion proves nothing",
        );
    }

    let (status, body) = app.delete(&format!("/api/cards/{card_id}")).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "body was {body}");
    assert_eq!(body, Value::Null, "204 must carry no body");

    for table in ["choices", "schedule", "reviews"] {
        assert_eq!(
            app.count(&format!("SELECT COUNT(*) FROM {table} WHERE card_id = {card_id}")).await,
            0,
            "{table} rows survived the delete",
        );
    }
    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM cards WHERE id = {card_id}")).await,
        0,
    );
    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM cards WHERE id = {survivor_id}")).await,
        1,
        "the other card in the deck must be untouched",
    );
}

#[tokio::test]
async fn deletes_a_short_answer_card_with_its_accepted_answers() {
    let app = common::spawn_app().await;
    let deck_id = create_deck(&app, "Test 1").await;
    let (created_status, card) = app.post("/api/cards", short_answer_card(deck_id)).await;
    assert_eq!(created_status, StatusCode::CREATED, "setup failed: {card}");
    let card_id = card["id"].as_i64().unwrap();

    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM accepted WHERE card_id = {card_id}")).await,
        1,
        "accepted must be populated before the delete",
    );

    let (status, _) = app.delete(&format!("/api/cards/{card_id}")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_eq!(
        app.count(&format!("SELECT COUNT(*) FROM accepted WHERE card_id = {card_id}")).await,
        0,
    );
}

#[tokio::test]
async fn deleting_an_unknown_card_is_404() {
    let app = common::spawn_app().await;
    let (status, body) = app.delete("/api/cards/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}
