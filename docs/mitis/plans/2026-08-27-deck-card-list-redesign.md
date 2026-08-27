# Deck Card List Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use mitis:subagent-driven-development (recommended) or mitis:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `/decks/:id` as a deck of flippable cards with persisted drag-to-reorder, a two-column multiple-choice grid, and a reveal/hide toggle for the correct answer.

**Architecture:** A new `position` column on `cards` gives each deck a persisted order, rewritten wholesale by one relative-move endpoint (`POST /api/cards/:id/move` with `{"before": id|null}`). The frontend list becomes a `@dnd-kit/sortable` list of `CardRow`s; each row owns a half-flip state machine (rotate to edge-on, swap the single mounted face, rotate back) and fetches its own answer detail on first flip.

**Tech Stack:** Rust/axum/sqlx (SQLite), React 19 + TypeScript + Vite, Tailwind v4 with the existing Bibble theme tokens, shadcn/Radix primitives, `@dnd-kit/core` + `@dnd-kit/sortable` + `@dnd-kit/utilities` (new).

**Spec:** [`../specs/2026-08-27-deck-card-list-redesign-design.md`](../specs/2026-08-27-deck-card-list-redesign-design.md)

**User decisions (already made):**
- Reorder is persisted properly — migration, backfill, endpoint, ordered list. Not visual-only.
- The card back is fetched on first flip via the existing `GET /api/cards/:id`. `GET /api/cards` is *not* extended to carry children.
- Multiple-choice answers are **hidden by default** on the back; the eye button reveals. Icon shows the action.
- The image thumbnail **keeps its lightbox** and does not flip (`stopPropagation`) — a deliberate non-flipping region inside the card body.
- Drag and drop uses `@dnd-kit/sortable` rather than hand-rolled, for variable-height rows and free keyboard reorder.
- Mockup colours are ignored; the existing theme tokens supply the palette.

---

## File Structure

| Path | Responsibility |
|---|---|
| `backend/migrations/0002_card_position.sql` | **new** — add `position`, backfill from creation order, index |
| `backend/src/routes/cards.rs` | `position` on the DTO, list ordering, create assignment, the `move` handler and its route |
| `backend/tests/cards.rs` | ordering + move endpoint coverage |
| `frontend/src/lib/api.ts` | `position` on `CardSummary`, `api.moveCard` |
| `frontend/src/components/deck/useFlip.ts` | **new** — the half-flip state machine, nothing else |
| `frontend/src/components/deck/CardBack.tsx` | **new** — per-kind back face, MC grid, reveal styling |
| `frontend/src/components/deck/CardRow.tsx` | **new** — one sortable row: grip, header strip, flip target, detail fetch |
| `frontend/src/pages/DeckPage.tsx` | page header, `DndContext`, optimistic reorder; the row body moves out to `CardRow` |
| `docs/HANDOVER.md` | record what shipped |

`DeckPage` carries the whole list inline today. Once a row has a header strip, two faces, a flip
state machine and its own fetch, that is three responsibilities in one file — hence the split into
`CardRow` (row mechanics) and `CardBack` (what an answer looks like). No other refactoring is in
scope.

---

## Task 1: `position` column, ordering, and create assignment

**Goal:** Every card carries a dense per-deck `position`, the list is ordered by it, and existing decks read unchanged after the migration.

**Files:**
- Create: `backend/migrations/0002_card_position.sql`
- Modify: `backend/src/routes/cards.rs` (`CardSummaryDto`, `fetch_summary`, `list`, `create`)
- Test: `backend/tests/cards.rs`

**Acceptance Criteria:**
- [ ] `GET /api/cards` returns cards ordered by `position ASC, id ASC`
- [ ] Every card row in the response carries a `position` integer
- [ ] A new card lands at the end of its own deck; positions are independent per deck
- [ ] Positions are 0-based and dense within a deck
- [ ] `POST`/`PATCH /api/cards` reject a client-supplied `position` (`deny_unknown_fields` already does this — a test pins it)
- [ ] The dev database migrates without changing the order any deck reads in

**Verify:** `cargo test --test cards` → all pass, including the four new tests

**Steps:**

- [ ] **Step 1: Write the migration**

Create `backend/migrations/0002_card_position.sql`:

```sql
-- Cards carry an explicit per-deck order so the deck screen can be reordered
-- by hand. Backfilled from `created_at ASC, id ASC` — the ordering the list
-- query used before this column existed — so every existing deck reads exactly
-- as it did before the migration.
--
-- Archived cards occupy positions like any other: one ordering per deck is
-- simpler than two views that can disagree, and it means un-archiving a card
-- returns it to where it was.
ALTER TABLE cards ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

UPDATE cards SET position = (
  SELECT COUNT(*) FROM cards c2
   WHERE c2.deck_id = cards.deck_id
     AND (c2.created_at < cards.created_at
          OR (c2.created_at = cards.created_at AND c2.id < cards.id))
);

-- Ordering only. Deliberately NOT unique on (deck_id, position): a whole-deck
-- renumber assigns positions row by row and would trip a unique constraint
-- part-way through the transaction.
CREATE INDEX idx_cards_deck_position ON cards(deck_id, position);
```

- [ ] **Step 2: Write the failing tests**

Add to the top of `backend/tests/cards.rs`, below the existing `mc` helper:

```rust
/// A minimal flashcard, for tests that care about ordering rather than content.
async fn flash(app: &common::TestApp, deck_id: i64, prompt: &str) -> i64 {
    let (status, c) = app
        .post("/api/cards", json!({
            "deck_id": deck_id, "kind": "flashcard",
            "prompt_md": prompt, "answer_md": "an answer",
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "helper card create failed: {c}");
    c["id"].as_i64().unwrap()
}

/// The deck's card ids in list order, archived included.
async fn order(app: &common::TestApp, deck_id: i64) -> Vec<i64> {
    let (_, rows) = app.get(&format!("/api/cards?deck_id={deck_id}&archived=all")).await;
    rows.as_array().unwrap().iter().map(|c| c["id"].as_i64().unwrap()).collect()
}

/// The deck's positions in list order — asserts density as well as order.
async fn positions(app: &common::TestApp, deck_id: i64) -> Vec<i64> {
    let (_, rows) = app.get(&format!("/api/cards?deck_id={deck_id}&archived=all")).await;
    rows.as_array().unwrap().iter().map(|c| c["position"].as_i64().unwrap()).collect()
}
```

Then add these tests at the end of the file:

