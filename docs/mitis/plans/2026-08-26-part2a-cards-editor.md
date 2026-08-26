# Part 2a — Cards API and the Card Editor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `mitis:subagent-driven-development`
> (recommended) or `mitis:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking. Tasks:
> `docs/mitis/plans/2026-08-26-part2a-cards-editor.md.tasks.json`
>
> Read `docs/HANDOVER.md` before starting. Its "Environment quirks" and "Conventions and
> traps" sections each cost a fix round to learn and are assumed throughout this plan.

## Context

Part 1 (schema, modules, decks) and the decks search/filter/sort follow-on are complete and
on `main`. The next step is the spec's build-sequencing **step 2, the card editor** — the
app's most-used screen, because every card is written by hand. The COS781 test is on
**11 September 2026**.

The `cards`, `choices`, `accepted` and `schedule` tables already exist from
`backend/migrations/0001_init.sql` and are entirely unused. **No migration is written in this
plan.** Editing an applied migration changes its checksum and sqlx then refuses to run
against the existing database — a comment-only edit is enough.

Spec step 2 reads "card editor and card CRUD — all three kinds, image upload, KaTeX
rendering". That is too much for one plan, so it splits:

- **2a (this plan)** — cards API for all three kinds, `/decks/:id` card list, keyboard-first
  editor. Markdown is stored and displayed as raw source.
- **2b (next plan)** — image upload to `data/images/`, and one shared `<Markdown>` component
  (`react-markdown` + `remark-math` + `rehype-katex`) used by the card list, the editor
  preview and later the session runner.

The split is clean because 2a does *no* rendering rather than rendering-then-replacing:
2b adds one component and points the existing call sites at it. Nothing in 2a is reworked.

**Outcome of 2a:** open a deck, write `mc_single` / `short_answer` / `flashcard` cards
without touching the mouse, save-and-next straight into the following card, fix a card, and
archive one. This unblocks Part 3 (practice mode), which needs cards to exist.

**Deliberately out of scope:** image upload, KaTeX/markdown rendering, moving a card between
decks, bulk operations, card search, sessions/grading/override, stats, SM-2 logic.

**User decisions (already made):**
- Split into 2a (core) and 2b (image + KaTeX), as above.
- Editor lives at the spec's routes: `/decks/:id` list, `/cards/new?deck_id=`,
  `/cards/:id/edit`. Full page, not a dialog — the keyboard save-and-next loop and an
  mc_single card with four choices plus an explanation are both cramped in a modal.
- Raw markdown source everywhere in 2a; all rendering arrives together in 2b.
- The card list carries edit, archive, and a show-archived toggle with unarchive.

---

## Cross-cutting conventions

Everything in `docs/HANDOVER.md` § "Conventions and traps" applies. The ones this plan leans
on hardest:

**One parameterized `query_as!`, never N literal ones.** Let SQL branch on bound parameters
(`? = 'all' OR …`, `CASE WHEN ? = 'oldest' THEN …`). Plain repeated `?` placeholders only —
the macro counts by occurrence, so `?1`/`?2` break the binding.

**Timestamps are ISO-8601 with `Z`**, one-second resolution. Any date ordering therefore
needs an `id` tiebreak, because ties are the normal case, not an edge case.

**Ordering tests must be able to detect what they assert.** Mutation evidence only proves
what you mutate in isolation.

**Every failure returns the envelope** `{"error","message","fields"}`. The frontend renders
`fields` inline beside the offending input, and **a rejected save never clears typed
content.**

**Regenerate the sqlx cache after any SQL change**, from the repo root, with
`export PATH="$HOME/.cargo/bin:$PATH"`:

```bash
DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
git add .sqlx
```

### The card JSON contract (both sides of the seam)

Tasks 3–5 build the server half and Tasks 6–7 the client half of this contract. A per-task
review structurally cannot catch a mismatch across it, so it is written down once, here.

Full card, as returned by `GET /api/cards/:id`, `POST /api/cards`, `PATCH /api/cards/:id`:

```json
{
  "id": 12, "deck_id": 3, "kind": "mc_single",
  "prompt_md": "Which linkage merges the two closest points?",
  "image_path": null, "answer_md": null,
  "explanation_md": "Single linkage uses the minimum pairwise distance.",
  "archived": false,
  "created_at": "2026-08-26T14:02:11Z", "updated_at": "2026-08-26T14:02:11Z",
  "choices": [
    { "id": 40, "text_md": "Single", "is_correct": true,  "position": 0 },
    { "id": 41, "text_md": "Complete", "is_correct": false, "position": 1 }
  ],
  "accepted": []
}
```

`GET /api/cards` returns the same objects **without** `choices` and `accepted` — the list
never needs them and loading every child row for a 200-card deck would be waste.

Create body (`POST /api/cards`). `position` and `normalised` are **server-assigned** and must
not be sent; `position` is the array index:

```json
{ "deck_id": 3, "kind": "mc_single", "prompt_md": "…", "explanation_md": null,
  "choices": [ { "text_md": "Single", "is_correct": true },
               { "text_md": "Complete", "is_correct": false } ] }
```

`PATCH /api/cards/:id` takes the same body minus `deck_id`, and is a **full replace of the
card's editable content** — kind, prompt, answer, explanation and all children. It is not
a field-by-field patch, and it deliberately does not use the absent-vs-null dance that
`PATCH /api/decks/:id` needs: the editor always holds the whole card and always submits the
whole card, so an omitted optional means null. Cards do not move between decks in 2a.

Every DTO is `#[serde(deny_unknown_fields)]`, matching `decks.rs`.

---

## Task 1: `normalise` — the answer comparison key

**Goal:** A pure, DB-free normalisation function with unit tests, ready for `accepted.normalised`
on insert and for Part 3's grading to reuse.

**Files:**
- Create: `backend/src/normalise.rs`
- Modify: `backend/src/lib.rs` (`pub mod normalise;`), `backend/Cargo.toml`
- Modify: `docs/mitis/specs/2026-08-26-quiz-study-app-design.md` (record the step-order amendment)

**Acceptance Criteria:**
- [ ] NFKC folds compatibility forms (full-width `Ｋ` and the ﬁ ligature) before comparison
- [ ] Case is folded, so `"K-Means"` and `"k means"` share a key
- [ ] Punctuation becomes a space rather than vanishing, so `"k-means"` == `"k means"`
- [ ] Internal whitespace runs collapse to one space and the result is trimmed
- [ ] Normalising an already-normalised string is a no-op (idempotent)
- [ ] An input of only punctuation and spaces normalises to the empty string

**Verify:** `cargo test normalise::` → all pass

**Steps:**

- [ ] **Step 1: Add the dependency**

In `backend/Cargo.toml`, under `[dependencies]`:

```toml
unicode-normalization = "0.1"
```

- [ ] **Step 2: Note the deliberate divergence from the spec**

The spec lists normalisation as: NFKC → lowercase → trim/collapse whitespace → strip
punctuation. Taken literally, stripping punctuation **last** deletes it in place, so
`"k-means"` → `"kmeans"` while `"k means"` → `"k means"` — the two never match, which is
exactly the case the feature exists to handle.

This task therefore replaces each punctuation character with a **space** and collapses
whitespace **after** that. Amend the spec's "Accepted answers" section to the corrected
order and say why, in one sentence. This is a legitimate divergence found in implementation,
which is the kind the spec is kept current for.

- [ ] **Step 3: Write the failing tests — `backend/src/normalise.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::normalise;

    #[test]
    fn folds_case() {
        assert_eq!(normalise("K-Means"), normalise("k-means"));
    }

    #[test]
    fn punctuation_becomes_a_space_not_nothing() {
        // The whole point: hyphenated and spaced spellings must share a key.
        assert_eq!(normalise("k-means"), "k means");
        assert_eq!(normalise("k means"), "k means");
        assert_eq!(normalise("Bayes' theorem"), "bayes theorem");
    }

    #[test]
    fn collapses_and_trims_whitespace() {
        assert_eq!(normalise("  decision \t\n  tree  "), "decision tree");
    }

    #[test]
    fn applies_nfkc_compatibility_folding() {
        // Full-width K (U+FF2B) and the fi ligature (U+FB01) only fold under NFKC.
        assert_eq!(normalise("\u{FF2B}-means"), "k means");
        assert_eq!(normalise("con\u{FB01}dence"), "confidence");
    }

    #[test]
    fn digits_and_letters_survive() {
        assert_eq!(normalise("10,000 rows"), "10 000 rows");
    }

    #[test]
    fn is_idempotent() {
        let once = normalise("K-Means  Clustering!");
        assert_eq!(normalise(&once), once);
    }

    #[test]
    fn punctuation_only_input_is_empty() {
        assert_eq!(normalise("  ---  "), "");
    }
}
```

