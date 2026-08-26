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
async fn short_answer_needs_at_least_one_accepted_answer() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "short_answer", "prompt_md": "p",
            "accepted": []
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "accepted");
}

#[tokio::test]
async fn empty_accepted_text_names_its_row() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (status, body) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "short_answer", "prompt_md": "p",
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
async fn a_rejected_create_writes_nothing() {
    // NB despite the name this only proves that no write is *attempted* for
    // a body that fails `validate` — `validate` runs before `st.pool.begin()`,
    // so a validation failure never opens a transaction at all. It cannot
    // distinguish "nothing was ever written" from "a partial write was rolled
    // back", because there is currently no reachable path where a child or
    // schedule insert fails after the card row has already been written
    // (all child data is validated up front). The transaction's rollback
    // path is real and still worth having for future code paths, but it is
    // not exercised by this test.
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

    let (_, list) = app.get(&format!("/api/cards?deck_id={d}")).await;
    assert_eq!(list.as_array().unwrap().len(), 0, "no partial card row");
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

    let (status, _) = app.get("/api/cards/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Discriminates `ORDER BY position` from `ORDER BY id` / natural rowid
/// order. For a freshly-created card the two orderings coincide (position is
/// assigned from array index at insert, so id and position are
/// co-monotonic), so this inserts an extra choice directly against the pool
/// with a LOWER position than the existing rows but, being inserted last, a
/// HIGHER autoincrement id. Under `ORDER BY position` it sorts first; under
/// id/rowid order it would sort last. That inversion is the whole point —
/// don't "simplify" this fixture back to sequential ids and positions.
#[tokio::test]
async fn choices_come_back_in_position_order_not_id_order() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", mc(d)).await;
    let id = created["id"].as_i64().unwrap();

    // The two choices from `mc()` already have id/position 0/0 and 1/1.
    // Insert a third with the highest id but position -1, so it must sort
    // first under `ORDER BY position` and last under `ORDER BY id`.
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
    let a = deck(&app, "Deck A").await;
    let b = deck(&app, "Deck B").await;

    for prompt in ["first", "second", "third"] {
        let mut card = mc(a);
        card["prompt_md"] = json!(prompt);
        app.post("/api/cards", card).await;
    }
    app.post("/api/cards", mc(b)).await;

    let (status, list) = app.get(&format!("/api/cards?deck_id={a}")).await;
    assert_eq!(status, StatusCode::OK);
    let rows = list.as_array().unwrap();
    assert_eq!(rows.len(), 3, "deck B's card must not leak in");

    // All three land in the same second, so this asserts the id tiebreak, not
    // created_at. Without `id ASC` the order is SQLite's incidental scan order.
    assert_eq!(rows[0]["prompt_md"], "first");
    assert_eq!(rows[1]["prompt_md"], "second");
    assert_eq!(rows[2]["prompt_md"], "third");

    assert!(rows[0].get("choices").is_none(), "the list carries no children");
    assert!(rows[0].get("accepted").is_none());
}

#[tokio::test]
async fn absent_deck_id_lists_every_deck() {
    let app = common::spawn_app().await;
    let a = deck(&app, "Deck A").await;
    let b = deck(&app, "Deck B").await;
    app.post("/api/cards", mc(a)).await;
    app.post("/api/cards", mc(b)).await;

    let (_, list) = app.get("/api/cards").await;
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn kind_filter_selects_one_kind() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    app.post("/api/cards", mc(d)).await;
    app.post("/api/cards", json!({
        "deck_id": d, "kind": "flashcard", "prompt_md": "p", "answer_md": "a"
    })).await;

    let (_, all) = app.get(&format!("/api/cards?deck_id={d}")).await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    let (_, only) = app.get(&format!("/api/cards?deck_id={d}&kind=flashcard")).await;
    let rows = only.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["kind"], "flashcard");

    let (_, explicit_all) = app.get(&format!("/api/cards?deck_id={d}&kind=all")).await;
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
    // No archive endpoint exists yet (Task 5), so the archived flag is set
    // directly against the pool — the same pattern `count`/`schedule_for`
    // already use to reach tables/columns the HTTP surface doesn't expose.
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let (_, live) = app.post("/api/cards", mc(d)).await;
    let live_id = live["id"].as_i64().unwrap();
    let (_, archived) = app.post("/api/cards", mc(d)).await;
    let archived_id = archived["id"].as_i64().unwrap();

    sqlx::query("UPDATE cards SET archived = 1 WHERE id = ?")
        .bind(archived_id)
        .execute(&app.pool)
        .await
        .unwrap();

    let (_, default_list) = app.get(&format!("/api/cards?deck_id={d}")).await;
    let rows = default_list.as_array().unwrap();
    assert_eq!(rows.len(), 1, "default excludes the archived card");
    assert_eq!(rows[0]["id"], live_id);

    let (_, only_archived) = app.get(&format!("/api/cards?deck_id={d}&archived=true")).await;
    let rows = only_archived.as_array().unwrap();
    assert_eq!(rows.len(), 1, "archived=true returns only the archived card");
    assert_eq!(rows[0]["id"], archived_id);

    let (_, both) = app.get(&format!("/api/cards?deck_id={d}&archived=all")).await;
    assert_eq!(both.as_array().unwrap().len(), 2, "archived=all returns both");
}