```rust
#[tokio::test]
async fn create_appends_at_the_end_of_the_deck() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let a = flash(&app, d, "first").await;
    let b = flash(&app, d, "second").await;
    let c = flash(&app, d, "third").await;

    assert_eq!(order(&app, d).await, vec![a, b, c]);
    assert_eq!(positions(&app, d).await, vec![0, 1, 2], "0-based and dense");
}

#[tokio::test]
async fn positions_are_per_deck() {
    let app = common::spawn_app().await;
    let d1 = deck(&app, "Deck one").await;
    let d2 = deck(&app, "Deck two").await;

    flash(&app, d1, "one").await;
    flash(&app, d2, "two").await;
    flash(&app, d1, "three").await;

    assert_eq!(positions(&app, d1).await, vec![0, 1]);
    assert_eq!(positions(&app, d2).await, vec![0], "a second deck starts again at 0");
}

#[tokio::test]
async fn a_client_supplied_position_is_rejected() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    // position is server-assigned, like choices.position and
    // accepted.normalised. deny_unknown_fields on CardInput is what enforces
    // it; this test pins that so a future #[serde(default)] cannot open a hole.
    let (status, _) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "flashcard",
            "prompt_md": "q", "answer_md": "a", "position": 7,
        }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn archiving_does_not_renumber_positions() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;

    let a = flash(&app, d, "first").await;
    let b = flash(&app, d, "second").await;
    let c = flash(&app, d, "third").await;

    app.post(&format!("/api/cards/{b}/archive"), json!({})).await;
    assert_eq!(order(&app, d).await, vec![a, b, c], "b keeps its slot while archived");
    assert_eq!(positions(&app, d).await, vec![0, 1, 2]);

    app.post(&format!("/api/cards/{b}/unarchive"), json!({})).await;
    assert_eq!(order(&app, d).await, vec![a, b, c], "and returns to it");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

```bash
cargo test --test cards
```

Expected: compile error — `position` is not a field of the JSON response / no such column.

- [ ] **Step 4: Add `position` to the DTO and both read queries**

In `backend/src/routes/cards.rs`, add the field to `CardSummaryDto` (after `archived`):

```rust
pub struct CardSummaryDto {
    pub id: i64,
    pub deck_id: i64,
    pub kind: String,
    pub prompt_md: String,
    pub image_path: Option<String>,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub archived: bool,
    /// Order within the deck: 0-based, dense, archived cards included.
    /// Server-assigned — `CardInput` does not accept it.
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}
```

Update `fetch_summary`'s SELECT:

```rust
        r#"SELECT id AS "id!: i64", deck_id AS "deck_id!: i64", kind,
                  prompt_md, image_path, answer_md, explanation_md,
                  archived AS "archived!: bool", position AS "position!: i64",
                  created_at, updated_at
           FROM cards WHERE id = ?"#,
```

Update `list`'s SELECT — the same column list, and the new `ORDER BY`. Replace the existing
`ORDER BY created_at ASC, id ASC` comment block with this one:

```rust
    // Hand-ordered: `position` is dense and 0-based per deck (see migration
    // 0002), and `POST /api/cards/:id/move` rewrites the whole deck's
    // positions in one transaction, so there are no ties to break in practice.
    // `id ASC` is kept as a determinism guarantee anyway — the same reasoning
    // that kept it behind created_at before this column existed.
    let rows = sqlx::query_as!(
        CardSummaryDto,
        r#"SELECT id AS "id!: i64", deck_id AS "deck_id!: i64", kind,
                  prompt_md, image_path, answer_md, explanation_md,
                  archived AS "archived!: bool", position AS "position!: i64",
                  created_at, updated_at
           FROM cards
           WHERE (? IS NULL OR deck_id = ?)
             AND (? = 'all' OR kind = ?)
             AND (? = 'all'
                  OR (? = 'true'  AND archived = 1)
                  OR (? = 'false' AND archived = 0))
           ORDER BY position ASC, id ASC"#,
        deck_id, deck_id, kind, kind, archived, archived, archived
    )
```

- [ ] **Step 5: Assign `position` on create**

In `create`, immediately after `let mut tx = st.pool.begin().await?;` and before the card INSERT:

```rust
    // End of the deck. A nonexistent deck yields 0 here and then fails on the
    // INSERT's foreign key below, so the 400 for a bad deck_id is unchanged.
    let position = sqlx::query_scalar!(
        r#"SELECT COALESCE(MAX(position), -1) + 1 AS "next!: i64"
           FROM cards WHERE deck_id = ?"#,
        deck_id
    )
    .fetch_one(&mut *tx)
    .await?;
```

Then extend the INSERT:

```rust
    let id = sqlx::query_scalar!(
        r#"INSERT INTO cards (deck_id, kind, prompt_md, image_path, answer_md,
                              explanation_md, position)
           VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id AS "id!: i64""#,
        deck_id, valid.kind, valid.prompt_md, valid.image_path, valid.answer_md,
        valid.explanation_md, position
    )
```

- [ ] **Step 6: Regenerate the sqlx offline cache and run the tests**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo sqlx migrate run --source backend/migrations
cargo sqlx prepare --workspace
cargo test --test cards
```

Both sqlx commands run **from the repo root**, never from `backend/`. Expected: all `cards` tests
pass.

- [ ] **Step 7: Confirm the dev database migrates without reordering anything**

The backfill cannot be covered by an integration test — `spawn_app` builds a fresh database with
every migration already applied, so there are never pre-existing rows for 0002 to backfill. Check
it by hand against the real database instead (the `migrate run` in Step 6 already applied it):

```bash
sqlite3 data/quizapp.db \
  "SELECT deck_id, position, id, created_at FROM cards ORDER BY deck_id, position;"
```

Expected: within each `deck_id`, `position` runs 0,1,2,… with no gaps or repeats, and `created_at`
is non-decreasing down each deck's block. If a deck shows a gap, the backfill is wrong — stop and
fix it rather than continuing.

- [ ] **Step 8: Run the full gate and commit**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
git add backend/migrations/0002_card_position.sql backend/src/routes/cards.rs \
        backend/tests/cards.rs .sqlx
git commit -m "feat(cards): per-deck position, backfilled from creation order

Adds a dense 0-based position per deck, backfilled from the created_at
ordering the list query used before, so existing decks read unchanged.
Archived cards keep their slots. New cards append to the end of their deck.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 2: `POST /api/cards/:id/move`

**Goal:** One relative-move endpoint that reorders a deck, validated, transactional, and without disturbing `updated_at`.

**Files:**
- Modify: `backend/src/routes/cards.rs` (`MoveCard`, `move_card`, `router`)
- Test: `backend/tests/cards.rs`

**Acceptance Criteria:**
- [ ] `POST /api/cards/:id/move` with `{"before": <id>}` places the card immediately before that card
- [ ] `{"before": null}` moves it to the end of its deck
- [ ] Positions remain dense and 0-based after any sequence of moves
- [ ] `before` naming a card in another deck, or a nonexistent card → 422 with field `before`
- [ ] `before` equal to the moved card's own id → 422 with field `before`
- [ ] A nonexistent card id → 404, and nothing is written
- [ ] The card's `updated_at` is unchanged by a move
- [ ] Archived cards interleaved in the deck keep their relative order across a move

**Verify:** `cargo test --test cards` → all pass, including the eight new tests

**Steps:**

- [ ] **Step 1: Write the failing tests**

Add at the end of `backend/tests/cards.rs`. These use the `flash`, `order` and `positions` helpers
added in Task 1.