- [ ] **Step 4: Run — expect failure**

`cargo test normalise::` → FAIL, `normalise` not defined.

- [ ] **Step 5: Implement**

Put this **above** the `#[cfg(test)]` block:

```rust
//! The comparison key for short-answer grading.
//!
//! Computed once on insert into `accepted.normalised` so matching an answer is
//! an indexed lookup, not a scan that re-normalises every row. Pure and
//! DB-free, per the spec's "testable core"; Part 3's grading calls the same
//! function on the student's input.

use unicode_normalization::UnicodeNormalization;

/// NFKC, lowercase, punctuation to spaces, whitespace collapsed, trimmed.
///
/// Punctuation becomes a space rather than being deleted so that "k-means"
/// and "k means" produce the same key. Deleting it in place would make those
/// two spellings disagree, which is the case this exists to handle.
pub fn normalise(input: &str) -> String {
    let folded: String = input.nfkc().flat_map(char::to_lowercase).collect();

    let spaced: String = folded
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();

    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

`split_whitespace` + `join` does the collapse and the trim in one pass, so there is no
separate trim step to forget.

- [ ] **Step 6: Register the module**

In `backend/src/lib.rs`, add `pub mod normalise;` alongside the existing modules.

- [ ] **Step 7: Run — expect pass**

```bash
cargo test normalise::
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 8: Commit**

```bash
git add backend/src/normalise.rs backend/src/lib.rs backend/Cargo.toml Cargo.lock \
        docs/mitis/specs/2026-08-26-quiz-study-app-design.md
git commit -m "feat: answer normalisation as a pure comparison key"
```

```json:metadata
{"files":["backend/src/normalise.rs","backend/src/lib.rs","backend/Cargo.toml","docs/mitis/specs/2026-08-26-quiz-study-app-design.md"],"verifyCommand":"cargo test normalise:: && cargo clippy --all-targets -- -D warnings","acceptanceCriteria":["NFKC folds compatibility forms before comparison","case is folded","punctuation becomes a space so k-means equals k means","whitespace runs collapse and the result is trimmed","normalisation is idempotent","punctuation-only input normalises to empty","spec amended to record the corrected step order"],"modelTier":"mechanical"}
```

---

## Task 2: Let the caller name the foreign key that failed

**Goal:** A foreign-key violation reports the field that actually caused it, so the cards
editor can render `deck_id` inline instead of being told about `module_id`.

**Files:**
- Modify: `backend/src/error.rs`, `backend/src/routes/decks.rs`

**Acceptance Criteria:**
- [ ] An untagged FK violation is a `422` naming no field (`fields: []`), never a wrong one
- [ ] `AppError::fk_as` retags an FK violation with a caller-supplied field and message
- [ ] `fk_as` leaves every non-FK error untouched, including unique violations
- [ ] `POST /api/decks` and `PATCH /api/decks/:id` still return `422` with `field: "module_id"`
- [ ] All existing tests pass **unchanged**

**Verify:** `cargo test --test decks && cargo test error::` → all pass

**Steps:**

- [ ] **Step 1: Understand why parsing is not an option**

SQLite reports every foreign-key violation as the bare string `FOREIGN KEY constraint
failed`, with no column, table or constraint name. There is nothing to parse. Only the
calling handler knows which reference it was trying to satisfy, so the caller must say.

- [ ] **Step 2: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `backend/src/error.rs`:

```rust
#[test]
fn fk_as_leaves_non_fk_errors_alone() {
    let err = AppError::validation([("name", "Name must not be empty")])
        .fk_as("deck_id", "That deck does not exist");
    let (status, body) = err.parts();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body.fields[0].field, "name", "fk_as must not rewrite other errors");
}

#[test]
fn untagged_fk_violation_names_no_field() {
    // Regression guard: the blanket branch used to claim "module_id" for every
    // foreign key in the schema. Naming the wrong field is worse than naming none.
    let (status, body) = fk_error().parts();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.fields.is_empty());
}

#[test]
fn fk_as_tags_the_caller_s_field() {
    let (status, body) = fk_error()
        .fk_as("deck_id", "That deck does not exist")
        .parts();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body.fields[0].field, "deck_id");
    assert_eq!(body.fields[0].message, "That deck does not exist");
}
```

`fk_error()` needs a genuine `sqlx::Error` carrying an FK violation — construct it by running
a real violating statement against an in-memory SQLite database with foreign keys on, rather
than faking one:

```rust
/// A real SQLite foreign-key violation. `sqlx`'s `DatabaseError` cannot be
/// constructed by hand, so provoke one.
fn fk_error() -> AppError {
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::ConnectOptions;

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut conn = SqliteConnectOptions::new()
                .in_memory(true)
                .foreign_keys(true)
                .connect()
                .await
                .unwrap();
            sqlx::query("CREATE TABLE parent (id INTEGER PRIMARY KEY)")
                .execute(&mut conn).await.unwrap();
            sqlx::query(
                "CREATE TABLE child (parent_id INTEGER REFERENCES parent(id))")
                .execute(&mut conn).await.unwrap();
            let err = sqlx::query("INSERT INTO child (parent_id) VALUES (99)")
                .execute(&mut conn).await.unwrap_err();
            assert!(err.as_database_error().unwrap().is_foreign_key_violation());
            AppError::Db(err)
        })
}
```

- [ ] **Step 3: Run — expect failure**

`cargo test error::` → FAIL: `fk_as` undefined, and `untagged_fk_violation_names_no_field`
fails because the current branch hardcodes `module_id`.

- [ ] **Step 4: Implement `fk_as`**

Add to `impl AppError` in `backend/src/error.rs`:

```rust
/// Retags a foreign-key violation with the field that caused it.
///
/// SQLite reports every FK failure as the bare string "FOREIGN KEY
/// constraint failed" — no column, no table, nothing to parse. Only the
/// caller knows which reference it was satisfying, so the caller names it.
/// Any other error passes through unchanged.
pub fn fk_as(self, field: &str, message: &str) -> Self {
    let is_fk = matches!(&self, AppError::Db(e)
        if e.as_database_error().is_some_and(|d| d.is_foreign_key_violation()));

    if is_fk {
        AppError::validation([(field, message)])
    } else {
        self
    }
}
```

- [ ] **Step 5: Make the blanket branch honest**

In the `AppError::Db` arm of `parts()`, the `is_foreign_key_violation()` branch keeps its
`422` and its generic message but drops the fabricated field:

```rust
if dbe.is_foreign_key_violation() {
    // No field: SQLite does not say which reference failed, and a handler
    // that knows should have called `fk_as` before this point.
    return (
        StatusCode::UNPROCESSABLE_ENTITY,
        ErrorBody {
            error: "validation",
            message: "A referenced record does not exist".into(),
            fields: vec![],
        },
    );
}
```

- [ ] **Step 6: Tag the two deck call sites**

In `backend/src/routes/decks.rs`, both the `INSERT` in `create` and the `UPDATE` in `patch`
can violate the decks→modules FK. Replace their bare `?` with:

```rust
    .await
    .map_err(|e| AppError::from(e).fk_as("module_id", "That module does not exist"))?;
```

The existing `unknown_module_is_rejected` test in `backend/tests/decks.rs` asserts
`fields[0].field == "module_id"` and must pass **unchanged** — that is the proof this step
preserved behaviour rather than merely moved it.

- [ ] **Step 7: Run — expect pass**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 8: Commit**