```rust
#[tokio::test]
async fn moves_a_card_to_the_front() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let a = flash(&app, d, "first").await;
    let b = flash(&app, d, "second").await;
    let c = flash(&app, d, "third").await;

    let (status, _) = app.post(&format!("/api/cards/{c}/move"), json!({ "before": a })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(order(&app, d).await, vec![c, a, b]);
    assert_eq!(positions(&app, d).await, vec![0, 1, 2]);
}

#[tokio::test]
async fn moves_a_card_to_the_middle() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let a = flash(&app, d, "first").await;
    let b = flash(&app, d, "second").await;
    let c = flash(&app, d, "third").await;

    // a lands immediately before c, i.e. between b and c.
    app.post(&format!("/api/cards/{a}/move"), json!({ "before": c })).await;
    assert_eq!(order(&app, d).await, vec![b, a, c]);
}

#[tokio::test]
async fn a_null_before_moves_a_card_to_the_end() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let a = flash(&app, d, "first").await;
    let b = flash(&app, d, "second").await;
    let c = flash(&app, d, "third").await;

    let (status, moved) = app
        .post(&format!("/api/cards/{a}/move"), json!({ "before": null }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moved["position"], 2, "the response carries the new position");
    assert_eq!(order(&app, d).await, vec![b, c, a]);
}

#[tokio::test]
async fn positions_stay_dense_across_a_sequence_of_moves() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let a = flash(&app, d, "first").await;
    let b = flash(&app, d, "second").await;
    let c = flash(&app, d, "third").await;
    let e = flash(&app, d, "fourth").await;

    app.post(&format!("/api/cards/{e}/move"), json!({ "before": a })).await;
    app.post(&format!("/api/cards/{a}/move"), json!({ "before": null })).await;
    app.post(&format!("/api/cards/{c}/move"), json!({ "before": b })).await;

    assert_eq!(order(&app, d).await, vec![e, c, b, a]);
    assert_eq!(positions(&app, d).await, vec![0, 1, 2, 3]);
}

#[tokio::test]
async fn moving_before_a_card_in_another_deck_is_rejected() {
    let app = common::spawn_app().await;
    let d1 = deck(&app, "Deck one").await;
    let d2 = deck(&app, "Deck two").await;
    let a = flash(&app, d1, "mine").await;
    let b = flash(&app, d1, "also mine").await;
    let outsider = flash(&app, d2, "elsewhere").await;

    let (status, body) = app
        .post(&format!("/api/cards/{a}/move"), json!({ "before": outsider }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "before");
    assert_eq!(order(&app, d1).await, vec![a, b], "a rejected move writes nothing");
}

#[tokio::test]
async fn moving_before_a_nonexistent_card_is_rejected() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let a = flash(&app, d, "only").await;

    let (status, body) = app
        .post(&format!("/api/cards/{a}/move"), json!({ "before": 99_999 }))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "before");
}

#[tokio::test]
async fn moving_a_card_before_itself_is_rejected() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let a = flash(&app, d, "only").await;

    let (status, body) = app.post(&format!("/api/cards/{a}/move"), json!({ "before": a })).await;
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
    let d = deck(&app, "Test 1").await;
    let a = flash(&app, d, "first").await;
    let b = flash(&app, d, "second").await;

    let (_, before_move) = app.get(&format!("/api/cards/{a}")).await;

    // Position is list metadata, not a content edit: Part 3's scheduling must
    // not see a reorder as a revision.
    let (_, moved) = app.post(&format!("/api/cards/{a}/move"), json!({ "before": null })).await;
    assert_eq!(moved["updated_at"], before_move["updated_at"]);
    assert_eq!(order(&app, d).await, vec![b, a]);
}

#[tokio::test]
async fn a_move_keeps_interleaved_archived_cards_in_place() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let a = flash(&app, d, "visible one").await;
    let hidden = flash(&app, d, "archived").await;
    let c = flash(&app, d, "visible two").await;
    app.post(&format!("/api/cards/{hidden}/archive"), json!({})).await;

    // The UI would send this while `hidden` is filtered out of the list: move
    // c before a. `hidden` must keep its relative slot rather than be pushed
    // to an end.
    app.post(&format!("/api/cards/{c}/move"), json!({ "before": a })).await;
    assert_eq!(order(&app, d).await, vec![c, a, hidden]);
    assert_eq!(positions(&app, d).await, vec![0, 1, 2]);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test --test cards
```

Expected: the new tests fail with 405 Method Not Allowed / 404 — the route does not exist.

Note on status codes: `AppError::Validation` maps to **422**, not 400 (`backend/src/error.rs`), and
the codebase is uniform on this — 35 `UNPROCESSABLE_ENTITY` assertions against 2 `BAD_REQUEST`.
The `deny_unknown_fields` rejection in Task 1 is 422 for the same reason.

- [ ] **Step 3: Add the request type and handler**

In `backend/src/routes/cards.rs`, add the input type next to the other `Deserialize` structs:

```rust
/// Where to put a card, relative to one of its deck-mates.
///
/// Relative rather than a whole-deck permutation on purpose: the deck screen
/// can be filtered (archived hidden), so the client does not know where the
/// hidden cards sit and cannot honestly send a complete order. "Before card X"
/// stays well-defined whatever is filtered out, and is idempotent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveCard {
    /// The card to land immediately before, or null for the end of the deck.
    pub before: Option<i64>,
}
```

Add the handler after `set_archived`:

```rust
/// Reorders one card within its deck.
///
/// The whole deck's positions are rewritten in a single transaction rather
/// than nudging neighbours or interpolating gaps: O(n) writes per move is
/// nothing for a deck of a few hundred cards, and it keeps the dense 0-based
/// invariant true by construction instead of by argument.
async fn move_card(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    AppJson(body): AppJson<MoveCard>,
) -> AppResult<Json<CardDto>> {
    let card = fetch_summary(&st.pool, id).await?; // 404 before any write

    if body.before == Some(id) {
        return Err(AppError::validation([(
            "before",
            "A card cannot move before itself",
        )]));
    }

    let mut tx = st.pool.begin().await?;

    let mut ids: Vec<i64> = sqlx::query_scalar!(
        r#"SELECT id AS "id!: i64" FROM cards
           WHERE deck_id = ? ORDER BY position ASC, id ASC"#,
        card.deck_id
    )
    .fetch_all(&mut *tx)
    .await?;

    // One check covers both "no such card" and "not in this deck": from the
    // client's side they are the same mistake, and distinguishing them would
    // leak whether an id exists in some other deck.
    if let Some(before) = body.before {
        if !ids.contains(&before) {
            return Err(AppError::validation([(
                "before",
                "That card is not in this deck",
            )]));
        }
    }

    ids.retain(|&x| x != id);
    match body.before {
        Some(before) => {
            let at = ids.iter().position(|&x| x == before).expect("checked above");
            ids.insert(at, id);
        }
        None => ids.push(id),
    }

    for (i, card_id) in ids.iter().enumerate() {
        let position = i as i64;
        // updated_at is deliberately untouched: see the test
        // `a_move_does_not_bump_updated_at`.
        sqlx::query!("UPDATE cards SET position = ? WHERE id = ?", position, card_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    Ok(Json(fetch_full(&st.pool, id).await?))
}
```

Only one early return sits inside the transaction — the `before`-not-in-deck check. The self-move
check runs before `pool.begin()`, which is preferable: rejecting it needs no transaction at all.
For the in-transaction one, `tx` is dropped without `commit`, which rolls back; nothing has been
written by then, so the rollback is belt-and-braces rather than load-bearing — but it is what the
"a rejected move writes nothing" assertion in `moving_before_a_card_in_another_deck_is_rejected`
pins.

- [ ] **Step 4: Register the route**

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cards", get(list).post(create))
        .route("/cards/{id}", get(get_one).patch(patch))
        .route("/cards/{id}/archive", axum::routing::post(archive))
        .route("/cards/{id}/unarchive", axum::routing::post(unarchive))
        .route("/cards/{id}/move", axum::routing::post(move_card))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo sqlx prepare --workspace
cargo test --test cards
```

Expected: all pass. If `query_scalar!` complains about the offline cache, `cargo sqlx prepare` was
not re-run after adding the new query.

- [ ] **Step 6: Run the full gate and commit**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
git add backend/src/routes/cards.rs backend/tests/cards.rs .sqlx
git commit -m "feat(cards): POST /api/cards/:id/move for relative reordering

Body is `before: id|null` — land immediately before that card, or at
the end of the deck. Relative rather than a full permutation because the
deck screen can be filtered, so the client cannot honestly send a
complete order. Rewrites the deck's positions in one transaction and
leaves updated_at alone.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 3: API client — `position` and `moveCard`

**Goal:** The frontend types know about `position`, and there is one typed call for the move endpoint.

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Acceptance Criteria:**
- [ ] `CardSummary` has `position: number`
- [ ] `api.moveCard(id, before)` posts `{ before }` to `/cards/:id/move` and returns the updated `Card`
- [ ] `CardInput` still has no `position` field
- [ ] `pnpm exec tsc --noEmit` is clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit` → no output

**Steps:**

- [ ] **Step 1: Add `position` to `CardSummary`**

In `frontend/src/lib/api.ts`:

```ts
export type CardSummary = {
  id: number
  deck_id: number
  kind: CardKind
  prompt_md: string
  image_path: string | null
  answer_md: string | null
  explanation_md: string | null
  archived: boolean
  /**
   * Order within the deck: 0-based, dense, archived cards included.
   * Server-assigned — absent from `CardInput` on purpose, like
   * `choices.position` and `accepted.normalised`.
   */
  position: number
  created_at: string
  updated_at: string
}
```

`CardInput` is left exactly as it is. The server rejects an unknown `position` key outright.

- [ ] **Step 2: Add `moveCard` to the `api` object**

Next to `archiveCard`/`unarchiveCard`:

```ts
  /**
   * Move a card to immediately before `before`, or to the end of its deck
   * when `before` is null.
   *
   * `before` is the card that FOLLOWS the moved one in the intended order —
   * not the row it was dropped on. Dragging downwards those differ by one;
   * see DeckPage's drag handler.
   */
  moveCard: (id: number, before: number | null) =>
    request<Card>('POST', `/cards/${id}/move`, { before }),
```

- [ ] **Step 3: Typecheck and commit**

```bash
cd frontend && pnpm exec tsc --noEmit
cd .. && git add frontend/src/lib/api.ts
git commit -m "feat(ui): position on CardSummary and api.moveCard

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 4: `useFlip` — the half-flip state machine

**Goal:** One hook that flips a variable-height element by rotating to edge-on, swapping the face while invisible, and rotating back — honouring `prefers-reduced-motion`.

**Files:**
- Create: `frontend/src/components/deck/useFlip.ts`

**Acceptance Criteria:**
- [ ] `useFlip` returns the current `face`, a `flip()` toggle, a `toFront()` escape hatch, and style props for the rotating element
- [ ] It takes no arguments — callers react to `face` changing, not to a callback
- [ ] Only one face is ever rendered, so the element keeps its natural height
- [ ] Rapid repeated clicks cannot interleave two flips
- [ ] `toFront()` is never dropped: called mid-flip it queues and runs when the machine settles
- [ ] Under `prefers-reduced-motion: reduce` the face swaps with no rotation
- [ ] Every timer and animation frame is cancelled on unmount — no state updates after teardown
- [ ] `pnpm exec tsc --noEmit` is clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build` → both clean

**Steps:**

- [ ] **Step 1: Write the hook**

Create `frontend/src/components/deck/useFlip.ts`:

```ts
import { useCallback, useEffect, useRef, useState } from 'react'

/** Which face is currently mounted. */
export type Face = 'front' | 'back'

/** One leg of the flip, in ms. The two legs make up the whole animation. */
const HALF_MS = 150

/**
 * A half-flip: rotate to edge-on, swap the face while it cannot be seen,
 * rotate back.
 *
 * Not a two-faced 3D flip. That needs both faces absolutely positioned inside
 * a fixed-height box, and a card's prompt is unclamped markdown of any height
 * (a recorded decision — see the Part 2b spec). Here only one face is ever
 * mounted, so the row keeps its natural height and the height change happens
 * during the 90° moment when the card is edge-on and invisible.
 *
 * Deliberately has no "flip started" callback. A caller that needs to react to
 * the new face watches the returned `face` in an effect — which keeps the
 * caller free of the ordering trap of passing in a callback that has to
 * reference values declared after this hook runs.
 */
export function useFlip() {
  const [face, setFace] = useState<Face>('front')
  const [angle, setAngle] = useState(0)
  // Transitions are suppressed for the single frame where the element jumps
  // from +90° to -90°: animating that would sweep it back through 0 and undo
  // the flip on screen.
  const [instant, setInstant] = useState(false)

  const busy = useRef(false)
  const cleanups = useRef<Array<() => void>>([])
  // A return-to-front requested while a flip was still in flight. The flip
  // that is running must finish — interrupting it mid-rotation would leave the
  // element at an arbitrary angle — so the request waits here and runs the
  // moment the machine settles.
  const pending = useRef<Face | null>(null)
  // `goTo` schedules a callback that may need to call `goTo` again, which it
  // cannot reference during its own definition. The ref is refreshed after
  // every render, so by the time a scheduled callback fires it holds the
  // current closure.
  const goToRef = useRef<(next: Face) => void>(() => {})

  const later = useCallback((fn: () => void, ms: number) => {
    const t = window.setTimeout(fn, ms)
    cleanups.current.push(() => window.clearTimeout(t))
  }, [])

  const nextFrame = useCallback((fn: () => void) => {
    const outer = window.requestAnimationFrame(() => {
      const inner = window.requestAnimationFrame(fn)
      cleanups.current.push(() => window.cancelAnimationFrame(inner))
    })
    cleanups.current.push(() => window.cancelAnimationFrame(outer))
  }, [])

  useEffect(
    () => () => {
      cleanups.current.forEach((c) => c())
      cleanups.current = []
    },
    [],
  )

  const goTo = useCallback(
    (next: Face) => {
      if (busy.current || next === face) return

      if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
        setFace(next)
        return
      }

      busy.current = true
      setInstant(false)
      setAngle(90)

      later(() => {
        // Edge-on: swap the face and jump to the far side with no transition.
        setInstant(true)
        setFace(next)
        setAngle(-90)

        // Two frames: one to paint the -90° state, one to re-enable the
        // transition before animating home. One frame is not reliably enough.
        nextFrame(() => {
          setInstant(false)
          setAngle(0)
          later(() => {
            busy.current = false
            const queued = pending.current
            pending.current = null
            if (queued !== null) goToRef.current(queued)
          }, HALF_MS)
        })
      }, HALF_MS)
    },
    [face, later, nextFrame],
  )

  useEffect(() => {
    goToRef.current = goTo
  }, [goTo])

  const flip = useCallback(() => {
    goTo(face === 'front' ? 'back' : 'front')
  }, [face, goTo])

  /**
   * Return to the question — used when the answer fetch fails.
   *
   * Unlike `flip`, this is never dropped. The fetch it backs out is started
   * the instant `face` becomes 'back', which is the midpoint of the flip, so
   * a fast failure lands while the machine is still busy; ignoring it would
   * leave the card resting on a face whose content never loaded.
   */
  const toFront = useCallback(() => {
    if (busy.current) {
      pending.current = 'front'
      return
    }
    goTo('front')
  }, [goTo])

  return {
    face,
    flip,
    toFront,
    /** Goes on the element that rotates. */
    rotatorStyle: {
      transform: `rotateY(${angle}deg)`,
      transition: instant ? 'none' : `transform ${HALF_MS}ms ease-in-out`,
    } satisfies React.CSSProperties,
    /** Goes on the element wrapping the rotator. */
    perspectiveStyle: { perspective: '1200px' } satisfies React.CSSProperties,
  }
}
```