```bash
git add backend/src/error.rs backend/src/routes/decks.rs
git commit -m "refactor: let handlers name the foreign key that failed"
```

```json:metadata
{"files":["backend/src/error.rs","backend/src/routes/decks.rs"],"verifyCommand":"cargo test && cargo clippy --all-targets -- -D warnings","acceptanceCriteria":["untagged FK violation is 422 with no field rather than a wrong one","fk_as retags an FK violation with the caller's field and message","fk_as passes non-FK errors through unchanged","POST and PATCH decks still return 422 with field module_id","every existing test passes unchanged"],"modelTier":"mechanical"}
```

---

## Task 3: `POST /api/cards` and `GET /api/cards/:id`

**Goal:** Create a card of any of the three kinds — children and schedule row in one
transaction — and read it back in the authoring view.

**Files:**
- Create: `backend/src/routes/cards.rs`, `backend/tests/cards.rs`
- Modify: `backend/src/routes/mod.rs`

**Acceptance Criteria:**
- [ ] `POST /api/cards` → `201` with the full card, for each of the three kinds
- [ ] `mc_single` requires ≥2 choices and exactly 1 correct; violations are `422` on `choices`
- [ ] `short_answer` requires ≥1 accepted and exactly 1 primary; violations are `422` on `accepted`
- [ ] `flashcard` requires a non-empty `answer_md`; violation is `422` on `answer_md`
- [ ] Children belonging to the wrong kind are rejected, naming that child field
- [ ] Empty `prompt_md` is `422` on `prompt_md`; an unknown `kind` is `422` on `kind`
- [ ] A non-existent `deck_id` is `422` naming `deck_id`
- [ ] `choices.position` is assigned from array order; `accepted.normalised` is computed server-side
- [ ] Exactly one `schedule` row exists per created card, `due_at` set
- [ ] A rejected create leaves **no** partial rows — no card, no children, no schedule row
- [ ] `GET /api/cards/:id` returns the full card with children in `position` order; unknown id is `404`

**Verify:** `cargo test --test cards` → all pass

**Steps:**

- [ ] **Step 1: Write the failing tests — `backend/tests/cards.rs`**

```rust
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
    assert_eq!(card["choices"][0]["text_md"], "Single", "children in position order");

    let (status, _) = app.get("/api/cards/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Extend the test harness with direct-pool helpers**

`a_rejected_create_leaves_nothing_behind` and `every_created_card_gets_a_schedule_row` assert
on rows the HTTP surface deliberately never exposes, so the harness needs a pool. Add to
`backend/tests/common/mod.rs`:

```rust
pub struct TestApp {
    pub router: Router,
    pub pool: sqlx::SqlitePool,   // NEW: for asserting on rows the API never returns
    _dir: tempfile::TempDir,
}
```

`spawn_app` keeps a clone of the pool before handing it to `AppState`, and:

```rust
impl TestApp {
    /// Scalar count, for asserting on tables the HTTP surface does not expose.
    pub async fn count(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }

    /// (row count, due_at) for a card's schedule row.
    pub async fn schedule_for(&self, card_id: i64) -> (i64, String) {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schedule WHERE card_id = ?")
            .bind(card_id).fetch_one(&self.pool).await.unwrap();
        let due: String = sqlx::query_scalar("SELECT due_at FROM schedule WHERE card_id = ?")
            .bind(card_id).fetch_one(&self.pool).await.unwrap();
        (n, due)
    }
}
```

The file already carries `#![allow(dead_code)]` for exactly this reason — helpers unused by
a given test binary must not trip clippy.

- [ ] **Step 3: Run — expect failure**

`cargo test --test cards` → FAIL (404s and a compile error).

- [ ] **Step 4: DTOs and validation — `backend/src/routes/cards.rs`**

```rust
//! Cards, and their kind-specific children.
//!
//! One `cards` table with a `kind` discriminator means the schema cannot
//! enforce per-kind invariants, so they are enforced here on every write —
//! see `validate`. Field names in the errors match the client's form
//! controls (`choices[1].text_md`) so the editor can render them inline.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, FieldError};
use crate::extract::AppJson;
use crate::normalise::normalise;
use crate::state::AppState;

const KINDS: [&str; 3] = ["mc_single", "short_answer", "flashcard"];

#[derive(Serialize)]
pub struct ChoiceDto {
    pub id: i64,
    pub text_md: String,
    pub is_correct: bool,
    pub position: i64,
}

#[derive(Serialize)]
pub struct AcceptedDto {
    pub id: i64,
    pub text: String,
    pub normalised: String,
    pub is_primary: bool,
}

/// List row. Deliberately without children: the list never renders them and
/// loading them for a 200-card deck would be pure waste.
#[derive(Serialize)]
pub struct CardSummaryDto {
    pub id: i64,
    pub deck_id: i64,
    pub kind: String,
    pub prompt_md: String,
    pub image_path: Option<String>,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Authoring view. Returns `is_correct` — the spec's answer-key leakage rule
/// governs the session endpoints, which do not exist yet and will have their
/// own DTOs.
#[derive(Serialize)]
pub struct CardDto {
    #[serde(flatten)]
    pub card: CardSummaryDto,
    pub choices: Vec<ChoiceDto>,
    pub accepted: Vec<AcceptedDto>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceInput {
    pub text_md: String,
    #[serde(default)]
    pub is_correct: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedInput {
    pub text: String,
    #[serde(default)]
    pub is_primary: bool,
}

/// The editable content of a card. `POST` wraps this with a `deck_id`;
/// `PATCH` (Task 5) uses it as-is and replaces the card wholesale.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardInput {
    pub kind: String,
    pub prompt_md: String,
    #[serde(default)]
    pub answer_md: Option<String>,
    #[serde(default)]
    pub explanation_md: Option<String>,
    #[serde(default)]
    pub choices: Vec<ChoiceInput>,
    #[serde(default)]
    pub accepted: Vec<AcceptedInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCard {
    pub deck_id: i64,
    #[serde(flatten)]
    pub card: CardInput,
}

/// A card that has passed validation: trimmed, kind-consistent, ready to write.
pub struct ValidCard {
    pub kind: String,
    pub prompt_md: String,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub choices: Vec<ChoiceInput>,
    pub accepted: Vec<AcceptedInput>,
}
```

Then the validator. It is a free function taking and returning owned data, with no pool
argument, so it stays unit-testable:

```rust
/// Enforces the per-kind invariants the schema cannot express (spec: "the
/// trade-off is that the schema cannot enforce per-kind invariants, so these
/// are validated in Rust on write"). Collects every problem rather than
/// stopping at the first, so the editor can highlight all of them at once.
pub fn validate(input: CardInput) -> AppResult<ValidCard> {
    let mut errors: Vec<FieldError> = Vec::new();
    let mut push = |field: &str, message: &str| {
        errors.push(FieldError { field: field.into(), message: message.into() })
    };

    if !KINDS.contains(&input.kind.as_str()) {
        // Nothing else can be judged without a kind, so fail immediately.
        return Err(AppError::validation([(
            "kind",
            "kind must be mc_single, short_answer or flashcard",
        )]));
    }

    let prompt_md = input.prompt_md.trim().to_string();
    if prompt_md.is_empty() {
        push("prompt_md", "A prompt is required");
    }

    let answer_md = input.answer_md.as_deref().map(str::trim).filter(|s| !s.is_empty())
        .map(str::to_string);
    let explanation_md = input.explanation_md.as_deref().map(str::trim)
        .filter(|s| !s.is_empty()).map(str::to_string);

    let choices: Vec<ChoiceInput> = input.choices.into_iter()
        .map(|c| ChoiceInput { text_md: c.text_md.trim().to_string(), ..c })
        .collect();
    let accepted: Vec<AcceptedInput> = input.accepted.into_iter()
        .map(|a| AcceptedInput { text: a.text.trim().to_string(), ..a })
        .collect();

    match input.kind.as_str() {
        "mc_single" => {
            if choices.len() < 2 {
                push("choices", "A multiple-choice card needs at least two options");
            }
            match choices.iter().filter(|c| c.is_correct).count() {
                1 => {}
                0 => push("choices", "Mark one option as correct"),
                _ => push("choices", "Only one option may be correct"),
            }
            for (i, c) in choices.iter().enumerate() {
                if c.text_md.is_empty() {
                    push(&format!("choices[{i}].text_md"), "An option cannot be blank");
                }
            }
            if !accepted.is_empty() {
                push("accepted", "Accepted answers belong to short-answer cards");
            }
            if answer_md.is_some() {
                push("answer_md", "An answer belongs to a flashcard");
            }
        }
        "short_answer" => {
            if accepted.is_empty() {
                push("accepted", "Add at least one accepted answer");
            }
            match accepted.iter().filter(|a| a.is_primary).count() {
                1 => {}
                0 => push("accepted", "Mark one answer as the primary wording"),
                _ => push("accepted", "Only one answer may be the primary wording"),
            }
            for (i, a) in accepted.iter().enumerate() {
                if a.text.is_empty() {
                    push(&format!("accepted[{i}].text"), "An answer cannot be blank");
                }
            }
            if !choices.is_empty() {
                push("choices", "Options belong to multiple-choice cards");
            }
            if answer_md.is_some() {
                push("answer_md", "An answer belongs to a flashcard");
            }
        }
        "flashcard" => {
            if answer_md.is_none() {
                push("answer_md", "A flashcard needs an answer");
            }
            if !choices.is_empty() {
                push("choices", "Options belong to multiple-choice cards");
            }
            if !accepted.is_empty() {
                push("accepted", "Accepted answers belong to short-answer cards");
            }
        }
        _ => unreachable!("kind was checked above"),
    }

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    Ok(ValidCard {
        kind: input.kind, prompt_md, answer_md, explanation_md, choices, accepted,
    })
}
```

Note the ordering inside each arm: the cardinality errors (`choices`, `accepted`) are pushed
before the per-row ones, because several tests assert on `fields[0]`. Keep it.

- [ ] **Step 5: Reads**

```rust
async fn fetch_summary(pool: &sqlx::SqlitePool, id: i64) -> AppResult<CardSummaryDto> {
    sqlx::query_as!(
        CardSummaryDto,
        r#"SELECT id AS "id!: i64", deck_id AS "deck_id!: i64", kind,
                  prompt_md, image_path, answer_md, explanation_md,
                  archived AS "archived!: bool", created_at, updated_at
           FROM cards WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("card"))
}

async fn fetch_full(pool: &sqlx::SqlitePool, id: i64) -> AppResult<CardDto> {
    let card = fetch_summary(pool, id).await?;

    let choices = sqlx::query_as!(
        ChoiceDto,
        r#"SELECT id AS "id!: i64", text_md, is_correct AS "is_correct!: bool",
                  position AS "position!: i64"
           FROM choices WHERE card_id = ? ORDER BY position"#,
        id
    )
    .fetch_all(pool)
    .await?;

    let accepted = sqlx::query_as!(
        AcceptedDto,
        r#"SELECT id AS "id!: i64", text, normalised, is_primary AS "is_primary!: bool"
           FROM accepted WHERE card_id = ? ORDER BY is_primary DESC, id"#,
        id
    )
    .fetch_all(pool)
    .await?;

    Ok(CardDto { card, choices, accepted })
}

async fn get_one(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<CardDto>> {
    Ok(Json(fetch_full(&st.pool, id).await?))
}
```

`accepted` orders primary-first so the editor's first row is the one shown as "the answer".

- [ ] **Step 6: The create handler — one transaction**

```rust
async fn create(
    State(st): State<AppState>,
    AppJson(body): AppJson<CreateCard>,
) -> AppResult<(StatusCode, Json<CardDto>)> {
    let valid = validate(body.card)?;
    let deck_id = body.deck_id;

    let mut tx = st.pool.begin().await?;

    // Card, children and schedule row go in together or not at all: a card
    // without its choices is unanswerable, and one without a schedule row
    // would need a migration when SM-2 lands.
    let id = sqlx::query_scalar!(
        r#"INSERT INTO cards (deck_id, kind, prompt_md, answer_md, explanation_md)
           VALUES (?, ?, ?, ?, ?) RETURNING id"#,
        deck_id, valid.kind, valid.prompt_md, valid.answer_md, valid.explanation_md
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::from(e).fk_as("deck_id", "That deck does not exist"))?;

    write_children(&mut tx, id, &valid).await?;

    sqlx::query!(
        r#"INSERT INTO schedule (card_id, due_at)
           VALUES (?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))"#,
        id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(fetch_full(&st.pool, id).await?)))
}

/// Inserts the kind-appropriate children. Task 5 reuses this after deleting
/// the old ones, which is why it takes a transaction rather than a pool.
async fn write_children(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    card_id: i64,
    valid: &ValidCard,
) -> AppResult<()> {
    for (i, c) in valid.choices.iter().enumerate() {
        let position = i as i64;
        sqlx::query!(
            "INSERT INTO choices (card_id, text_md, is_correct, position)
             VALUES (?, ?, ?, ?)",
            card_id, c.text_md, c.is_correct, position
        )
        .execute(&mut **tx)
        .await?;
    }
    for a in &valid.accepted {
        // The comparison key is computed once here, on write, so grading is an
        // indexed lookup rather than a scan that re-normalises every row.
        let key = normalise(&a.text);
        sqlx::query!(
            "INSERT INTO accepted (card_id, text, normalised, is_primary)
             VALUES (?, ?, ?, ?)",
            card_id, a.text, key, a.is_primary
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
```

Dropping `tx` without committing rolls back, so an error anywhere above leaves no partial
rows — that is what `a_rejected_create_leaves_nothing_behind` proves.

- [ ] **Step 7: Router, for now**

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cards", axum::routing::post(create))
        .route("/cards/{id}", get(get_one))
}
```

Task 4 adds `.get(list)` to `/cards`; Task 5 adds the patch and archive routes. Register it
in `backend/src/routes/mod.rs`: `pub mod cards;` and `.merge(cards::router())`.

- [ ] **Step 8: Regenerate the cache and run**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
cargo test --test cards
cargo clippy --all-targets -- -D warnings
```

The list assertion inside `a_rejected_create_leaves_nothing_behind` needs `GET /api/cards`
from Task 4. Until then, assert `app.count("SELECT COUNT(*) FROM cards").await == 0` instead
and switch it to the HTTP call in Task 4.

- [ ] **Step 9: Commit**

```bash
git add backend/src/routes backend/tests .sqlx
git commit -m "feat: create and read cards of all three kinds"
```

```json:metadata
{"files":["backend/src/routes/cards.rs","backend/src/routes/mod.rs","backend/tests/cards.rs","backend/tests/common/mod.rs",".sqlx"],"verifyCommand":"cargo test --test cards && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build","acceptanceCriteria":["POST /api/cards returns 201 with the full card for each of the three kinds","mc_single requires at least two choices and exactly one correct, erroring on choices","short_answer requires at least one accepted and exactly one primary, erroring on accepted","flashcard requires a non-empty answer_md","children belonging to the wrong kind are rejected naming that child field","empty prompt_md and unknown kind are 422 on their own fields","non-existent deck_id is 422 naming deck_id","position comes from array order and normalised is computed server-side","exactly one schedule row per created card with due_at set","a rejected create leaves no card, child or schedule rows","GET /api/cards/:id returns children in position order and 404s on an unknown id"],"modelTier":"standard"}
```

---

## Task 4: `GET /api/cards`

**Goal:** List a deck's cards, filtered by kind and archived state, in authoring order, from
one parameterized query.

**Files:**
- Modify: `backend/src/routes/cards.rs`, `backend/tests/cards.rs`

**Acceptance Criteria:**
- [ ] `?deck_id=<n>` returns only that deck's cards; cards from other decks never appear
- [ ] Absent `deck_id` returns cards across all decks
- [ ] `?archived=` defaults to excluding archived cards; `true` returns only archived; `all` returns both
- [ ] `?kind=` accepts `all` (default) or one of the three kinds
- [ ] Rows are `CardSummaryDto` — **no** `choices` or `accepted` keys
- [ ] Ordered oldest-first by `created_at`, tie-broken by `id` ascending
- [ ] `?kind=essay` → `422` on `kind`; `?archived=maybe` → `422` on `archived`;
      `?deck_id=abc` → `422` on `deck_id`