- [ ] **Step 2: Typecheck**

```bash
cd frontend && pnpm exec tsc --noEmit
```

Expected: clean. If `React.CSSProperties` is unresolved, add `import type React from 'react'` at
the top.

- [ ] **Step 4: Commit**

```bash
cd .. && git add frontend/src/components/deck/useFlip.ts
git commit -m "feat(ui): useFlip, a half-flip that survives variable heights

Rotates to edge-on, swaps the single mounted face while it is invisible,
rotates back. A two-faced 3D flip would need a fixed row height, which
the unclamped markdown prompts rule out.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 5: `CardBack` — the answer face

**Goal:** One component that renders a card's answer per kind, including the two-column multiple-choice grid and its reveal styling.

**Files:**
- Create: `frontend/src/components/deck/CardBack.tsx`

**Acceptance Criteria:**
- [ ] Flashcard renders `answer_md` through `<Markdown>`
- [ ] Short answer renders the primary accepted answer emphasised, alternates as muted chips
- [ ] Multiple choice renders a `sm:grid-cols-2` grid, choices lettered A, B, C… by array order (which is `position` order from the API)
- [ ] Unrevealed choices are uniform — no visual tell of which is correct
- [ ] Revealed: correct choice uses `bg-success`, the rest `bg-muted`
- [ ] The correct choice carries a visually-hidden "Correct answer" when revealed, so the state is never colour-only
- [ ] `explanation_md` renders in muted text below the answer for every kind, when present
- [ ] `pnpm exec tsc --noEmit` is clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build` → both clean

**Steps:**

- [ ] **Step 1: Write the component**

Create `frontend/src/components/deck/CardBack.tsx`:

```tsx
import type { Card } from '@/lib/api'
import { Markdown } from '@/components/Markdown'
import { cn } from '@/lib/utils'

type Props = {
  card: Card
  /** Multiple choice only: whether the correct option is shown as correct. */
  revealed: boolean
}

const LETTERS = 'ABCDEFGHIJ'

/**
 * Unrevealed options are deliberately uniform: the back of a multiple-choice
 * card is a self-test, so nothing may hint at the answer until the eye button
 * is pressed. `--success` is the theme's designated correct-answer colour.
 */
function choiceClass(isCorrect: boolean, revealed: boolean): string {
  if (!revealed) return 'bg-accent/85 text-accent-foreground'
  return isCorrect
    ? 'bg-success text-success-foreground font-medium'
    : 'bg-muted text-muted-foreground'
}

/**
 * A card's answer. One component per kind would triple the file count for
 * three small branches that share their explanation footer, so they live
 * together here.
 */
export function CardBack({ card, revealed }: Props) {
  return (
    <div className="space-y-3">
      {card.kind === 'mc_single' && (
        <ul className="grid gap-2 sm:grid-cols-2">
          {card.choices.map((c, i) => (
            <li
              key={c.id}
              className={cn(
                'flex items-start gap-1.5 rounded-lg px-3 py-2 text-sm transition-colors',
                choiceClass(c.is_correct, revealed),
              )}
            >
              <span className="font-semibold">{LETTERS[i] ?? '•'}.</span>
              <Markdown className="min-w-0 flex-1">{c.text_md}</Markdown>
              {revealed && c.is_correct && <span className="sr-only">Correct answer</span>}
            </li>
          ))}
        </ul>
      )}

      {card.kind === 'short_answer' && <ShortAnswer card={card} />}

      {card.kind === 'flashcard' && <Markdown>{card.answer_md ?? ''}</Markdown>}

      {/* One field on `cards` with no per-kind rule behind it, so it shows for
          every kind rather than only flashcards. */}
      {card.explanation_md && (
        <Markdown className="border-t pt-2 text-sm text-muted-foreground">
          {card.explanation_md}
        </Markdown>
      )}
    </div>
  )
}

/** The API orders `accepted` by `is_primary DESC, id`, so the primary is first. */
function ShortAnswer({ card }: { card: Card }) {
  const [primary, ...alternates] = card.accepted
  return (
    <div className="space-y-2">
      <p className="font-medium">{primary?.text ?? '—'}</p>
      {alternates.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-xs text-muted-foreground">also accepted:</span>
          {alternates.map((a) => (
            <span
              key={a.id}
              className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground"
            >
              {a.text}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}
```

`accepted[].text` is plain text, not markdown — the editor stores exactly what was typed as the
grading key — so it renders as text, not through `<Markdown>`.

- [ ] **Step 2: Typecheck and commit**

```bash
cd frontend && pnpm exec tsc --noEmit
cd .. && git add frontend/src/components/deck/CardBack.tsx
git commit -m "feat(ui): CardBack, the per-kind answer face

Multiple choice gets a two-column grid whose options stay uniform until
revealed, then colour the correct one with the theme's success token and
announce it to screen readers.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 6: `CardRow` — the sortable, flippable row

**Goal:** One row: drag grip, header strip with kind badge and pill controls, and a flip target that fetches its answer on first flip.

**Files:**
- Modify: `frontend/package.json` (add `@dnd-kit/core`, `@dnd-kit/sortable`, `@dnd-kit/utilities`)
- Create: `frontend/src/components/deck/CardRow.tsx`

**Acceptance Criteria:**
- [ ] The row is a `useSortable` item; only the grip starts a drag
- [ ] Clicking the card body flips it; Enter and Space do the same from the keyboard
- [ ] The body's `aria-label` alternates between "Show answer" and "Show question"
- [ ] The image thumbnail opens its lightbox and does **not** flip the card, by mouse **and** by
      keyboard — `onKeyDown` must ignore keys originating on descendants, or Enter's default
      action (the button's click) is cancelled and the lightbox becomes unreachable
- [ ] Kind badge, edit pill and archive pill sit in the header strip, outside the flip target
- [ ] The eye button appears for `mc_single` only, carries `aria-pressed`, and its label alternates "Reveal answer" / "Hide answer"
- [ ] Reveal resets when the card flips back to the question
- [ ] The answer is fetched once per row on first flip (an effect watching `face`); the back shows a skeleton until it lands
- [ ] A failed fetch toasts and returns the row to the question; an aborted fetch is silent
- [ ] `pnpm exec tsc --noEmit` and `pnpm build` are clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build` → both clean

**Steps:**

- [ ] **Step 1: Install the drag-and-drop packages**

```bash
cd frontend
pnpm add @dnd-kit/core @dnd-kit/sortable @dnd-kit/utilities
```

`@dnd-kit/utilities` is a transitive dependency of `sortable`, but `CardRow` imports `CSS` from it
directly, so it is declared explicitly rather than relied on by accident. Check the install output
for React peer-dependency warnings — this project is on React 19; if `pnpm` reports an unmet peer,
stop and report it rather than forcing the install.

- [ ] **Step 2: Write the component**

Create `frontend/src/components/deck/CardRow.tsx`:

```tsx
import { useEffect, useRef, useState } from 'react'
import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import {
  Archive as ArchiveIcon,
  ArchiveRestore,
  Eye,
  EyeOff,
  GripVertical,
  Pencil,
} from 'lucide-react'
import { toast } from 'sonner'
import { KIND_LABEL, type Card, type CardSummary } from '@/lib/api'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { CardImage } from '@/components/CardImage'
import { Markdown } from '@/components/Markdown'
import { CardBack } from '@/components/deck/CardBack'
import { useFlip } from '@/components/deck/useFlip'
import { cn } from '@/lib/utils'

type Props = {
  card: CardSummary
  /** Fetches the full card. Supplied by the page so the row owns no api import policy. */
  loadCard: (id: number, signal: AbortSignal) => Promise<Card>
  onEdit: () => void
  onArchiveToggle: () => void
}

/**
 * One card in the deck list.
 *
 * The answer is fetched here, per row, and kept for the row's lifetime. A
 * page-level cache would need an eviction rule; this needs none — the only
 * thing that invalidates an answer is an edit, and editing navigates to
 * `/cards/:id/edit`, which remounts the whole page on return. Archive and
 * unarchive change no answer content.
 */
export function CardRow({ card, loadCard, onEdit, onArchiveToggle }: Props) {
  const [full, setFull] = useState<Card | null>(null)
  const [loading, setLoading] = useState(false)
  const [revealed, setRevealed] = useState(false)
  const inFlight = useRef<AbortController | null>(null)

  const { face, flip, toFront, rotatorStyle, perspectiveStyle } = useFlip()

  // Driven by `face` rather than by a callback passed into useFlip: the fetch
  // needs `toFront` for its failure path, and useFlip returns that, so a
  // callback would have to close over a value declared after it.
  useEffect(() => {
    if (face !== 'back') {
      // Flipping away abandons a fetch still in the air — by the time it
      // landed the user would be looking at the question again.
      inFlight.current?.abort()
      inFlight.current = null
      setLoading(false)
      return
    }
    if (full || inFlight.current) return

    const controller = new AbortController()
    inFlight.current = controller
    setLoading(true)
    loadCard(card.id, controller.signal)
      .then(setFull)
      .catch((e: unknown) => {
        if ((e as Error)?.name === 'AbortError') return
        toast.error('Could not load the answer')
        toFront()
      })
      .finally(() => {
        if (inFlight.current === controller) {
          inFlight.current = null
          setLoading(false)
        }
      })
  }, [card.id, face, full, loadCard, toFront])

  // Unmount mid-flight: abort rather than resolve into a dead component.
  useEffect(() => () => inFlight.current?.abort(), [])

  // Revealing is per visit to the answer, not per card: flipping back to the
  // question and forward again should be a fresh self-test.
  useEffect(() => {
    if (face === 'front') setRevealed(false)
  }, [face])

  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: card.id })

  const showingAnswer = face === 'back'

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={cn('flex items-start gap-1', isDragging && 'relative z-10 opacity-80')}
    >
      <button
        type="button"
        ref={setActivatorNodeRef}
        {...attributes}
        {...listeners}
        aria-label={`Reorder ${card.prompt_md.slice(0, 40)}`}
        title="Drag to reorder"
        className={cn(
          'mt-4 shrink-0 cursor-grab touch-none rounded-md p-1 text-muted-foreground',
          'hover:bg-muted hover:text-foreground active:cursor-grabbing',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        )}
      >
        <GripVertical className="size-4" />
      </button>

      <div
        style={perspectiveStyle}
        className={cn('min-w-0 flex-1', card.archived && 'opacity-60')}
      >
        <div
          style={rotatorStyle}
          className="space-y-3 rounded-xl border bg-card p-3 shadow-sm"
        >
          {/* Header strip. Outside the flip target below, which keeps these
              controls out of the flip and avoids nesting buttons in a button. */}
          <div className="flex items-center gap-2">
            <Badge variant="outline">{KIND_LABEL[card.kind]}</Badge>
            {card.archived && <Badge variant="secondary">Archived</Badge>}

            <Button
              variant="secondary"
              size="icon-sm"
              className="rounded-full"
              aria-label={`Edit card ${card.id}`}
              title="Edit card"
              onClick={onEdit}
            >
              <Pencil />
            </Button>
            <Button
              variant="secondary"
              size="icon-sm"
              className="rounded-full"
              aria-label={`${card.archived ? 'Unarchive' : 'Archive'} card ${card.id}`}
              title={card.archived ? 'Unarchive card' : 'Archive card'}
              onClick={onArchiveToggle}
            >
              {card.archived ? <ArchiveRestore /> : <ArchiveIcon />}
            </Button>

            {card.kind === 'mc_single' && showingAnswer && full && (
              <Button
                variant="secondary"
                size="icon-sm"
                className="ml-auto rounded-full"
                aria-pressed={revealed}
                aria-label={revealed ? 'Hide answer' : 'Reveal answer'}
                title={revealed ? 'Hide answer' : 'Reveal answer'}
                onClick={() => setRevealed((r) => !r)}
              >
                {revealed ? <EyeOff /> : <Eye />}
              </Button>
            )}
          </div>

          {/* The flip target. A div rather than a button because the image
              thumbnail inside it is itself a button. */}
          <div
            role="button"
            tabIndex={0}
            aria-label={showingAnswer ? 'Show question' : 'Show answer'}
            onClick={flip}
            onKeyDown={(e) => {
              // Only keys pressed on the card body itself. A keydown from a
              // focusable descendant — the image thumbnail's button, or a link
              // in the markdown — must reach its own default action: Enter's
              // default action IS the button's click, so preventing it here
              // would make the lightbox unreachable by keyboard.
              if (e.target !== e.currentTarget) return
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                flip()
              }
            }}
            className="cursor-pointer rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {showingAnswer ? (
              loading || !full ? (
                <div className="space-y-2" aria-busy="true">
                  <div className="h-4 w-2/3 animate-pulse rounded bg-muted" />
                  <div className="h-4 w-1/3 animate-pulse rounded bg-muted" />
                </div>
              ) : (
                <CardBack card={full} revealed={revealed} />
              )
            ) : (
              <div className="flex items-start justify-between gap-3">
                {/* Unclamped on purpose: a truncated single line cannot render
                    markdown without a half-open `$…$` or a stray list marker
                    looking broken. Recorded trade-off from Part 2b. */}
                <Markdown className="min-w-0 flex-1">{card.prompt_md}</Markdown>
                {card.image_path !== null && (
                  // Keeps its lightbox, so it is a deliberate non-flipping
                  // region: the click must not reach the flip handler.
                  <div onClick={(e) => e.stopPropagation()}>
                    <CardImage path={card.image_path} alt={card.prompt_md} />
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </li>
  )
}
```