- [ ] Exactly **one** `query_as!` serves every combination

**Verify:** `cargo test --test cards` → all pass

**Steps:**

- [ ] **Step 1: Write the failing tests**

Append to `backend/tests/cards.rs`. The ordering test must be able to detect the tiebreak, so
it relies on same-second creation — which is the normal case at one-second resolution, not a
contrivance:

```rust
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
```

The archived-filter test belongs with the archive endpoint and lands in Task 5.

- [ ] **Step 2: Run — expect failure**

`cargo test --test cards` → FAIL.

- [ ] **Step 3: Implement the list handler**

One query, branching on bound parameters. Repeated plain `?` placeholders only:

```rust
#[derive(Deserialize)]
pub struct ListQuery {
    pub deck_id: Option<String>,
    /// "all" (default) or one of the three kinds.
    pub kind: Option<String>,
    /// "false" (default), "true", or "all".
    pub archived: Option<String>,
}

async fn list(
    State(st): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<CardSummaryDto>>> {
    let deck_id = match q.deck_id.as_deref() {
        None | Some("") => None,
        Some(raw) => Some(raw.parse::<i64>().map_err(|_| {
            AppError::validation([("deck_id", "deck_id must be a number")])
        })?),
    };

    let kind = q.kind.as_deref().unwrap_or("all").to_string();
    if kind != "all" && !KINDS.contains(&kind.as_str()) {
        return Err(AppError::validation([(
            "kind",
            "kind must be mc_single, short_answer, flashcard or \"all\"",
        )]));
    }

    let archived = q.archived.as_deref().unwrap_or("false").to_string();
    if !["false", "true", "all"].contains(&archived.as_str()) {
        return Err(AppError::validation([(
            "archived",
            "archived must be \"true\", \"false\" or \"all\"",
        )]));
    }

    // Oldest first: a deck reads in the order it was written. The id tiebreak
    // is load-bearing, not decoration — timestamps have one-second resolution,
    // so a burst of save-and-next cards all share a created_at and would
    // otherwise come back in SQLite's incidental scan order.
    let rows = sqlx::query_as!(
        CardSummaryDto,
        r#"SELECT id AS "id!: i64", deck_id AS "deck_id!: i64", kind,
                  prompt_md, image_path, answer_md, explanation_md,
                  archived AS "archived!: bool", created_at, updated_at
           FROM cards
           WHERE (? IS NULL OR deck_id = ?)
             AND (? = 'all' OR kind = ?)
             AND (? = 'all'
                  OR (? = 'true'  AND archived = 1)
                  OR (? = 'false' AND archived = 0))
           ORDER BY created_at ASC, id ASC"#,
        deck_id, deck_id, kind, kind, archived, archived, archived
    )
    .fetch_all(&st.pool)
    .await?;

    Ok(Json(rows))
}
```

- [ ] **Step 4: Route it**

```rust
.route("/cards", get(list).post(create))
```

- [ ] **Step 5: Switch the rollback test to HTTP**

In `a_rejected_create_leaves_nothing_behind`, replace the direct `cards` count placeholder
from Task 3 with the `GET /api/cards?deck_id=` assertion as originally written. The
`choices` and `schedule` counts stay on the pool — no endpoint exposes them.

- [ ] **Step 6: Prove each filter independently**

Mutation evidence only counts when one thing changes at a time. For each of the three
`WHERE` clauses, delete only that clause, confirm only its test goes red, and restore it. Do
the same for `id ASC` in the `ORDER BY`. Record the result in the execution ledger.

- [ ] **Step 7: Regenerate, run, commit**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
cargo test --test cards && cargo clippy --all-targets -- -D warnings
git add backend/src/routes/cards.rs backend/tests/cards.rs .sqlx
git commit -m "feat: list cards by deck, kind and archived state"
```

```json:metadata
{"files":["backend/src/routes/cards.rs","backend/tests/cards.rs",".sqlx"],"verifyCommand":"cargo test --test cards && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build","acceptanceCriteria":["deck_id filters to one deck and absent deck_id lists all","archived defaults to excluding archived, true and all behave as documented","kind accepts all or one of the three kinds","rows carry no choices or accepted keys","ordered by created_at then id ascending, with the id tiebreak proven on same-second rows","invalid kind, archived and deck_id each return 422 on their own field","exactly one query_as! serves every combination"],"modelTier":"standard"}
```

---

## Task 5: `PATCH /api/cards/:id`, archive, unarchive

**Goal:** Edit a card — including changing its kind — with children replaced in one
transaction, and archive or restore it.

**Files:**
- Modify: `backend/src/routes/cards.rs`, `backend/tests/cards.rs`

**Acceptance Criteria:**
- [ ] `PATCH` replaces prompt, answer, explanation, kind and all children
- [ ] Changing `mc_single` → `flashcard` leaves **no** orphaned `choices` rows
- [ ] Changing `mc_single` → `short_answer` leaves no choices and writes `accepted` with keys
- [ ] `PATCH` re-runs the full per-kind validation against the **new** kind
- [ ] A rejected `PATCH` leaves the stored card **completely** unchanged, children included
- [ ] `PATCH` bumps `updated_at`; `PATCH` on an unknown id is `404` and writes nothing
- [ ] `POST /api/cards/:id/archive` sets `archived`, drops the card from the default list, and
      `?archived=true` finds it
- [ ] `POST /api/cards/:id/unarchive` restores it; both are `404` on an unknown id
- [ ] Archiving is idempotent, and a card's `reviews` history is never touched

**Verify:** `cargo test --test cards` → all pass

**Steps:**

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn patch_replaces_content_and_bumps_updated_at() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", mc(d)).await;
    let id = created["id"].as_i64().unwrap();

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
}

#[tokio::test]
async fn changing_kind_clears_the_other_kind_s_children() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", mc(d)).await;
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
}

#[tokio::test]
async fn a_rejected_patch_changes_nothing() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", mc(d)).await;
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
    assert_eq!(after["choices"].as_array().unwrap().len(), 2);
    assert_eq!(after["choices"][0]["text_md"], "Single", "children untouched");
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
}

#[tokio::test]
async fn archive_hides_the_card_and_unarchive_restores_it() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", mc(d)).await;
    let id = created["id"].as_i64().unwrap();

    let (status, archived) = app.post(&format!("/api/cards/{id}/archive"), json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(archived["archived"], true);

    let (_, default_list) = app.get(&format!("/api/cards?deck_id={d}")).await;
    assert_eq!(default_list.as_array().unwrap().len(), 0);

    let (_, archived_list) = app
        .get(&format!("/api/cards?deck_id={d}&archived=true")).await;
    assert_eq!(archived_list.as_array().unwrap().len(), 1);

    let (_, all) = app.get(&format!("/api/cards?deck_id={d}&archived=all")).await;
    assert_eq!(all.as_array().unwrap().len(), 1);

    // Archiving twice is a no-op, not an error — the UI can fire it twice.
    let (status, _) = app.post(&format!("/api/cards/{id}/archive"), json!({})).await;
    assert_eq!(status, StatusCode::OK);

    let (_, restored) = app.post(&format!("/api/cards/{id}/unarchive"), json!({})).await;
    assert_eq!(restored["archived"], false);
    let (_, back) = app.get(&format!("/api/cards?deck_id={d}")).await;
    assert_eq!(back.as_array().unwrap().len(), 1);
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
    let d = deck(&app, "Test 1").await;
    let (_, created) = app.post("/api/cards", mc(d)).await;
    let id = created["id"].as_i64().unwrap();

    let (_, before) = app.get(&format!("/api/decks?module_id=all")).await;
    assert_eq!(before[0]["card_count"], 1);

    app.post(&format!("/api/cards/{id}/archive"), json!({})).await;
    let (_, after) = app.get(&format!("/api/decks?module_id=all")).await;
    assert_eq!(after[0]["card_count"], 0, "the decks query already filters archived = 0");
}

#[tokio::test]
async fn a_deck_s_card_count_reflects_created_cards() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Test 1").await;
    app.post("/api/cards", mc(d)).await;
    let (_, decks) = app.get("/api/decks").await;
    assert_eq!(decks[0]["card_count"], 1);
}
```