Two details that will bite if changed: `touch-none` on the grip is what stops a touch drag from
scrolling the page instead of dragging, and the `<div onClick={stopPropagation}>` wrapper around
`<CardImage>` is what keeps the lightbox from also flipping the card.

- [ ] **Step 3: Typecheck**

```bash
cd frontend && pnpm exec tsc --noEmit
```

Expected: clean. If `setActivatorNodeRef` is reported as missing from `useSortable`'s return type,
the installed `@dnd-kit/sortable` is older than v7 — check the version before working around it.

- [ ] **Step 5: Commit**

```bash
cd .. && git add frontend/package.json frontend/pnpm-lock.yaml \
                frontend/src/components/deck/CardRow.tsx
git commit -m "feat(ui): CardRow, a sortable card that flips to its answer

Grip-only drag via @dnd-kit/sortable, a header strip of pill controls
outside the flip target, and a per-row answer fetch on first flip. The
image thumbnail keeps its lightbox and stops the click short of the flip.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 7: `DeckPage` — header, drag context, optimistic reorder

**Goal:** The deck screen matches the mockup's layout and commits reorders to the server, reverting cleanly on failure.

**Files:**
- Modify: `frontend/src/pages/DeckPage.tsx`

**Acceptance Criteria:**
- [ ] Header has a back arrow to `/decks`, the deck name, module badge, description and card count, and an accent "Add Card" button
- [ ] The card list is a `DndContext` + `SortableContext` of `CardRow`s
- [ ] Dragging a card reorders it locally at once and posts the move
- [ ] `before` is read from the card that follows the moved one in the new order — **not** `over.id`
- [ ] Dropping a card last sends `before: null`
- [ ] A failed move toasts, reverts the local order and refetches
- [ ] A list response that a drag superseded is discarded, not applied over the reorder
- [ ] A list fetch dropped by a drag is re-issued once the move commits, so the show-archived
      switch and the list cannot end up disagreeing
- [ ] Keyboard reorder works: focus a grip, Space to lift, arrows to move, Space to drop
- [ ] The show-archived switch, archive/unarchive and the empty state all still work
- [ ] `pnpm exec tsc --noEmit` and `pnpm build` are clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build` → both clean

**Steps:**

- [ ] **Step 1: Replace the imports at the top of `frontend/src/pages/DeckPage.tsx`**

```tsx
import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { ArrowLeft, Plus } from 'lucide-react'
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core'
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { toast } from 'sonner'
import { api, type CardSummary, type Deck } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { CardRow } from '@/components/deck/CardRow'
```

`KIND_LABEL`, `Markdown` and `CardImage` are no longer used here — they moved into `CardRow`.
Leaving the imports behind will fail the build on `noUnusedLocals`.

- [ ] **Step 2: Guard `loadCards` against a superseded response**

The page now has two writers to `cards`: this fetch and the drag's optimistic reorder. `loadCards`
guards only `setLoading` with its stale-response check, so an in-flight response can land on top of
a newer local reorder. Two edits.

Immediately before `setCards(rows)`:

```ts
      // A newer request — or a drag's optimistic reorder, which clears this
      // ref — has superseded this response. Applying it would overwrite newer
      // state with older server data.
      if (inFlight.current !== controller) return
      setCards(rows)
```

And in the `finally`, clear the ref when this call owns it, so `inFlight.current !== null` genuinely
means "a request is in flight" — without this it keeps pointing at a *completed* controller and the
drag handler cannot tell in-flight from finished:

```ts
    } finally {
      if (inFlight.current === controller) {
        inFlight.current = null
        setLoading(false)
      }
    }
```

Safe on every other consumer: `loadCards`' own leading `inFlight.current?.abort()` and the unmount
cleanup both use optional chaining and no-op on null.

- [ ] **Step 3: Add the sensors and the drag handler**

Inside the component, after the existing `archive`/`unarchive` functions:

```tsx
  // A small distance threshold so a click on the grip is still a click, and
  // the keyboard sensor so reordering does not require a pointer at all.
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event
    if (!over || active.id === over.id) return

    const from = cards.findIndex((c) => c.id === active.id)
    const to = cards.findIndex((c) => c.id === over.id)
    if (from === -1 || to === -1) return

    // Any list response still in flight predates this reorder and would land
    // on top of it, so it must not be applied. But it may have been fetching a
    // *different* filter set — a show-archived toggle the user hit moments
    // earlier — so dropping it outright would leave the switch and the list
    // disagreeing. Note that we dropped one and re-issue it once the move
    // settles.
    const droppedFetch = inFlight.current !== null
    inFlight.current?.abort()
    inFlight.current = null
    setLoading(false)

    const previous = cards
    const next = arrayMove(cards, from, to)
    setCards(next)

    // `before` is the card that FOLLOWS the moved one in the new order, not
    // `over.id`. Dragging downwards, the over-row is the one being displaced
    // and sits *above* the landing slot, so sending it would be off by one.
    // Reading the new neighbour is correct in both directions with no special
    // case. When archived cards are hidden this is the next *visible* card,
    // which is exactly the semantics the endpoint implements: hidden cards
    // keep their slots above it.
    const landed = next.findIndex((c) => c.id === active.id)
    const before = landed + 1 < next.length ? next[landed + 1].id : null

    void api.moveCard(Number(active.id), before)
      .then(() => {
        // Only when we actually dropped one: a refetch on every drag would be
        // a wasted round trip, since the optimistic order already matches what
        // the server just committed. Running it after the move resolves means
        // it reconciles the reorder AND the filter set in one request.
        if (droppedFetch) void loadCards()
      })
      .catch(() => {
        toast.error('Could not reorder cards')
        setCards(previous)
        void loadCards()
      })
  }
```

- [ ] **Step 4: Replace the page header**