The last two also close the Part 1 gap where `DeckDto.card_count` was only ever asserted at
zero, because no endpoint could create a card.

- [ ] **Step 2: Run — expect failure**

`cargo test --test cards` → FAIL.

- [ ] **Step 3: Implement `patch`**

```rust
/// Full replace of a card's editable content.
///
/// Not a field-by-field patch, and deliberately not the absent-vs-null dance
/// `PATCH /api/decks/:id` needs: the editor always holds the whole card and
/// always submits the whole card, so an omitted optional means null. It is a
/// PATCH by route because the spec's API table says so. Cards do not move
/// between decks in 2a.
async fn patch(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    AppJson(body): AppJson<CardInput>,
) -> AppResult<Json<CardDto>> {
    // 404 before validation and before any write, matching decks::patch.
    fetch_summary(&st.pool, id).await?;
    let valid = validate(body)?;

    let mut tx = st.pool.begin().await?;

    sqlx::query!(
        r#"UPDATE cards
              SET kind = ?, prompt_md = ?, answer_md = ?, explanation_md = ?,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = ?"#,
        valid.kind, valid.prompt_md, valid.answer_md, valid.explanation_md, id
    )
    .execute(&mut *tx)
    .await?;

    // Both child tables are cleared regardless of kind: a kind change must not
    // leave rows that would resurface if the kind changed back.
    sqlx::query!("DELETE FROM choices WHERE card_id = ?", id).execute(&mut *tx).await?;
    sqlx::query!("DELETE FROM accepted WHERE card_id = ?", id).execute(&mut *tx).await?;
    write_children(&mut tx, id, &valid).await?;

    tx.commit().await?;

    Ok(Json(fetch_full(&st.pool, id).await?))
}
```

Validation runs **before** the transaction opens, so `a_rejected_patch_changes_nothing`
passes for the plain reason that nothing was ever written — not because a rollback happened
to work.

- [ ] **Step 4: Implement archive and unarchive**

```rust
async fn archive(State(st): State<AppState>, Path(id): Path<i64>)
    -> AppResult<Json<CardDto>> {
    set_archived(&st, id, true).await
}

async fn unarchive(State(st): State<AppState>, Path(id): Path<i64>)
    -> AppResult<Json<CardDto>> {
    set_archived(&st, id, false).await
}

/// Cards are archived, never deleted: a hard delete would orphan the card's
/// `reviews` rows and silently rewrite history. `reviews.card_id` has no
/// ON DELETE CASCADE for the same reason.
async fn set_archived(st: &AppState, id: i64, archived: bool) -> AppResult<Json<CardDto>> {
    fetch_summary(&st.pool, id).await?;   // 404 before the write

    sqlx::query!(
        r#"UPDATE cards
              SET archived = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = ?"#,
        archived, id
    )
    .execute(&st.pool)
    .await?;

    Ok(Json(fetch_full(&st.pool, id).await?))
}
```

Both handlers take no body. They are `POST` per the spec's API table, and the harness's
`post` helper sends `{}` with a JSON content type, which axum ignores when no extractor asks
for it.

- [ ] **Step 5: Complete the router**

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cards", get(list).post(create))
        .route("/cards/{id}", get(get_one).patch(patch))
        .route("/cards/{id}/archive", axum::routing::post(archive))
        .route("/cards/{id}/unarchive", axum::routing::post(unarchive))
}
```

- [ ] **Step 6: Regenerate, run, commit**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
cargo test && cargo clippy --all-targets -- -D warnings
git add backend/src/routes/cards.rs backend/tests/cards.rs .sqlx
git commit -m "feat: edit, archive and restore cards"
```

```json:metadata
{"files":["backend/src/routes/cards.rs","backend/tests/cards.rs",".sqlx"],"verifyCommand":"cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build","acceptanceCriteria":["PATCH replaces prompt, answer, explanation, kind and all children","changing kind leaves no orphaned choices or accepted rows","PATCH revalidates against the new kind","a rejected PATCH leaves the card and its children completely unchanged","PATCH bumps updated_at and 404s on an unknown id without writing","archive drops the card from the default list and archived=true finds it","unarchive restores it; both 404 on an unknown id","archiving twice is a no-op","archived cards stop counting toward a deck's card_count"],"modelTier":"standard"}
```

---

## Task 6: API client and the `/decks/:id` card list

**Goal:** Open a deck and see its cards, with edit, archive and a show-archived toggle.

**Files:**
- Create: `frontend/src/pages/DeckPage.tsx`
- Modify: `frontend/src/lib/api.ts`, `frontend/src/App.tsx`, `frontend/src/components/DeckCard.tsx`
- Add via the shadcn CLI: `frontend/src/components/ui/switch.tsx`

**Acceptance Criteria:**
- [ ] `/decks/:id` shows the deck's name, module badge, description and card count
- [ ] Each row shows a kind badge and the first line of `prompt_md` as raw text
- [ ] "New card" goes to `/cards/new?deck_id=:id`; a row's edit goes to `/cards/:id/edit`
- [ ] Archiving a row removes it from the list without a full page reload
- [ ] The show-archived toggle reveals archived rows, visibly muted, offering unarchive
- [ ] An empty deck shows an empty state pointing at "New card"
- [ ] A deck id that does not exist renders a not-found state, not a crash or a blank page
- [ ] A deck card on `/decks` opens the deck; its edit and module-filter buttons still work
- [ ] `pnpm exec tsc --noEmit` and `pnpm build` are clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build`, then the manual walkthrough

**Steps:**

- [ ] **Step 1: Extend `frontend/src/lib/api.ts`**

Types mirror the contract at the top of this plan exactly:

```ts
export type CardKind = 'mc_single' | 'short_answer' | 'flashcard'

export type Choice = { id: number; text_md: string; is_correct: boolean; position: number }
export type Accepted = { id: number; text: string; normalised: string; is_primary: boolean }

export type CardSummary = {
  id: number
  deck_id: number
  kind: CardKind
  prompt_md: string
  image_path: string | null
  answer_md: string | null
  explanation_md: string | null
  archived: boolean
  created_at: string
  updated_at: string
}

export type Card = CardSummary & { choices: Choice[]; accepted: Accepted[] }

/** `position` and `normalised` are server-assigned; never send them. */
export type ChoiceInput = { text_md: string; is_correct: boolean }
export type AcceptedInput = { text: string; is_primary: boolean }

/**
 * The whole editable card. Unlike `updateDeck`, this is a full replace, not a
 * sparse patch — the editor always holds the entire card, so an omitted
 * optional means null on the server.
 */
export type CardInput = {
  kind: CardKind
  prompt_md: string
  answer_md?: string | null
  explanation_md?: string | null
  choices?: ChoiceInput[]
  accepted?: AcceptedInput[]
}

export type CardQuery = {
  deckId?: number
  kind?: CardKind | 'all'
  archived?: 'true' | 'false' | 'all'
}

function cardQueryString({ deckId, kind, archived }: CardQuery): string {
  const params = new URLSearchParams()
  if (deckId !== undefined) params.set('deck_id', String(deckId))
  if (kind && kind !== 'all') params.set('kind', kind)
  if (archived) params.set('archived', archived)
  const s = params.toString()
  return s === '' ? '' : `?${s}`
}
```

Add to the `api` object, keeping the existing `request<T>` style:

```ts
  getDeck: (id: number, signal?: AbortSignal) =>
    request<Deck>('GET', `/decks/${id}`, undefined, signal),
  listCards: (query: CardQuery = {}, signal?: AbortSignal) =>
    request<CardSummary[]>('GET', `/cards${cardQueryString(query)}`, undefined, signal),
  getCard: (id: number, signal?: AbortSignal) =>
    request<Card>('GET', `/cards/${id}`, undefined, signal),
  createCard: (input: CardInput & { deck_id: number }) =>
    request<Card>('POST', '/cards', input),
  updateCard: (id: number, input: CardInput) =>
    request<Card>('PATCH', `/cards/${id}`, input),
  archiveCard: (id: number) => request<Card>('POST', `/cards/${id}/archive`, {}),
  unarchiveCard: (id: number) => request<Card>('POST', `/cards/${id}/unarchive`, {}),
```

- [ ] **Step 2: Add `GET /api/decks/:id` to the backend**

`getDeck` needs an endpoint that does not exist — `DeckPage` must render a deck header
without loading every deck. It is three lines, because `decks::fetch_one` is already written:

```rust
async fn get_one(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<DeckDto>> {
    Ok(Json(fetch_one(&st.pool, id).await?))
}
```

Route it as `.route("/decks/{id}", get(get_one).patch(patch))`, and add two tests to
`backend/tests/decks.rs`: a fetch by id returning the same shape the list returns, and a
`404` on an unknown id. Note this as an addition to the spec's API table in Task 8.

- [ ] **Step 3: Add the switch primitive**

```bash
cd frontend && pnpm dlx shadcn@latest add switch
```

- [ ] **Step 4: Build `frontend/src/pages/DeckPage.tsx`**

Structure, following the data-loading shape `DecksPage` already uses — `useCallback` loaders,
an `AbortController` cleanup on unmount, and a stale-response guard:

- `useParams()` for the deck id; a non-numeric or unknown id renders a not-found state with a
  link back to `/decks`
- State: `deck`, `cards`, `showArchived`, `loading`, `notFound`
- `loadCards` passes `archived: showArchived ? 'all' : 'false'`, so the toggle shows archived
  rows *alongside* live ones rather than instead of them
- Header: deck name in `font-display`, module badge (reusing the `Badge` variants
  `DeckCard.tsx` uses), description, card count
- "New card" → `navigate(\`/cards/new?deck_id=${deck.id}\`)`
- Rows: kind badge (`Multiple choice` / `Short answer` / `Flashcard`), the first line of
  `prompt_md` truncated, then edit and archive/unarchive buttons. Archived rows get
  `opacity-60` and an `Archived` badge
- `firstLine(prompt_md)` = `prompt_md.split('\n')[0]`, rendered as plain text. **No markdown
  rendering** — that is 2b, and it arrives for every call site at once
- Archive and unarchive call the API then refresh the list; failures raise a toast and leave
  the row as it was
- Empty state: "No cards yet" plus the "New card" action

- [ ] **Step 5: Route it**

In `frontend/src/App.tsx`, inside the `AppShell` route:

```tsx
<Route path="/decks/:id" element={<DeckPage />} />
```

- [ ] **Step 6: Open the deck from `/decks`**

In `frontend/src/components/DeckCard.tsx`, make the body area a link to `/decks/${deck.id}`.
The header band's module chip and edit button are already `<button>` elements and must keep
working — nest the link on the body only, rather than wrapping the whole `<article>`, so
there are no interactive elements inside an anchor.

- [ ] **Step 7: Verify**

```bash
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

Then, with `cargo run` and `pnpm dev` (port **5273**): open a deck from `/decks`, confirm the
empty state, and — after Task 7 exists — the list, archive, toggle and unarchive round trip.

- [ ] **Step 8: Commit**

```bash
git add frontend/src backend/src/routes/decks.rs backend/tests/decks.rs
git commit -m "feat: deck detail page listing a deck's cards"
```

```json:metadata
{"files":["frontend/src/pages/DeckPage.tsx","frontend/src/lib/api.ts","frontend/src/App.tsx","frontend/src/components/DeckCard.tsx","frontend/src/components/ui/switch.tsx","backend/src/routes/decks.rs","backend/tests/decks.rs"],"verifyCommand":"cargo test --test decks && cargo clippy --all-targets -- -D warnings && cd frontend && pnpm exec tsc --noEmit && pnpm build","acceptanceCriteria":["/decks/:id shows deck name, module badge, description and card count","each row shows a kind badge and the first line of prompt_md as raw text","New card and per-row edit navigate to the right routes","archiving a row removes it from the list without a page reload","the show-archived toggle reveals muted archived rows offering unarchive","an empty deck shows an empty state pointing at New card","an unknown deck id renders a not-found state","GET /api/decks/:id returns the deck and 404s on an unknown id","a deck card on /decks opens the deck with its existing buttons still working","tsc --noEmit and pnpm build are clean"],"modelTier":"standard"}
```

---

## Task 7: The card editor

**Goal:** Write and edit cards of all three kinds without touching the mouse.

**Files:**
- Create: `frontend/src/pages/CardEditorPage.tsx`,
  `frontend/src/components/card-editor/ChoicesEditor.tsx`,
  `frontend/src/components/card-editor/AcceptedEditor.tsx`
- Modify: `frontend/src/App.tsx`
- Add via the shadcn CLI: `frontend/src/components/ui/radio-group.tsx`

**Acceptance Criteria:**
- [ ] `/cards/new?deck_id=<n>` creates; `/cards/:id/edit` loads and updates
- [ ] All three kinds are authorable; switching kind keeps the prompt and explanation
- [ ] `mc_single`: a choices list with a radio marking exactly one correct
- [ ] `short_answer`: an accepted-answers list with a radio marking the primary wording
- [ ] `flashcard`: an answer textarea
- [ ] The prompt is autofocused on mount; every action is reachable by keyboard
- [ ] `Cmd/Ctrl+Enter` saves and starts the next card, keeping deck and kind and refocusing the prompt
- [ ] `Cmd/Ctrl+S` saves and returns to the deck; `Escape` returns without saving
- [ ] `Enter` in the last choice/accepted row appends a row and focuses it
- [ ] Inline field errors render beside the right control, including `choices[1].text_md`
- [ ] **A rejected save never clears typed content**
- [ ] Saving twice in a row cannot double-create (the save button and shortcuts disable while busy)
- [ ] `pnpm exec tsc --noEmit` and `pnpm build` are clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build`, then the manual walkthrough

**Steps:**

- [ ] **Step 1: Add the radio-group primitive**

```bash
cd frontend && pnpm dlx shadcn@latest add radio-group
```

- [ ] **Step 2: `ChoicesEditor`**

Props: `value: ChoiceInput[]`, `onChange`, `errors: Record<string, string>`.

- One row per choice: a radio (the correct marker, a single `RadioGroup` across all rows so
  exactly one can be chosen) and an `Input` for `text_md`
- Add row, remove row; refuse to drop below two rows for `mc_single`
- `Enter` in the **last** row's input appends a row and focuses it; in any other row it moves
  focus to the next row's input. Do **not** let it submit the form
- Per-row errors come from `errors['choices[' + i + '].text_md']`; the list-level error from
  `errors.choices`, rendered once above the list
- A `ref` array so the newly added row can be focused after render

- [ ] **Step 3: `AcceptedEditor`**

The same shape, over `AcceptedInput[]`: radio marks `is_primary` rather than `is_correct`,
the input is `text`, the minimum is one row, and errors key off `accepted` and
`accepted[i].text`. Label the radio column "Shown as the answer", because that is what
`is_primary` means to the person studying.

Keep the two components separate rather than generalising them into one. They differ in
field names, minimum row count and labelling, and a shared abstraction over three
differences would be harder to read than the duplication.

- [ ] **Step 4: `CardEditorPage`**

```tsx
// Route: /cards/new?deck_id=<n>  and  /cards/:id/edit
```

- Mode comes from the route: `:id` present → edit, otherwise create with `deck_id` from the
  query string. A create without a valid `deck_id` renders an error with a link to `/decks`
- Edit mode loads via `api.getCard(id)` and seeds the form, mapping `choices` and `accepted`
  down to their `*Input` shapes (dropping `id`, `position`, `normalised`)
- State: `kind`, `promptMd`, `answerMd`, `explanationMd`, `choices`, `accepted`, `errors`,
  `busy`
- Switching kind keeps `promptMd` and `explanationMd` and keeps each kind's own child state,
  so flipping to flashcard and back does not lose typed choices. The server clears whatever
  does not belong to the saved kind, so only the active kind's children are ever sent
- Submitting builds a `CardInput` carrying only the active kind's children:
  `mc_single` → `choices`; `short_answer` → `accepted`; `flashcard` → `answer_md`

Saving, following `DeckDialog`'s error handling exactly:

```tsx
async function save(): Promise<Card | null> {
  setBusy(true)
  setErrors({})
  try {
    return mode === 'edit'
      ? await api.updateCard(cardId, buildInput())
      : await api.createCard({ deck_id: deckId, ...buildInput() })
  } catch (e) {
    // Never reset the form here — a rejected save must keep what was typed.
    if (e instanceof ApiError) {
      const byField = e.byField()
      setErrors(byField)
      if (Object.keys(byField).length === 0) toast.error(e.message)
    } else {
      toast.error('Could not reach the server')
    }
    return null
  } finally {
    setBusy(false)
  }
}
```

Keyboard handling, on a container `onKeyDown` so it works wherever focus sits:

| Keys | Action |
| --- | --- |
| `Cmd/Ctrl+Enter` | save, then start the next card: keep `deck_id` and `kind`, clear prompt, answer, explanation and children back to their empty rows, refocus the prompt, toast "Card saved" |
| `Cmd/Ctrl+S` | save, then `navigate(\`/decks/${deckId}\`)` |
| `Escape` | `navigate(\`/decks/${deckId}\`)` without saving |

Guard every shortcut on `!busy`, and `preventDefault()` on `Cmd+S` so the browser's save
dialog never opens. In edit mode, `Cmd+Enter` saves and returns rather than starting a new
card — "next" only means anything while authoring a run of cards.

Visible hints for the shortcuts belong next to the save button. A keyboard-first screen whose
shortcuts are invisible is a mouse-first screen with extra steps.

- [ ] **Step 5: Route both paths**

```tsx
<Route path="/cards/new" element={<CardEditorPage />} />
<Route path="/cards/:id/edit" element={<CardEditorPage />} />
```

- [ ] **Step 6: Verify**

```bash
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

Then the full end-to-end walkthrough in the Verification section below.

- [ ] **Step 7: Commit**

```bash
git add frontend/src
git commit -m "feat: keyboard-first card editor for all three kinds"
```

```json:metadata
{"files":["frontend/src/pages/CardEditorPage.tsx","frontend/src/components/card-editor/ChoicesEditor.tsx","frontend/src/components/card-editor/AcceptedEditor.tsx","frontend/src/App.tsx","frontend/src/components/ui/radio-group.tsx"],"verifyCommand":"cd frontend && pnpm exec tsc --noEmit && pnpm build","acceptanceCriteria":["/cards/new?deck_id= creates and /cards/:id/edit loads and updates","all three kinds are authorable and switching kind keeps prompt and explanation","mc_single marks exactly one correct choice via a radio group","short_answer marks one primary accepted answer","flashcard offers an answer textarea","prompt is autofocused and every action is keyboard reachable","Cmd/Ctrl+Enter saves and starts the next card keeping deck and kind","Cmd/Ctrl+S saves and returns; Escape returns without saving","Enter in the last child row appends and focuses a new row","inline field errors render beside the right control including indexed child paths","a rejected save never clears typed content","saving cannot double-create while busy","tsc --noEmit and pnpm build are clean"],"modelTier":"standard"}
```

---

## Task 8: Whole-plan verification and documentation

**Goal:** The gate passes end to end, and the written record matches what was built.

**Files:**
- Modify: `README.md`, `docs/HANDOVER.md`,
  `docs/mitis/specs/2026-08-26-quiz-study-app-design.md`

**Acceptance Criteria:**
- [ ] The full verification gate passes from a clean checkout
- [ ] The manual walkthrough below is completed and its results recorded
- [ ] `README.md` documents the new routes and endpoints
- [ ] `docs/HANDOVER.md` reflects the new state of play, and the resolved FK-field item is
      removed from "Known-and-accepted minors"
- [ ] The spec's API table gains `GET /api/decks/:id` and `POST /api/cards/:id/unarchive`
- [ ] Any remaining browser-only checks are listed under "Needs a human at a browser"

**Verify:** the gate below, all four commands green

**Steps:**

- [ ] **Step 1: Run the gate**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

`--all-targets` matters: plain `cargo clippy -- -D warnings` does not build test targets, and
this plan adds several hundred lines of them.

- [ ] **Step 2: Review across the seam**

Tasks 3–5 and Tasks 6–7 are the two halves of one contract, and a per-task review cannot see
across it. Run one review over the combined diff of all seven tasks, with the JSON contract
at the top of this plan as the specification, checking specifically that every field name,
nullability and error-field path agrees on both sides.

- [ ] **Step 3: Clear the database debris and walk through it**

```bash
rm -f data/quizapp.db     # Part 1 verification debris: REVIEW_MOD_1, "kinetics 100%"
cargo run                 # recreates and migrates
```

Then the manual walkthrough in the Verification section below.

- [ ] **Step 4: Update the documentation**

`README.md`: the new frontend routes and the cards endpoints.

`docs/HANDOVER.md`: update "Where things stand" and "Next up" (Part 2b: image upload and
KaTeX), drop the resolved `AppError::Db` FK item from "Known-and-accepted minors", and
rewrite "Needs a human at a browser" for the new screens.

The spec: add `GET /api/decks/:id` and `POST /api/cards/:id/unarchive` to the API table, and
confirm Task 1's normalisation amendment landed.

- [ ] **Step 5: Commit**

```bash
git add README.md docs
git commit -m "docs: record Part 2a and refresh the handover"
```

```json:metadata
{"files":["README.md","docs/HANDOVER.md","docs/mitis/specs/2026-08-26-quiz-study-app-design.md"],"verifyCommand":"cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build && cd frontend && pnpm exec tsc --noEmit && pnpm build","acceptanceCriteria":["the full four-command gate passes","one review runs across the API-to-client seam using the JSON contract as the spec","the manual walkthrough is completed and recorded","README documents the new routes and endpoints","HANDOVER reflects the new state and drops the resolved FK item","the spec's API table gains GET /api/decks/:id and POST /api/cards/:id/unarchive"],"modelTier":"standard"}
```

---

## Verification

### The gate

From the repo root, with `export PATH="$HOME/.cargo/bin:$PATH"`:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

### Manual walkthrough

`cargo run`, then `cd frontend && pnpm dev` — port **5273**, not 5173.

1. Open a deck from `/decks`. The card list is empty and points at "New card".
2. Write an `mc_single` card with four choices **entirely by keyboard**: type the prompt, tab
   to the choices, `Enter` to add each row, mark one correct, `Cmd+Enter`. The form comes back
   ready for the next card with the kind retained and the prompt focused.
3. Write a `short_answer` card with two accepted answers, and a `flashcard`, the same way.
4. Save an `mc_single` with two options marked correct. Expect an inline error on the choices
   list and **every typed value still on screen**.
5. Leave one choice blank and save. The error appears beside *that* row.
6. Edit a card, switch its kind to flashcard, save, reload the page. No stale choices.
7. Archive a card: it leaves the list and the deck's card count drops. Flip the
   show-archived toggle — it reappears, muted. Unarchive restores it.
8. `Escape` from the editor returns to the deck without saving; `Cmd+S` saves and returns.
9. Check the schedule invariant:

```bash
sqlite3 data/quizapp.db \
  "SELECT (SELECT COUNT(*) FROM cards) AS cards, (SELECT COUNT(*) FROM schedule) AS sched"
```

The two numbers must match.

### Needs a human at a browser

No agent could verify these in Part 1 and the same holds here. Check and record:

- The editor at 375px: the choices rows, the radio column, the action bar
- Both themes, and the kind badges legible in each
- That the keyboard loop genuinely feels like a loop — prompt, choices, save, next, without
  a pause to find where focus went
- Whether the deck detail page reads well at 100+ cards, which is what COS781 will actually
  be. If not, the kind filter and prompt search deferred out of Task 4 are the fix.