Replace the existing header block (the `<div className="flex flex-wrap items-start justify-between gap-3">` and its contents) with:

```tsx
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 items-start gap-3">
          <Button
            variant="ghost"
            size="icon"
            className="mt-0.5 shrink-0"
            aria-label="Back to decks"
            title="Back to decks"
            onClick={() => navigate('/decks')}
          >
            <ArrowLeft />
          </Button>
          <div className="min-w-0 space-y-1">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="font-display text-2xl font-bold">{deck.name}</h1>
              {deck.module_id !== null && <Badge variant="secondary">{deck.module_name}</Badge>}
            </div>
            {deck.description && <p className="text-muted-foreground">{deck.description}</p>}
            <p className="text-sm text-muted-foreground">
              {deck.card_count} card{deck.card_count === 1 ? '' : 's'}
            </p>
          </div>
        </div>
        <Button
          className="h-10 bg-accent px-4 text-accent-foreground hover:bg-accent/80"
          onClick={() => navigate(`/cards/new?deck_id=${deck.id}`)}
        >
          <Plus className="size-4" />
          Add card
        </Button>
      </div>
```

- [ ] **Step 5: Replace the card list**

Replace the whole `<ul className="space-y-2">…</ul>` block with:

```tsx
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={cards.map((c) => c.id)}
          strategy={verticalListSortingStrategy}
        >
          <ul className="space-y-3">
            {cards.map((c) => (
              <CardRow
                key={c.id}
                card={c}
                loadCard={api.getCard}
                onEdit={() => navigate(`/cards/${c.id}/edit`)}
                onArchiveToggle={() => void (c.archived ? unarchive(c) : archive(c))}
              />
            ))}
          </ul>
        </SortableContext>
      </DndContext>
```

`loadCard={api.getCard}` matches `CardRow`'s `(id, signal) => Promise<Card>` — `api.getCard`'s
`signal` is optional, which is assignable to a required parameter.

- [ ] **Step 6: Typecheck, build, and check the whole screen by hand**

```bash
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

Then, with `cargo run` in one terminal and `pnpm dev` in another, open
`http://localhost:5273/decks/<id>` and confirm each of these:

1. Clicking a card body flips it with a rotation; the answer appears.
2. Tab to a card body, press Enter — same flip. Press Space — same flip, page does not scroll.
3. A multiple-choice back shows uniform options; the eye button colours the correct one and the
   label flips to "Hide answer".
4. Flip back and forward again — the reveal has reset.
5. Clicking a card's image opens the lightbox and does **not** flip the card. Then **Tab to the
   thumbnail and press Enter** — the lightbox must open, and the card must NOT flip. Repeat with
   Space. This keyboard half is the point: the mouse half passed while the keyboard path was
   broken (Enter bubbled to the flip target, which cancelled the button's click), and that bug
   was caught by review rather than by this list.
6. Drag a card by its grip to a new position; reload the page — the new order persists.
7. Tab to a grip, press Space, press ArrowDown twice, press Space — the card moves, and reload
   confirms it persisted.
8. Toggle "Show archived" on, drag a visible card past an archived one, reload — the archived card
   is still where it was.
9. In macOS System Settings → Accessibility → Display, turn on "Reduce motion", then flip a card —
   the face swaps with no rotation.

- [ ] **Step 7: Commit**

```bash
cd .. && git add frontend/src/pages/DeckPage.tsx
git commit -m "feat(ui): deck screen as a sortable deck of flippable cards

Back arrow and Add card in the header, dnd-kit drag context around the
list, and an optimistic reorder that reverts on failure. `before` comes
from the moved card's new follower, not the dropped-on row, which is what
makes downward drags land in the right slot.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Task 8: Full gate and handover

**Goal:** The whole verification gate passes on the finished branch, and the handover records what shipped.

**Files:**
- Modify: `docs/HANDOVER.md`

**Acceptance Criteria:**
- [ ] `cargo test` passes with the new tests included
- [ ] `cargo clippy --all-targets -- -D warnings` is clean
- [ ] `SQLX_OFFLINE=true cargo build` succeeds
- [ ] `pnpm exec tsc --noEmit` and `pnpm build` are clean
- [ ] `docs/HANDOVER.md` describes the new deck screen, the `position` column and the move endpoint, and points at the spec

**Verify:** the four gate commands below, all clean

**Steps:**

- [ ] **Step 1: Run the whole gate**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

`--all-targets` matters: plain `cargo clippy -- -D warnings` does not build test targets, and this
plan adds a lot of test code. Do not quietly drop the flag.

- [ ] **Step 2: Update the handover**

In `docs/HANDOVER.md`, under "Where things stand", replace the `/decks/:id` bullet:

```markdown
- `/decks/:id` screen: the deck as a list of flippable cards. Click or Enter on a card body flips
  it (a half-flip: rotate to edge-on, swap the single mounted face, rotate back — a two-faced 3D
  flip would need a fixed row height, which the unclamped markdown prompts rule out) and the
  answer is fetched per row on first flip via `GET /api/cards/:id`. Multiple-choice backs show a
  two-column grid whose options stay uniform until the eye button reveals the correct one. Rows
  drag to reorder by their grip (`@dnd-kit/sortable`, keyboard reorder included), and the order
  persists. Archive/unarchive and the show-archived toggle are unchanged. Design:
  [`mitis/specs/2026-08-27-deck-card-list-redesign-design.md`](mitis/specs/2026-08-27-deck-card-list-redesign-design.md)
```

And add to the API bullets:

```markdown
- `cards.position`: a dense 0-based order per deck (migration `0002`), backfilled from the
  `created_at` ordering the list used before, with archived cards keeping their slots.
  `GET /api/cards` orders by it. `POST /api/cards/:id/move` takes `{"before": id|null}` — land
  immediately before that card, or at the end of the deck — and rewrites the deck's positions in
  one transaction without touching `updated_at`. It is relative rather than a whole-deck
  permutation because the deck screen can be filtered, so the client cannot honestly send a
  complete order.
```

Under "Next up", add a line to the two-things-already-in-place paragraph:

```markdown
A third: `cards.position` is the deck's authored order, and practice mode should read it rather
than inventing an order of its own.
```

- [ ] **Step 3: Commit**

```bash
cd .. && git add docs/HANDOVER.md
git commit -m "docs: bring the handover up to date for the deck card list redesign

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Verification Summary

| Command | Run from | Expects |
|---|---|---|
| `cargo test` | repo root | all pass, including 14 new card tests |
| `cargo clippy --all-targets -- -D warnings` | repo root | no output |
| `SQLX_OFFLINE=true cargo build` | repo root | success |
| `pnpm exec tsc --noEmit` | `frontend/` | no output |
| `pnpm build` | `frontend/` | success |

`export PATH="$HOME/.cargo/bin:$PATH"` before any `cargo sqlx` command, and run every cargo
command from the repo root — from `backend/` the cwd-relative `DATABASE_URL` default silently
creates `backend/data/`.

There is no frontend test framework, by an existing spec decision. Task 7 Step 6 is the manual
walkthrough that stands in for it; do not skip it.
