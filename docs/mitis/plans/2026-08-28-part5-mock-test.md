# Part 5: the mock test — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use mitis:subagent-driven-development (recommended) or mitis:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build mock test mode — the whole deck, every card once, in a reload-stable order, with no feedback during the run and a full per-question results screen afterwards. Enable the "Mock test" tile that `e09e76d` already built on `/decks/:id`.

**Architecture:** Two new pure modules and one new endpoint. `backend/src/mock.rs` turns `(card ids, seed)` into a deterministic order by rank-by-hash; `grading.rs` gains a bounded edit-distance for typed flashcards. `sessions.rs` branches on session mode in five places, and `GET /api/sessions/:id/results` serves the post-terminal record. The frontend gets its own route and page rather than branching the practice runner. No migration, no new column, no new table, no new dependency.

**Tech Stack:** Rust + axum + sqlx/SQLite; React 19 + TypeScript + Tailwind + shadcn.

**Design doc:** [`../specs/2026-08-28-part5-mock-test-design.md`](../specs/2026-08-28-part5-mock-test-design.md). **Read it before Task 1.** It carries the reasoning for every ruling below, and §5 and §8b in particular describe three answer leaks that are easy to reintroduce.

**User decisions (already made):**
- A mock test is **one deck** — no module picker in the UI. The backend keeps its multi-deck and `module_id` capability.
- A mock test is **the whole deck**, every non-archived card exactly once. No length input. The server computes `target_count`.
- A mock flashcard is **typed and auto-graded** against `answer_md` — normalised, then spelling-tolerant. Practice flashcards keep self-grading.
- **Elapsed count-up clock.** No countdown, no auto-submit.
- The results screen shows **every question in order** with your answer, the correct answer, the explanation and right/wrong.

**Two spec amendments this plan implements** (both in the design doc, §1 and §7):
- Mock mode is the **whole non-archived pool** served exactly once in a per-session deterministic order — not "`target_count` cards sampled uniformly at random". `target_count` records the pool at creation.
- Results come from a new `GET /api/sessions/:id/results`, gated on `ended_at IS NOT NULL`, not from `/finish`. `/finish` is unchanged.

---

## File Structure

| Path | Responsibility |
| --- | --- |
| `backend/src/mock.rs` | **new** — `mock_order(card_ids, seed)`, pure: no database, no clock, no `rand` |
| `backend/src/lib.rs` | `pub mod mock;` |
| `backend/src/grading.rs` | `levenshtein`, `fuzzy_tolerance`, `grade_flashcard_typed`; `grade_short_answer` untouched |
| `backend/src/routes/sessions.rs` | mode-aware create / next / reveal / answer / override, plus `/results` |
| `backend/tests/mock.rs` | **new** — all mock coverage; `tests/sessions.rs` is already 1700 lines |
| `backend/tests/sessions.rs` | two amendments only (Tasks 3 and 4) |
| `backend/tests/common/mod.rs` | **unchanged** — the shared harness already has what is needed |
| `frontend/src/lib/api.ts` | new types and two client functions |
| `frontend/src/lib/format.ts` | **new** — `formatDuration` moved here, plus `formatClock` |
| `frontend/src/pages/MockSessionPage.tsx` | **new** — the run and its state machine |
| `frontend/src/components/session/MockTimer.tsx` | **new** — owns its own tick |
| `frontend/src/components/session/MockRunHeader.tsx` | **new** — progress, clock, end-early |
| `frontend/src/components/session/ResultRow.tsx` | **new** — one question, kind-agnostic |
| `frontend/src/components/session/MockResults.tsx` | **new** — the results screen |
| `frontend/src/components/session/SummaryTiles.tsx` | **new** — extracted, shared by both summaries |
| `frontend/src/pages/SessionPage.tsx` | **one change**: redirect a mock session to `/mock/:id` |
| `frontend/src/App.tsx` | the `/mock/:id` route |
| `frontend/src/pages/DeckPage.tsx` | enable the tile; route by mode |

No migration: `sessions.mode` already permits `'mock'` and `sessions.target_count` already exists, both from `0001_init.sql:57-64`. No new colour token: `--success` and `--destructive` already exist in both palettes and are already in `check-contrast.py`'s enforced tier.

---

## Task 1: `backend/src/mock.rs` — the deterministic serve order

**Goal:** A pure, dependency-free module turning `(card ids, seed)` into a stable order that is uniformly random per seed, testable without an RNG or a database.

**Files:** Create `backend/src/mock.rs`; modify `backend/src/lib.rs`

Rank-by-hash, **not** Fisher–Yates — design doc §2a. A shuffle is a function of the list, so archiving one card mid-test would reshuffle every remaining card and a reload would serve a different question, which is the defect this exists to prevent.

**Code:**

```rust
fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn sort_key(seed: u64, card_id: i64) -> u64 {
    mix64(seed ^ mix64(card_id as u64))
}

pub fn mock_order(card_ids: &[i64], seed: u64) -> Vec<i64> {
    let mut ordered = card_ids.to_vec();
    ordered.sort_unstable_by_key(|card_id| (sort_key(seed, *card_id), *card_id));
    ordered
}
```

The `(key, card_id)` tuple makes the order total even on a hash collision, so determinism does not rest on sort stability. `mix64` is written here rather than taken from `rand` because `StdRng`'s output is documented as unstable across minor releases, and a `cargo update` reordering an in-progress mock test is precisely the failure being designed out.

**Acceptance Criteria:**
- [ ] `mock_order` is pure — no database, no `rand`, no clock, no `unsafe`
- [ ] The same `(ids, seed)` yields the same order over 100 repeats
- [ ] The output is a permutation of the input: same length, same set
- [ ] The result does not depend on the input's arrival order — reversed and rotated inputs give identical results
- [ ] Removing one id preserves the relative order of the remaining ids
- [ ] Consecutive seeds `1..=10` over a ten-card pool do not all produce the same order
- [ ] Over 6000 seeds on a three-card pool all six permutations appear, each within ±25% of 1000
- [ ] Empty input yields an empty order; a single id yields itself
- [ ] `cargo clippy --all-targets -- -D warnings` is clean

**Mutation evidence required**, one change at a time:

| Test | Single change that must make it red |
| --- | --- |
| uniformity over 6000 seeds | `sort_key` returns `card_id as u64` |
| input-order independence | swap to Fisher–Yates over the vector |
| stability under removal | swap to Fisher–Yates over the vector |
| different seeds differ | `sort_key` ignores `seed` |
| consecutive-seed decorrelation | drop the inner `mix64(card_id)` |

**Verify:** `cargo test --lib mock && cargo clippy --all-targets -- -D warnings`

```json:metadata
{"files": ["backend/src/mock.rs", "backend/src/lib.rs"], "verifyCommand": "cargo test --lib mock && cargo clippy --all-targets -- -D warnings", "acceptanceCriteria": ["mock_order is pure with no database, rand, clock or unsafe", "The same ids and seed always yield the same order over 100 repeats", "The output is a permutation of the input with the same length and set", "The result does not depend on the input's arrival order", "Removing one id preserves the relative order of the rest", "Consecutive seeds do not all produce the same order", "All six permutations of a three-card pool appear within 25 percent of uniform over 6000 seeds", "Empty and single-element inputs are handled", "clippy --all-targets -D warnings is clean"], "modelTier": "standard"}
```

---

## Task 2: `grading.rs` — Levenshtein and typed-flashcard grading

**Goal:** A spelling-tolerant verdict for a typed flashcard, pure, with a stated tolerance rule and a length guard — leaving `grade_short_answer` untouched.

**Files:** Modify `backend/src/grading.rs`

**The tolerance rule.** `n` is the char count of the **normalised expected** answer, so the card sets the tolerance rather than the student's typing:

| n | edits forgiven | worked example |
| --- | --- | --- |
| 0–7 | 0 | `ridge`/`bridge` is distance 1 — different words, must not match |
| 8–15 | 1 | `clustering` absorbs `clusterng`; `maximise`/`minimise` is distance **2**, correctly rejected |
| 16–23 | 2 | `information gain` absorbs `informaton gain` |
| 24+ | 2 (capped) | a 40-char answer gets 5% tolerance — near-exact, which is the intent |

`FUZZY_DIVISOR = 8` because the errors are asymmetric: a false reject is one click from fixed via "I was right", a false accept is permanent. Divisor 6 was tried and rejected on a concrete case — `bridge` is six characters, so it landed in the one-edit bucket and graded `ridge` correct. The measured cost of 8 is that `entropy` (7 chars) gets no tolerance, so `entrpy` grades wrong; that is a false reject and therefore recoverable. `FUZZY_MAX_LENGTH = 120` is a cost guard — Levenshtein is O(n·m) and a multi-kilobyte prose answer would be tens of millions of cells per answer, while a tolerance capped at 2 makes the fuzzy branch meaningless up there anyway.

**Code:**

```rust
pub const FUZZY_DIVISOR: usize = 6;
pub const FUZZY_MAX_TOLERANCE: usize = 2;
pub const FUZZY_MAX_LENGTH: usize = 120;

pub fn fuzzy_tolerance(expected_length: usize) -> usize {
    (expected_length / FUZZY_DIVISOR).min(FUZZY_MAX_TOLERANCE)
}

pub fn grade_flashcard_typed(given: &str, answer_md: &str) -> bool {
    let submitted = normalise(given);
    let expected = normalise(answer_md);
    if submitted.is_empty() || expected.is_empty() {
        return false;
    }
    if submitted == expected {
        return true;
    }
    let expected_length = expected.chars().count();
    let submitted_length = submitted.chars().count();
    if expected_length > FUZZY_MAX_LENGTH || submitted_length > FUZZY_MAX_LENGTH {
        return false;
    }
    let tolerance = fuzzy_tolerance(expected_length);
    if expected_length.abs_diff(submitted_length) > tolerance {
        return false;
    }
    levenshtein(&submitted, &expected) <= tolerance
}
```

`levenshtein_distance` is a two-row dynamic program counting over **chars, not bytes** — prompts carry accented text and NFKC output, where a byte-wise distance overcounts. There is no `strsim` in the tree and none is added.

Three pieces need labelling correctly, because the obvious reading of each is wrong:
- **The exact-match early return is not an "exact first" optimisation.** Below the length guard the fuzzy branch already handles an exact match (distance 0 clears any tolerance). Its only load-bearing job is *above* the guard, where the fuzzy branch returns false unconditionally — without it a long prose answer retyped perfectly would grade wrong.
- **The `abs_diff` prefilter is a pure optimisation.** Removing it must change no verdict, and a test asserts that by staying green.
- **Only the whole empty-key guard is provable.** With one side empty the prefilter already rejects, so each half alone is redundant; the case that needs it is *both* sides normalising to empty. Mutate it as one unit.

**Recorded accepted cost:** `type i error` and `type ii error` are distance 1 within a tolerance of 2, so one grades as the other. Fuzzy matching can only produce false *accepts*, and there is no reverse override, so this is unfixable from the UI. A test pins the behaviour so the choice is visible rather than accidental — see the design doc §4.

**Acceptance Criteria:**
- [ ] `levenshtein` is symmetric, zero on equality, and equals the other length against an empty string; `kitten`/`sitting` is 3
- [ ] `levenshtein` counts chars not bytes — `café`/`cafe` is 1
- [ ] `fuzzy_tolerance` matches the table at the boundaries 5/6, 11/12, 17/18 and 1000
- [ ] An exact answer grades correct at **any** length, including above `FUZZY_MAX_LENGTH`
- [ ] A single typo in an 8+ char answer grades correct; `maximise` against `minimise` grades **wrong**, and so does `ridge` against `bridge`
- [ ] Casing and punctuation never matter: `  K-MEANS!  ` matches `k-means`
- [ ] Blank, whitespace-only and punctuation-only submissions grade wrong; an `answer_md` of `---` never matches anything
- [ ] Above `FUZZY_MAX_LENGTH` one typo grades wrong while the exact text still grades correct
- [ ] `grade_short_answer` is byte-for-byte unchanged and all its existing unit tests pass
- [ ] `type i error` vs `type ii error` has a test recording the accepted behaviour

**Mutation evidence required:**

| Test | Single change |
| --- | --- |
| char-not-byte distance | `levenshtein_distance` iterates `.bytes()` |
| divisor, short-word rejection | `FUZZY_DIVISOR` → 4 |
| tolerance cap | `FUZZY_MAX_TOLERANCE` → 5 |
| long-answer guard | delete the `FUZZY_MAX_LENGTH` branch |
| empty-key guard | delete **both halves** at once — either half alone is redundant |
| exact match above the guard | delete the exact-match early return entirely |
| prefilter is not load-bearing | delete the `abs_diff` prefilter — every verdict test must **stay green** |

**Verify:** `cargo test --lib grading && cargo clippy --all-targets -- -D warnings`

```json:metadata
{"files": ["backend/src/grading.rs"], "verifyCommand": "cargo test --lib grading && cargo clippy --all-targets -- -D warnings", "acceptanceCriteria": ["levenshtein is symmetric, zero on equality, the other length against empty, and 3 for kitten/sitting", "levenshtein counts chars not bytes", "fuzzy_tolerance matches the stated table at every boundary", "An exact answer grades correct at any length including above FUZZY_MAX_LENGTH", "A single typo in an 8-plus character answer is correct while maximise against minimise and ridge against bridge are wrong", "Casing and punctuation never affect the verdict", "Empty, whitespace-only and punctuation-only inputs and expectations never match", "Above FUZZY_MAX_LENGTH only exact matches count", "grade_short_answer is unchanged and its existing tests pass", "The type i error versus type ii error behaviour is recorded in a test", "Deleting the abs_diff prefilter changes no verdict"], "modelTier": "standard"}
```

---

## Task 3: `POST /api/sessions` accepts mock and stores `target_count`

**Goal:** A mock session can be created; the server computes its length from the pool; a client-supplied `target_count` is refused rather than ignored.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/sessions.rs`; create `backend/tests/mock.rs`

**Validation, in this order** — all 422 with the envelope's `fields`:

| Condition | Field | Message |
| --- | --- | --- |
| `mode` not in `MODES` | `mode` | mode must be practice, mock or sm2 |
| `mode` is `sm2` | `mode` | Only practice and mock modes are available yet |
| both `deck_ids` and `module_id` | `deck_ids` | Choose either decks or a module, not both |
| neither | `deck_ids` | Choose at least one deck or a module |
| `deck_ids` present and empty | `deck_ids` | Choose at least one deck |
| `target_count` present, mode `practice` | `target_count` | Practice sessions have no target count |
| `target_count` present, mode `mock` | `target_count` | A mock test is the whole deck, so its length is not yours to set |
| a deck id not found | `deck_ids` | That deck does not exist |
| `module_id` not found | `module_id` | That module does not exist |
| module has no decks | `module_id` | That module has no decks |
| resolved pool has zero non-archived cards | whichever was supplied | Those decks have no cards to practise |

The practice `target_count` wording and the empty-pool wording are **deliberately unchanged**, so `rejects_a_target_count_on_a_practice_session`, `refuses_to_create_a_session_with_no_eligible_cards` and `archived_cards_do_not_count_as_eligible` stay green untouched.

On success the insert binds `target_count = Some(pool_count)` for mock and `None` for practice. The empty-pool refusal still fires **before** the insert, so an empty deck never produces a mock with `target_count = 0`.

**Existing test to amend:** `rejects_mock_and_sm2_modes_for_now` (`backend/tests/sessions.rs:138-156`) loops over `["mock","sm2"]`. Rename to `rejects_sm2_mode_for_now`, drop `mock`, and assert the new wording. **Do not relax it to a field-name check** — the handover records that exact mistake, "an error assertion that checked only the field name while two different messages used that field".

**Integration tests** (new, in `backend/tests/mock.rs`; add local helpers `start_mock_session` and `submit`, and do not modify `tests/common/mod.rs`):
- [ ] `creates_a_mock_session_with_the_pool_size_as_its_target` — 201, `mode: "mock"`, `target_count == 3` for a three-card deck
- [ ] `a_mock_target_count_excludes_archived_cards` — archive one of three, `target_count == 2`
- [ ] `rejects_a_client_supplied_target_count_on_a_mock_session` — 422, exact message, and `COUNT(sessions) == 0`
- [ ] `refuses_to_create_a_mock_session_for_an_empty_deck` — 422 and no row
- [ ] `a_mock_session_accepts_a_module_wide_pool` — the API stays uniform even though the UI sends one deck

**Mutation evidence:**

| Test | Single change |
| --- | --- |
| `…pool_size_as_its_target` | bind `NULL` instead of `pool_count` |
| `…excludes_archived_cards` | drop `archived = 0` from the count |
| `rejects_a_client_supplied_target_count…` | let the mock branch ignore `target_count` |
| `rejects_sm2_mode_for_now` | let `sm2` through |

**Verify:** `cargo test --test sessions --test mock && SQLX_OFFLINE=true cargo build`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/sessions.rs", "backend/tests/mock.rs"], "verifyCommand": "cargo test --test sessions --test mock && SQLX_OFFLINE=true cargo build", "acceptanceCriteria": ["A mock session is created with 201 and target_count equal to the non-archived pool size", "Archived cards are excluded from the computed target_count", "A client-supplied target_count is rejected with the mock-specific message and writes no session row", "sm2 is still rejected with the reworded message and its test is amended rather than relaxed", "An empty pool refuses mock creation before any insert", "A module-wide mock pool is still accepted by the API", "The practice target_count and empty-pool messages are unchanged and their tests pass"], "modelTier": "mechanical"}
```

---

## Task 4: `GET /api/sessions/:id/next` — a stable mock serve with no score

**Goal:** Mock mode serves each card exactly once in a reload-stable order, tells the client which mode it is in, and carries no running score.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/mock.rs`, `backend/tests/sessions.rs`

`load_active_session` grows `mode`:

```sql
SELECT id AS "id!: i64", mode, deck_ids, ended_at FROM sessions WHERE id = ?
```

Mock candidates — the unanswered live pool:

```sql
SELECT cards.id AS "card_id!: i64"
FROM cards
WHERE cards.archived = 0
  AND cards.deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
  AND NOT EXISTS (
        SELECT 1 FROM reviews
        WHERE reviews.card_id = cards.id AND reviews.session_id = ?
      )
ORDER BY cards.id
```

Then `mock_order(&ids, session.id as u64).first().copied()`; `None` → 409 `Every card in this mock test has been answered`.

**Response variants** — `#[serde(untagged)]`, each carrying an explicit `mode`, because untagged is a deserialisation feature and puts no discriminator on the wire:

```rust
pub struct PracticeNext { mode, card, pool_count, answered_count, correct_count }
pub struct MockNext     { mode, card, pool_count, answered_count }
```

`correct_count` is **absent from `MockNext` structurally, not blanked** — comparing it across two serves is live per-question feedback. `pool_count` is the **full** pool (reuse the existing cached count query via a shared `count_pool` helper), never the unanswered count, or the runner would read "3 of 7" then "4 of 6".

**Choice order in mock is deterministic too**, via `mock_order` over the choice ids seeded from `session.id` and `card_id`. Practice keeps `choices.shuffle(&mut rand::thread_rng())` unchanged.

**Existing test to amend:** `next_never_returns_answer_data_for_any_kind` (`tests/sessions.rs:394`) pins the practice envelope key set at `:424-431`; it becomes `["answered_count","card","correct_count","mode","pool_count"]`. Leave the card-level key assertion (`:411-415`) and the forbidden-substring sweep (`:433-451`) alone.

**Integration tests:**
- [ ] `a_mock_serve_carries_the_mode_and_no_running_score` — envelope keys are exactly `["answered_count","card","mode","pool_count"]`
- [ ] **`a_mock_serve_is_identical_across_reloads`** — 20 `/next` calls without answering; the **whole response body** is equal every time, choice order included
- [ ] `a_mock_test_serves_every_card_exactly_once` — 12-card deck; all 12 ids, no repeats, then 409
- [ ] `a_mock_serve_never_returns_answer_content_for_any_kind` — the leakage sweep, on the mock path
- [ ] `a_mock_pool_count_does_not_shrink_as_cards_are_answered`
- [ ] `archiving_a_card_mid_mock_ends_the_run_early` — 409 after `target_count - 1` answers, not a 500 or a loop
- [ ] `archiving_a_card_mid_mock_does_not_reorder_the_remaining_cards` — the remaining serve order is the same subsequence
- [ ] `two_mock_sessions_on_the_same_deck_get_different_orders`
- [ ] `a_finished_mock_session_conflicts_on_next`

**Mutation evidence:**

| Test | Single change |
| --- | --- |
| `…no_running_score` | add `correct_count` to `MockNext` |
| `…identical_across_reloads` | seed the order from `rand::random()` |
| `…identical_across_reloads` (choices) | use `thread_rng()` for mock choice order |
| `…every_card_exactly_once` | drop the `NOT EXISTS` clause |
| `…pool_count_does_not_shrink` | report the candidate-list length |
| `…does_not_reorder_the_remaining_cards` | swap `mock_order` for Fisher–Yates |
| `…different_orders` | seed from a constant |
| amended practice key test | omit `mode` from `PracticeNext` |

**Verify:** `cargo test --test sessions --test mock && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/mock.rs", "backend/tests/sessions.rs"], "verifyCommand": "cargo test --test sessions --test mock && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build", "acceptanceCriteria": ["The mock serve carries mode and omits correct_count structurally rather than as a null", "Repeated /next calls without answering return a byte-identical body including choice order", "A mock test serves every non-archived pool card exactly once and then 409s", "The mock serve carries no answer content for any of the three kinds", "pool_count is the full pool and stays constant while answered_count climbs", "Archiving a card mid-run ends the run early with a 409 and does not reorder the remaining cards", "Two sessions on the same deck get different orders", "The practice envelope key test is amended to include mode and still pins the exact key set", "Practice choice shuffling still uses rand and is unchanged"], "modelTier": "frontier"}
```

---

## Task 5: `POST /api/sessions/:id/reveal` refuses mock sessions

**Goal:** Close the reveal oracle. A mock flashcard is server-graded, so Part 3 §4's "nothing could do better, since the student is the grader" no longer applies.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/mock.rs`

| Condition | Status | Message |
| --- | --- | --- |
| session mode is `mock` | 409 | A mock test does not reveal answers |
| kind is not `flashcard` | 409 | Only a flashcard can be revealed |

Check the **mode first, then the kind**, so a mock `mc_single` and a mock `flashcard` produce an identical refusal. Kind-first would let the error message be used to probe what kind a card is.

**Integration tests:**
- [ ] `a_mock_session_refuses_to_reveal_a_flashcard` — 409, exact message, and the body contains none of `answer_md`, the answer text, or `explanation_md`
- [ ] `a_mock_reveal_refusal_does_not_disclose_the_card_kind` — all three kinds get an identical status and message
- [ ] `a_mock_reveal_writes_nothing` — `COUNT(reviews) == 0`
- [ ] Practice `revealing_a_flashcard_returns_its_answer` (`tests/sessions.rs:684`) still passes untouched

**Mutation evidence:**

| Test | Single change |
| --- | --- |
| `…refuses_to_reveal_a_flashcard` | delete the mode check |
| `…does_not_disclose_the_card_kind` | order the checks kind-first |

**Verify:** `cargo test --test sessions --test mock`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/mock.rs"], "verifyCommand": "cargo test --test sessions --test mock", "acceptanceCriteria": ["A mock session refuses /reveal with 409 for every kind, with mode checked before kind", "The refusal body carries no answer_md, answer text or explanation_md", "The refusal is identical across the three kinds so it cannot probe the card kind", "A refused reveal writes no rows", "Practice reveal behaviour is unchanged and its existing tests pass"], "modelTier": "mechanical"}
```

---

## Task 6: `POST /api/sessions/:id/answer` — mode-aware fields, auto-graded flashcards, no feedback

**Goal:** In mock mode a flashcard is typed and auto-graded, the response carries nothing about correctness, and a card cannot be answered twice.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/mock.rs`

**Allowed field by (mode, kind):**

| kind | practice | mock |
| --- | --- | --- |
| `mc_single` | `choice_id` | `choice_id` |
| `short_answer` | `given` | `given` |
| `flashcard` | `self_grade` | `given` |

`reject_fields_for_other_kinds` already takes the allowed field name, so it grows a `mode` parameter and picks its message from it:

| rejected field | mode | Message |
| --- | --- | --- |
| `given` | practice | Only a short-answer card takes typed text |
| `given` | mock | Only a short-answer or flashcard takes typed text |
| `choice_id` | either | Only a multiple-choice card has options |
| `self_grade` | practice | Only a flashcard is self-graded |
| `self_grade` | mock | A mock test grades flashcards automatically |

**Required-field and state errors:**

| Condition | Status | Field | Message |
| --- | --- | --- | --- |
| mock flashcard, `given` absent | 422 | `given` | This field is required |
| mock flashcard, `given` whitespace-only | 422 | `given` | Type an answer |
| card not in pool / archived | 422 | `card_id` | That card is not in this session |
| negative `ms` | 422 | `ms` | ms must not be negative |
| mock, card already answered in this session | **409** | — | That card has already been answered in this mock test |

The duplicate guard runs **before** grading, so a repeat neither grades nor writes:

```sql
SELECT COUNT(*) AS "already_answered!: i64"
FROM reviews WHERE session_id = ? AND card_id = ?
```

409 rather than 422 because it is a session-state conflict, not a bad field. This does not reopen Part 3 §3's trust boundary, which was about serve *order* — order stays unenforced.

The mock flashcard branch reuses the short-answer required/blank path verbatim, then `grade_flashcard_typed(trimmed, answer_md)`. `answer_md` is guaranteed present and non-blank by card validation, so no `Internal` fallback is needed. **Store `given` = the raw trimmed text** (so the results screen shows what you wrote and the override has a wording), `self_grade` = `NULL`, `correct` = the verdict.

**Response** — `#[serde(untagged)]`, each variant carrying `mode`:

```rust
pub struct PracticeAnswer { mode, review_id, correct, expected, explanation_md, can_override }
pub struct MockAnswer     { mode, answered_count, pool_count }
```

`MockAnswer` has no field capable of holding answer content. `review_id` is omitted as honest minimalism — it is **not** a security control, since review ids are guessable; Task 8's state gate is. `can_override` moves to `/results`.

**Existing tests that must stay green untouched:** `rejects_the_wrong_answer_field_for_each_kind` (`tests/sessions.rs:1082`) checks field names on a practice session, so it passes only if the practice messages are unchanged. `a_flashcard_self_grade_is_persisted` (`:1035`) is the Part 7 guard — `reviews.self_grade` must still be written in practice.

**Integration tests:**
- [ ] `a_mock_flashcard_is_typed_and_auto_graded` — matching text → row `correct = 1`; nonsense → `correct = 0`, read from the database since the response says nothing
- [ ] `a_mock_flashcard_absorbs_a_small_typo` — one-char error on a 7+ char answer → `correct = 1`
- [ ] `a_mock_flashcard_rejects_a_self_grade` — 422, exact message, `COUNT(reviews) == 0`
- [ ] `a_mock_flashcard_requires_typed_text` — absent → `This field is required`; `"   "` → `Type an answer`
- [ ] `a_mock_flashcard_stores_the_wording_and_no_self_grade` — `given` is the raw trimmed text, `self_grade IS NULL`
- [ ] `a_mock_multiple_choice_still_rejects_typed_text` — 422 with the mock wording
- [ ] **`a_mock_answer_response_carries_no_verdict`** — keys are exactly `["answered_count","mode","pool_count"]`, and the body contains none of `correct`, `expected`, `explanation_md`, `can_override`, `review_id`, nor any answer text of any kind
- [ ] `a_mock_card_cannot_be_answered_twice` — second POST is 409 and `COUNT(reviews) == 1`
- [ ] `a_practice_flashcard_is_still_self_graded` — the four grades still work and `self_grade` is still stored
- [ ] `answering_a_mock_card_does_not_touch_the_schedule_table` — `schedule_for` identical before and after

**Mutation evidence:**

| Test | Single change |
| --- | --- |
| `…carries_no_verdict` | return the practice variant from the mock branch |
| `…rejects_a_self_grade` | make `reject_fields_for_other_kinds` mode-blind |
| `…typed_and_auto_graded` | grade a mock flashcard as always-correct |
| `…absorbs_a_small_typo` | call `grade_short_answer` instead |
| `…stores_the_wording…` | store the normalised form instead of the raw text |
| `…cannot_be_answered_twice` | delete the duplicate guard |
| `a_practice_flashcard_is_still_self_graded` | route practice flashcards through the typed path |

**Verify:** `cargo test --test sessions --test mock && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/mock.rs"], "verifyCommand": "cargo test --test sessions --test mock && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build", "acceptanceCriteria": ["A mock flashcard takes given, is auto-graded against answer_md, and rejects self_grade with the mock message", "A mock flashcard absorbs a small typo through grade_flashcard_typed rather than grade_short_answer", "A mock flashcard stores the raw trimmed wording and leaves self_grade NULL", "The mock answer response keys are exactly mode, answered_count and pool_count and carry no verdict or answer content", "A card already answered in a mock session is refused with 409, writes no second row, and the guard runs before grading", "Practice flashcards keep self-grading and keep populating reviews.self_grade", "Practice per-kind field rejection messages are unchanged and their test passes", "Answering in mock mode does not touch the schedule table"], "modelTier": "frontier"}
```

---

## Task 7: `GET /api/sessions/:id/results` — the per-question record

**Goal:** Every question in answer order with the student's answer, the correct answer, the explanation and the verdict — readable only after the session has been submitted, and reloadable.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/mock.rs`

| Condition | Status | Message |
| --- | --- | --- |
| session unknown | 404 | session not found |
| `ended_at IS NULL` | 409 | This session has not been submitted yet |

Gated on `ended_at` **only, not on mode** — one rule. Practice sessions get a per-question record too, which costs nothing and gives Part 6 a head start. The 404 check runs first.

**Response** — nested, **never `#[serde(flatten)]`** (Part 3 §4 layer 1):

```rust
pub struct ResultsResponse { summary: SummaryResponse, questions: Vec<ResultQuestion> }
pub struct ResultQuestion {
    review_id: i64, card_id: i64, kind: String, prompt_md: String,
    image_path: Option<String>, given: Option<String>, self_grade: Option<String>,
    expected: Vec<String>, explanation_md: Option<String>,
    correct: bool, overridden: bool, can_override: bool,
    ms: Option<i64>, answered_at: String,
}
```

`review_id` is what makes the override reachable from the results screen — a row otherwise carries only `card_id` and the override takes a review id. `can_override` is computed server-side so the client never reimplements the eligibility rule. **No `choices` and no choice ids**, deliberately: all three kinds render through one component.

Three queries. The reviews, in answer order — the `reviews.id` tiebreak is mandatory because timestamps have one-second resolution, and it **mirrors the sort direction**, both ascending:

```sql
SELECT reviews.id AS "review_id!: i64", reviews.card_id AS "card_id!: i64",
       reviews.answered_at, reviews.given, reviews.self_grade,
       reviews.correct AS "correct!: bool", reviews.overridden AS "overridden!: bool",
       reviews.ms, cards.kind, cards.prompt_md, cards.image_path,
       cards.answer_md, cards.explanation_md
FROM reviews
JOIN cards ON cards.id = reviews.card_id
WHERE reviews.session_id = ?
ORDER BY reviews.answered_at, reviews.id
```

Then the correct choice texts and the accepted wordings, each one bulk query rather than N+1:

```sql
SELECT choices.card_id AS "card_id!: i64", choices.text_md
FROM choices
WHERE choices.is_correct = 1
  AND choices.card_id IN (SELECT card_id FROM reviews WHERE session_id = ?)
ORDER BY choices.card_id, choices.position
```

```sql
SELECT accepted.card_id AS "card_id!: i64", accepted.text
FROM accepted
WHERE accepted.card_id IN (SELECT card_id FROM reviews WHERE session_id = ?)
ORDER BY accepted.card_id, accepted.is_primary DESC, accepted.id
```

Fold them in a **pure function** — `assemble_results(reviews, correct_choices, accepted) -> Vec<ResultQuestion>` — mirroring `fold_candidate_rows`, so ordering, `expected` selection and `can_override` are unit-testable without a database.

`expected` is one shape for all three kinds, matching Part 3 §11: the correct choice texts / the accepted wordings primary-first / `[answer_md]`.

Extract `summarise()` out of `finish` so both share one cached query. **`/finish` itself is unchanged**, so its nine existing tests are untouched.

**Integration tests:**
- [ ] **`results_are_refused_while_the_session_is_active`** — 409, and after answering all three kinds the body contains none of `expected`, `answer_md`, `explanation_md`, the accepted wordings, the correct choice text, or the flashcard answer. This is the leak boundary.
- [ ] `results_list_every_question_in_answer_order` — 12 cards, ids in the served order
- [ ] `results_carry_the_prompt_the_given_the_expected_and_the_explanation` — one case per kind
- [ ] `results_report_correctness_per_question` — a mix of right and wrong
- [ ] `results_survive_a_reload` — two GETs return equal bodies
- [ ] `results_on_an_active_practice_session_are_also_refused` — one rule, not a mock special case
- [ ] `results_include_a_practice_flashcards_self_grade` — `self_grade: "hard"`, `given: null`
- [ ] `results_mark_an_overridden_question_correct_and_overridden`
- [ ] `results_can_override_is_true_only_where_the_override_would_be_accepted` — a wrong mock short-answer and a wrong mock flashcard are `true`; a wrong `mc_single`, and everything correct, are `false`
- [ ] `results_on_an_unknown_session_are_not_found` — 404
- [ ] `results_of_an_empty_session_are_an_empty_list_with_a_null_accuracy`
- [ ] Unit: `assemble_results` orders by `answered_at` then `id` and picks `expected` per kind

**Mutation evidence:**

| Test | Single change |
| --- | --- |
| `…refused_while_the_session_is_active` | delete the `ended_at IS NULL` check |
| `…every_question_in_answer_order` | reverse the ORDER BY direction — proven by a deliberately constructed timestamp tie |
| ~~drop the `reviews.id` tiebreak~~ | **Not provable, and kept anyway.** With three reviews sharing an `answered_at`, SQLite returns them in rowid order regardless, so removing the tiebreak changes nothing observable. It stays because the order is only *incidentally* right — nothing in SQLite guarantees rowid order for an unqualified `ORDER BY`. This is the same situation the handover already records for the decks list query, where one half of a tiebreak is provable and the other is kept on principle. Do not delete it on the grounds that no test covers it. |
| `…the_expected_and_the_explanation` | make `expected` always come from `accepted` |
| accepted primary-first | drop `is_primary DESC` |
| `…can_override_is_true_only_where…` | make `can_override` a bare `!correct` |
| `…include_a_practice_flashcards_self_grade` | omit `self_grade` from the projection |
| `…survive_a_reload` | make `/results` a POST that mutates `ended_at` |

**Verify:** `cargo test --test sessions --test mock --lib && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/mock.rs"], "verifyCommand": "cargo test --test sessions --test mock --lib && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build", "acceptanceCriteria": ["GET /results 409s while ended_at IS NULL for practice and mock alike, leaking no answer content", "An unknown session is 404 and that check runs before the active check", "Questions come back one per review in answered_at then id order, with the tiebreak proved by a deliberately constructed tie", "Each question carries review_id, prompt_md, image_path, given or self_grade, expected, explanation_md, correct, overridden, can_override, ms and answered_at", "expected is the correct choice texts, the accepted wordings primary-first, or answer_md, per kind", "can_override is true only where the override endpoint would accept the review", "Two GETs return equal bodies so the results page survives a reload", "The summary is nested rather than flattened and reuses the query finish already caches", "/finish is unchanged and all its existing tests pass", "assemble_results is a pure function with its own unit tests"], "modelTier": "standard"}
```

---

## Task 8: `POST /api/reviews/:id/override` — flashcards, and the mock gate

**Goal:** The safety valve for a prose answer the auto-grader marked wrong — and closing the oracle that safety valve would otherwise open mid-run.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/mock.rs`

The lookup grows `mode`, `ended_at` and `answer_md`:

```sql
SELECT reviews.id AS "id!: i64", reviews.card_id AS "card_id!: i64",
       reviews.given, reviews.correct AS "correct!: bool",
       cards.kind, cards.answer_md, sessions.mode, sessions.ended_at
FROM reviews
JOIN cards ON cards.id = reviews.card_id
JOIN sessions ON sessions.id = reviews.session_id
WHERE reviews.id = ?
```

**Eligibility, checked in this order:**

| # | session mode | `ended_at` | kind | outcome |
| --- | --- | --- | --- | --- |
| 1 | `mock` | NULL | any | 409 Submit the mock test before overriding an answer |
| 2 | any | — | `mc_single` | 409 A multiple-choice answer cannot be overridden |
| 3 | `practice` | any | `flashcard` | 409 Grade the flashcard again instead of overriding it |
| 4 | any | — | any | 409 That answer was already marked correct (when `correct`) |
| 5 | any | — | any | 409 There is no answer to accept (when `given` normalises empty) |
| 6 | `mock` | set | `flashcard` | 200: flip only, `accepted_added: false`, `expected` from `answer_md` |
| 7 | any | — | `short_answer` | 200, unchanged, including the `accepted` insert |

**The order is the security property.** The mock-active gate runs before the kind check *and* before the already-correct check, so a live mock gets one identical refusal regardless of kind or verdict. Either running first would leak through its own message — this endpoint returns `expected` and distinguishes an already-correct review, which makes it a per-card answer-and-correctness oracle if left ungated. See design doc §8b.

The flashcard path runs **only** the `UPDATE`; no transaction is needed for a single statement and no `accepted` row is written. Practice flashcards stay non-overridable — making the check merely kind-aware would delete a decided Part 3 invariant.

`overriding_after_the_session_finished_still_works` (`tests/sessions.rs:1559`) and `refuses_to_override_a_multiple_choice_or_flashcard_review` (`:1480`) both pass **unchanged**. The second uses a practice session and asserts only the status, so it is the canary for gating on kind alone instead of (mode, kind).

**Integration tests:**
- [ ] `a_wrong_mock_flashcard_can_be_overridden_after_submitting` — 200, row `correct = 1` and `overridden = 1`, `accepted_added: false`, `expected == [answer_md]`
- [ ] `overriding_a_mock_flashcard_adds_no_accepted_row` — `COUNT(accepted) == 0` before and after
- [ ] **`overriding_is_refused_while_a_mock_test_is_unsubmitted`** — 409 for a wrong short-answer, a wrong flashcard and a correct answer, with **the identical message in all three**, and no `expected` or answer text in the body
- [ ] `a_practice_flashcard_still_cannot_be_overridden` — 409 with the practice wording
- [ ] `a_mock_multiple_choice_still_cannot_be_overridden` — 409 after submitting
- [ ] `overriding_a_mock_short_answer_still_teaches_the_card` — the `accepted` row is inserted and the wording grades correct next time
- [ ] `the_mock_summary_and_results_count_an_override_as_correct` — `/results` shows `correct: true, overridden: true, can_override: false`, and the summary's `correct_count` rises

**Mutation evidence:**

| Test | Single change |
| --- | --- |
| `overriding_is_refused_while_a_mock_test_is_unsubmitted` | delete the mock-active gate |
| same test, message uniformity | move the kind check above the mock gate |
| same test, correct case | move the already-correct check above the mock gate |
| `a_practice_flashcard_still_cannot_be_overridden` | make the flashcard branch mode-blind |
| `…adds_no_accepted_row` | run the `accepted` insert on the flashcard path |
| `…expected == [answer_md]` | read `expected` from `accepted` on the flashcard path |
| `…short_answer_still_teaches_the_card` | skip the `accepted` insert for mock short-answers |

**Verify:** `cargo test --test sessions --test mock && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/mock.rs"], "verifyCommand": "cargo test --test sessions --test mock && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build", "acceptanceCriteria": ["An incorrect mock flashcard review can be overridden after submitting, flipping correct and overridden with accepted_added false and expected equal to answer_md", "Overriding a mock flashcard writes no accepted row", "Override is refused with 409 while a mock session is unsubmitted, with one identical message across kinds and verdicts and no answer content in the body", "The mock-active gate is checked before the kind check and before the already-correct check", "A practice flashcard review still cannot be overridden and the existing practice test passes unchanged", "A multiple-choice review still cannot be overridden in either mode", "A mock short-answer override still inserts the accepted row and teaches the card", "Results and the summary reflect an override as correct"], "modelTier": "standard"}
```

---

## Task 9: Regenerate the sqlx cache and run the whole backend gate

**Goal:** The offline cache matches the code and the backend half is green before any frontend work begins.

**Files:** Modify `.sqlx/` (regenerated)

Build a scratch database in a temp directory by running `0001`, `0002`, `0003` in order with `sqlite3`, point `DATABASE_URL` at it, and prepare from the **repo root**:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
DATABASE_URL=sqlite:///tmp/part5-scratch/scratch.db cargo sqlx prepare --workspace
SQLX_OFFLINE=true cargo build
```

**Never point `DATABASE_URL` at `data/quizapp.db`.** Note that `cargo run` against a scratch database also needs `SQLX_OFFLINE=true`, or the macros go online and fail with roughly twenty errors that look like a compile catastrophe and are not.

**Acceptance Criteria:**
- [ ] `.sqlx/` regenerated against a **scratch** database, never `data/quizapp.db`
- [ ] `SQLX_OFFLINE=true cargo build` is clean with no stale cache entries left behind
- [ ] `cargo test` green, with no ignored or commented-out tests
- [ ] `cargo clippy --all-targets -- -D warnings` clean — `--all-targets` is what gives the new test file any lint coverage
- [ ] Migrations `0001`–`0003` byte-identical: `git diff --stat backend/migrations` is empty. Editing an applied migration changes its checksum and sqlx then refuses to run against an existing database
- [ ] Every mutation named in Tasks 1–8 has been run **one change at a time** and the named test recorded as going red

**Verify:** `cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build && git diff --stat backend/migrations`

```json:metadata
{"files": [".sqlx"], "verifyCommand": "cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build && git diff --stat backend/migrations", "acceptanceCriteria": ["The sqlx cache is regenerated against a scratch database and SQLX_OFFLINE=true cargo build is clean with no stale entries", "cargo test is green with no ignored tests", "clippy --all-targets -D warnings is clean", "Migrations 0001 to 0003 are byte-identical", "Every mutation named in tasks 1 to 8 was run one change at a time with the red test recorded"], "modelTier": "mechanical"}
```

---

## Task 10: API types and client functions for mock mode

**Goal:** Encode the mock contract in `api.ts` — three new `NextResponse` fields, the mock `/answer` view, and `/results` — with no `any` and no change to any practice type.

**Files:** Modify `frontend/src/lib/api.ts`

`NextResponse` gains `mode: SessionMode`, `target_count: number | null` and `started_at: string`. Flat, **not** a union discriminated on `mode`: `Session` already types `target_count` this way, and a union would force `SessionPage.tsx` to narrow for fields it never reads.

```ts
export type RecordedAnswer = { review_id?: never; correct?: never }

export type ResultQuestion = {
  review_id: number
  card_id: number
  kind: CardKind
  prompt_md: string
  image_path: string | null
  given: string | null
  self_grade: SelfGrade | null
  expected: string[]
  explanation_md: string | null
  correct: boolean
  overridden: boolean
  can_override: boolean
  ms: number | null
  answered_at: string
}

export type SessionResults = {
  summary: SessionSummary
  questions: ResultQuestion[]
}
```

```ts
  recordAnswer: (sessionId: number, input: SubmitAnswerInput) =>
    request<RecordedAnswer>('POST', `/sessions/${sessionId}/answer`, input),
  sessionResults: (sessionId: number, signal?: AbortSignal) =>
    request<SessionResults>('GET', `/sessions/${sessionId}/results`, undefined, signal),
```

**Two separately typed views of one URL, not a union.** The two responses are consumed by two pages that never see each other's data, and mock does not read the response body at all — it only needs to know the POST succeeded. A union would hand `MockSessionPage` a narrowing it has no use for. `RecordedAnswer` uses the codebase's established optional-`never` idiom (as `CreateSessionInput` and `SubmitAnswerInput` already do) so a mis-wired call cannot silently read `result.correct`.

`SubmitAnswerInput` is **not** modified — its first arm is already `{card_id, given, ms?}`, exactly the mock flashcard shape. Stated so nobody "fixes" it.

**Acceptance Criteria:**
- [ ] `NextResponse` carries `mode`, `target_count` and `started_at` alongside the existing four fields
- [ ] `AnswerResult`, `SubmitAnswerInput`, `OverrideResult` and `SessionSummary` are unchanged
- [ ] `SessionResults` nests `summary` rather than spreading it, matching the wire shape
- [ ] `recordAnswer` and `sessionResults` go through the existing `request` wrapper, so `ApiError` and `byField()` work unchanged
- [ ] No `any`, no `as` cast outside the pre-existing `request` wrapper, no `@ts-ignore`
- [ ] `ResultQuestion.given`, `.self_grade` and `.ms` are nullable

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm exec oxlint`

```json:metadata
{"files": ["frontend/src/lib/api.ts"], "verifyCommand": "cd frontend && pnpm exec tsc -b --noEmit && pnpm exec oxlint", "acceptanceCriteria": ["NextResponse carries mode, target_count and started_at alongside the existing four fields", "AnswerResult, SubmitAnswerInput, OverrideResult and SessionSummary are unchanged", "SessionResults nests summary rather than spreading it", "recordAnswer and sessionResults go through the existing request wrapper so ApiError and byField work unchanged", "No any, no cast outside the pre-existing request wrapper, no ts-ignore", "ResultQuestion given, self_grade and ms are nullable", "SubmitAnswerInput is not modified"], "modelTier": "mechanical"}
```

---

## Task 11: Extract `formatDuration`, add `formatClock`

**Goal:** One place to format time, without changing a pixel of the practice summary.

**Files:** Create `frontend/src/lib/format.ts`; modify `frontend/src/components/session/SessionSummary.tsx`

Move `formatDuration` from `SessionSummary.tsx:6-11` **body unchanged**. `lib/format.ts`, not `lib/utils.ts` — the latter is shadcn-generated and treated as vendored third-party code.

```ts
export function formatClock(totalMilliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(totalMilliseconds / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  const paddedSeconds = String(seconds).padStart(2, '0')
  return hours === 0
    ? `${minutes}:${paddedSeconds}`
    : `${hours}:${String(minutes).padStart(2, '0')}:${paddedSeconds}`
}
```

Two formats deliberately: `formatDuration` is the prose form for a summary tile, `formatClock` is the zero-padded running clock.

**Acceptance Criteria:**
- [ ] `formatDuration`'s body is identical to the version removed from `SessionSummary.tsx`
- [ ] The local copy is deleted — `noUnusedLocals` would otherwise fail the gate
- [ ] `SessionSummary.tsx`'s rendered output is unchanged: same tiles, same strings, same classes
- [ ] `formatClock` zero-pads seconds, omits the hours segment below one hour, floors rather than rounds, and clamps a negative input to `0:00`
- [ ] `formatClock(0)` is `0:00`, `formatClock(67000)` is `1:07`, `formatClock(3787000)` is `1:03:07`
- [ ] `format.ts` exports functions only — no component, no React import

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

```json:metadata
{"files": ["frontend/src/lib/format.ts", "frontend/src/components/session/SessionSummary.tsx"], "verifyCommand": "cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint", "acceptanceCriteria": ["formatDuration's body is identical to the version removed from SessionSummary.tsx", "The local copy in SessionSummary.tsx is deleted", "SessionSummary.tsx's rendered output is unchanged", "formatClock zero-pads seconds, omits hours below one hour, floors rather than rounds, and clamps negatives to 0:00", "formatClock(0) is 0:00, formatClock(67000) is 1:07, formatClock(3787000) is 1:03:07", "format.ts exports functions only with no React import"], "modelTier": "mechanical"}
```

---

## Task 12: Extract `SummaryTiles`

**Goal:** One four-tile summary grid, shared by the practice summary and the mock results screen.

**Files:** Create `frontend/src/components/session/SummaryTiles.tsx`; modify `frontend/src/components/session/SessionSummary.tsx`

Move the `<dl>` from `SessionSummary.tsx:21-42` verbatim. Props `{ summary: SessionSummary }`. `SessionSummary.tsx` keeps its heading, its `overridden_count` line and its "Back to decks" button, rendering `<SummaryTiles>` between them.

**Acceptance Criteria:**
- [ ] `SummaryTiles` takes `summary: SessionSummary` and renders the same four tiles with identical markup and classes
- [ ] The accuracy expression and the `formatDuration` call move across unchanged
- [ ] `SessionSummary.tsx` renders exactly as before
- [ ] `MockResults` can pass `results.summary` straight in, with no adapter
- [ ] Named export, props type declared directly above the component, matching the `components/session/` convention

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

```json:metadata
{"files": ["frontend/src/components/session/SummaryTiles.tsx", "frontend/src/components/session/SessionSummary.tsx"], "verifyCommand": "cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint", "acceptanceCriteria": ["SummaryTiles takes summary: SessionSummary and renders the same four tiles with identical markup and classes", "The accuracy expression and the formatDuration call move across unchanged", "SessionSummary.tsx renders exactly as before", "results.summary can be passed straight in with no adapter", "Named export with the props type declared directly above the component"], "modelTier": "mechanical"}
```

---

## Task 13: The run header and the timer

**Goal:** Progress and an elapsed clock, with the tick isolated so it cannot re-render the question.

**Files:** Create `frontend/src/components/session/MockTimer.tsx`, `frontend/src/components/session/MockRunHeader.tsx`

```tsx
type MockTimerProps = { startedAt: string }

export function MockTimer({ startedAt }: MockTimerProps) {
  const startedAtMilliseconds = Date.parse(startedAt)
  const [elapsedMilliseconds, setElapsedMilliseconds] = useState(() =>
    Math.max(0, Date.now() - startedAtMilliseconds),
  )

  useEffect(() => {
    const interval = setInterval(
      () => setElapsedMilliseconds(Math.max(0, Date.now() - startedAtMilliseconds)),
      1000,
    )
    return () => clearInterval(interval)
  }, [startedAtMilliseconds])

  return (
    <span role="timer" aria-hidden="true" className="font-display tabular-nums">
      {formatClock(elapsedMilliseconds)}
    </span>
  )
}
```

**The ticking state lives in this leaf.** `MockRunHeader` receives `startedAt` and passes it down; it never receives an elapsed value. Lifting it into the page would re-render the prompt's `<Markdown>` — `react-markdown` with KaTeX — once per second, worst on the 100+ card COS781 deck the handover already flags as an unverified responsiveness worry.

Four rules, each a bug if reversed: the baseline is the server's `started_at`, not mount time, so a reload continues the clock; each tick recomputes from `Date.now()` rather than incrementing, so a throttled tab cannot drift; the value is floored and clamped at zero, because one-second `started_at` resolution can otherwise show a negative; and it is `aria-hidden` with no live region, because a per-second announcement is a screen-reader firehose. `Date.parse` is safe because these timestamps are ISO-8601 with `Z`.

`MockRunHeader` props: `{ questionNumber, totalQuestions, startedAt, onEndEarly, ending }`. Shell `rounded-xl border bg-card px-4 py-2.5 shadow-sm`, matching `SessionPage.tsx:276`. Progress bar `bg-brand` on a `bg-secondary` track.

`usePrefersReducedMotion` is deliberately **not** used: a count-up clock is content, not decoration, and suppressing it would remove information. No transition is added on question change either.

**Acceptance Criteria:**
- [ ] `MockTimer` owns its own `useState` and `setInterval`; no elapsed value is passed in or lifted out
- [ ] The baseline is `started_at`, not mount time — a reload continues the clock rather than restarting it
- [ ] Each tick recomputes from the current time rather than incrementing
- [ ] The interval is cleared on unmount and re-created only when `startedAt` changes
- [ ] The clock is `aria-hidden`, `role="timer"`, no `aria-live`, and `tabular-nums`
- [ ] The header reads "Question N of M" from `answered_count + 1` and `target_count`, **never** `pool_count`, and N never exceeds M
- [ ] No verdict, streak, sparkle or correctness signal appears anywhere in the header
- [ ] Only `bg-brand` and `bg-secondary` are used for the progress bar; no new colour token
- [ ] `python3 frontend/scripts/check-contrast.py` reports the same row count as before Part 5, with an empty RECORDED tier

**Verify:** `python3 frontend/scripts/check-contrast.py && cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

```json:metadata
{"files": ["frontend/src/components/session/MockTimer.tsx", "frontend/src/components/session/MockRunHeader.tsx"], "verifyCommand": "python3 frontend/scripts/check-contrast.py && cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint", "acceptanceCriteria": ["MockTimer owns its own useState and setInterval with no elapsed value passed in or lifted out", "The baseline is started_at rather than mount time so a reload continues the clock", "Each tick recomputes from the current time rather than incrementing", "The interval is cleared on unmount and re-created only when startedAt changes", "The clock is aria-hidden with role=timer, no aria-live, and tabular-nums", "The header reads Question N of M from answered_count plus one and target_count, never pool_count", "No verdict, streak or correctness signal appears in the header", "The progress bar uses only bg-brand and bg-secondary with no new colour token", "check-contrast.py reports the same row count as before with an empty RECORDED tier"], "modelTier": "standard"}
```

---

## Task 14: The results screen

**Goal:** Every question in answer order with the student's answer, the correct answer, the explanation and right/wrong — plus per-row override, reusing only tokens the contrast script already proves.

**Files:** Create `frontend/src/components/session/ResultRow.tsx`, `frontend/src/components/session/MockResults.tsx`

`ResultRow` props: `{ question: ResultQuestion; index: number; onOverride: (reviewId: number) => void; overriding: boolean }`.

**Kind-agnostic.** A results row carries no `choices` and no choice ids — only `given` text and `expected: string[]`. All three kinds render identically: prompt, image, your answer, expected, explanation — differing only in a kind badge from the existing `KIND_LABEL`. `ChoiceList.tsx` is therefore **untouched** by Part 5: reused verbatim during the run, not involved afterwards. A second choice renderer with correctness styling is exactly what the "one rendering path" rule exists to prevent.

Layout `rounded-xl border bg-card p-4 shadow-sm` plus `border-l-4 border-success` / `border-l-4 border-destructive`. The chip uses `bg-success text-success-foreground` / `bg-destructive text-destructive-foreground` — the construction `AnswerVerdict.tsx:32-37` already uses, whose pairs are already enforced in the contrast script. Reuse `AnswerVerdict`'s wording ("Correct" / "Counted as correct" / "Not quite") so the two screens agree, and pair it with a lucide `Check`/`X` so correctness is never encoded by colour alone.

`expected` renders as primary-then-alternates through `<Markdown>`, as `AnswerVerdict` already does. `explanation_md` reuses its `border-t pt-3 text-sm text-muted-foreground` treatment.

`MockResults` props: `{ results: SessionResults; onOverride: (reviewId: number) => void; overridingReviewId: number | null }`. Heading, `<SummaryTiles summary={results.summary} />`, the flashcard note when any question is a flashcard, the row list, "Back to decks". Do **not** reuse `SessionExhausted` — its heading is "Nothing left to practise", wrong here.

**Forbidden: any alpha tint** (`bg-success/10` and friends). An alpha-composited colour's contrast depends on the surface beneath it — the class of bug Part 4 fixed at 2.14:1 — light mode's stack has only ~1.2:1 between adjacent surfaces so the tint would be invisible in Makka Pakka, and it would create a new pair needing hand-chosen values and new enforced rows in both themes.

**Acceptance Criteria:**
- [ ] Every review renders, in answer order, one numbered row each
- [ ] All three kinds render through one code path; no row reads `choices` or a choice id
- [ ] Correctness is carried by an opaque chip **and** an icon **and** a text label — never colour alone, never an alpha tint
- [ ] Only `--success`, `--destructive` and their existing `-foreground` partners are used; no new token in either palette and no new row in `check-contrast.py`
- [ ] `python3 frontend/scripts/check-contrast.py` reports the same row count as before Part 5
- [ ] "I was right" appears only when `can_override && !overridden`, and disables while its request is in flight
- [ ] An override updates that row **and** the summary tiles' correct count and accuracy in place, without refetching `/results`
- [ ] An overridden row reads "Counted as correct", matching `AnswerVerdict`'s wording
- [ ] When the run contains any flashcard, a note explains that flashcards are matched against their written answer and invites marking
- [ ] `image_path` renders through the existing `CardImage`; markdown and KaTeX through the existing `Markdown`
- [ ] New surfaces are `rounded-xl`

**Verify:** `python3 frontend/scripts/check-contrast.py && cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

```json:metadata
{"files": ["frontend/src/components/session/ResultRow.tsx", "frontend/src/components/session/MockResults.tsx"], "verifyCommand": "python3 frontend/scripts/check-contrast.py && cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint", "acceptanceCriteria": ["Every review renders in answer order as one numbered row each", "All three kinds render through one code path and no row reads choices or a choice id", "Correctness is carried by an opaque chip plus an icon plus a text label, never colour alone and never an alpha tint", "Only success and destructive and their existing foreground partners are used, with no new token and no new contrast-script row", "check-contrast.py reports the same row count as before Part 5", "I was right appears only when can_override and not overridden, and disables while in flight", "An override updates the row and the summary tiles in place without refetching results", "An overridden row reads Counted as correct matching AnswerVerdict's wording", "When the run contains any flashcard a note explains the matching and invites marking", "image_path renders through CardImage and markdown through Markdown", "New surfaces are rounded-xl"], "modelTier": "standard"}
```

---

## Task 15: `MockSessionPage` — the runner and its state machine

**Goal:** The mock run: a `/next`-first state machine, keyboard-first input with no feedback of any kind, and a reload that lands on the same card.

**Files:** Create `frontend/src/pages/MockSessionPage.tsx`; modify `frontend/src/pages/SessionPage.tsx`

**State machine:**

```
mount        → GET /next
                 200 → if (mode !== 'mock') navigate('/session/:id', {replace:true}); else run
                 409 → POST /finish → GET /results → render results
submit       → POST /answer (recordAnswer) → GET /next → same two outcomes
end early    → POST /finish → GET /results
```

`/next`-first, **not** `/results`-first: it never requests the answer-key endpoint while the run is live, which makes the no-leak property structural rather than dependent on the server's 409, and it makes the walkthrough's Network check a simple absence assertion. Its cost — one redundant idempotent `POST /finish` per results reload — is accepted.

**Keyboard handling** — a strict subset of practice's, container-level on a `tabIndex={-1}` div, no window listener:

```ts
function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
  if (busy || results !== null || card === null) return

  if (event.key === 'Enter') {
    event.preventDefault()
    submit()
    return
  }

  if (card.kind !== 'mc_single') return

  const digit = Number(event.key)
  if (!Number.isInteger(digit) || digit < 1 || digit > 9) return
  const choice = card.choices[digit - 1]
  if (choice) {
    event.preventDefault()
    setSelectedChoiceId(choice.id)
  }
}
```

**Do not port `SessionPage.tsx:222`'s `if (typing) return`.** Digits are only ever wanted for `mc_single`, and a mock `mc_single` card has no text input mounted at all, so gating on kind makes typing interference *structurally* impossible rather than defensively guarded — and porting the guard would imply the digits could otherwise fire during typing, which is misleading. **No Space branch** either: in mock a flashcard is a text input, so a Space branch would eat spaces out of a typed answer.

**Double-submit guard.** In practice, Enter alternates submit/advance, so two keydowns degrade harmlessly to "advance". In mock one Enter does both, and `busy` is React state, so two keydowns in one tick can both read it as false and both post. Holding Enter is exactly what a keyboard-first runner invites. Guard with a ref checked and set **synchronously**, beside the state that drives the UI:

```ts
const submitting = useRef(false)
async function send(input: SubmitAnswerInput) {
  if (sessionId === null || submitting.current) return
  submitting.current = true
  setBusy(true)
  try { /* … */ } finally { submitting.current = false; setBusy(false) }
}
```

`submit()` dispatches on kind: `mc_single` needs `selectedChoiceId !== null` and sends `{choice_id}`; `short_answer` and `flashcard` both need `typedAnswer.trim() !== ''` and send `{given: typedAnswer}`. Both send `ms: elapsedSince(servedAt.current)`.

Copy `elapsedSince` (`SessionPage.tsx:35-37`) and the `/^\d+$/` id parse (`:41`) rather than extracting them — Part 4 already set the precedent of refusing a refactor that rides along with a feature, and extracting means editing two working pages for zero behaviour change. Reuse the abort-controller pattern from `SessionPage.tsx:63-98` verbatim, including the stale-response guard and the unmount abort.

Hint text says **"submit"**, never "check" — "Check" implies feedback. The button label is **"Submit"**.

**`SessionPage.tsx` change, the only one:** inside `loadNext`, after the response lands, redirect to `/mock/${sessionId}` when `response.mode === 'mock'`. Nothing else in that file moves.

**Acceptance Criteria:**
- [ ] All three kinds render and submit: `mc_single` via `ChoiceList`, `short_answer` and `flashcard` both via `Input` as free text
- [ ] A mock flashcard shows **no** reveal button and **no** self-grade buttons, and posts `{card_id, given, ms}`
- [ ] No verdict, expected answer, explanation, streak badge or sparkle burst can render — none of those components is imported
- [ ] `1`–`9` selects a multiple-choice option; digits typed into the answer input are inserted as characters, never intercepted
- [ ] Space typed into the answer input inserts a space and triggers no action
- [ ] `Enter` submits and advances in one press; **holding `Enter` cannot post two answers for one card**
- [ ] Focus lands on the input for `short_answer` and `flashcard`, and on the container for `mc_single`; no window-level listener is added
- [ ] Reloading mid-run lands on the same card, with the same question number and a clock that continues
- [ ] A `/next` 409 finishes the session and shows results; reloading the results URL shows them again
- [ ] `/mock/:id` for a practice session redirects to `/session/:id`, and `/session/:id` for a mock session redirects to `/mock/:id`
- [ ] "End test early" finishes and shows results for what was answered
- [ ] `ms` is sent on every answer and is never negative
- [ ] An unreachable server toasts and leaves the page usable; a non-numeric id renders a "Session not found" block
- [ ] **`/results` is never requested while the run is live**

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

```json:metadata
{"files": ["frontend/src/pages/MockSessionPage.tsx", "frontend/src/pages/SessionPage.tsx"], "verifyCommand": "cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint", "acceptanceCriteria": ["All three kinds render and submit, with short_answer and flashcard both as free text via Input", "A mock flashcard shows no reveal button and no self-grade buttons and posts card_id, given and ms", "No verdict, expected answer, explanation, streak badge or sparkle burst can render and none is imported", "Digits 1 to 9 select a multiple-choice option and digits typed into the answer input are inserted as characters", "Space typed into the answer input inserts a space and triggers no action", "Enter submits and advances in one press and holding Enter cannot post two answers for one card", "Focus lands on the input for typed kinds and on the container for mc_single with no window listener", "Reloading mid-run lands on the same card with the same question number and a continuing clock", "A /next 409 finishes the session and shows results, and reloading the results URL shows them again", "A mismatched mode redirects between /mock/:id and /session/:id in both directions", "End test early finishes and shows results for what was answered", "ms is sent on every answer and is never negative", "An unreachable server toasts and a non-numeric id renders Session not found", "/results is never requested while the run is live"], "modelTier": "frontier"}
```

---

## Task 16: Wire the route and enable the deck-page tile

**Goal:** Make the mock test reachable — the last two edits in the slice.

**Files:** Modify `frontend/src/App.tsx`, `frontend/src/pages/DeckPage.tsx`

`App.tsx`: add `<Route path="/mock/:id" element={<MockSessionPage />} />`. No lazy loading — the bundle is already 884 kB and code-splitting is deferred to build step 8.

`DeckPage.tsx:43-48`: `available: true`, and

```ts
    note: 'Every card in the deck, once, in a fixed order. No feedback until the end.',
```

This note is the **only** place the student learns the test's shape, since there is no pre-start dialog. It cannot interpolate `deck.card_count` (it is a module const) and does not need to — the run header supplies the number immediately.

`startSession` branches its destination on mode via a lookup rather than an inline ternary, so Part 7's `sm2` has an obvious slot. The `disabled` guard and the `title` need no change: `card_count` already excludes archived cards, which is exactly the count the server computes as `target_count`.

**Acceptance Criteria:**
- [ ] `/mock/:id` renders `MockSessionPage` inside the `AppShell` layout
- [ ] The Mock test tile is enabled, and its note describes the test rather than naming a build part
- [ ] Practice still navigates to `/session/:id`; mock navigates to `/mock/:id`; `sm2` stays disabled
- [ ] Both tiles remain disabled when `deck.card_count === 0` and while a session is starting, with the "Add a card to this deck first" title on an empty deck
- [ ] The tile's icon gets `text-brand` now that `available` is true, consistent with Practice
- [ ] No lazy loading or `Suspense` is introduced

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

```json:metadata
{"files": ["frontend/src/App.tsx", "frontend/src/pages/DeckPage.tsx"], "verifyCommand": "cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint", "acceptanceCriteria": ["/mock/:id renders MockSessionPage inside the AppShell layout", "The Mock test tile is enabled and its note describes the test rather than naming a build part", "Practice navigates to /session/:id, mock to /mock/:id, and sm2 stays disabled", "Both tiles remain disabled at card_count zero and while starting, with the Add a card first title on an empty deck", "The tile's icon gets text-brand now that available is true", "No lazy loading or Suspense is introduced"], "modelTier": "mechanical"}
```

---

## Task 17: Full gate, browser walkthrough, handover — USER GATE

**Goal:** Run the whole gate, drive the feature in a browser, and record honestly what was and was not observed.

**Automated gate**, from the repo root:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
python3 frontend/scripts/check-contrast.py
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
```

The contrast script must report the **same row count as before Part 5, with an empty RECORDED tier**. An unchanged count is the evidence that no new colour pair crept in.

**Start the servers the way that actually works here:**

```bash
export SQLX_OFFLINE=true
cargo run &
cd frontend && pnpm dev --host 0.0.0.0
```

Then navigate to the **Network** address vite prints (e.g. `http://192.168.2.161:5273`), **not** `localhost`. Without `--host`, vite binds only to `localhost`, which resolves to IPv6 `::1`, while Chrome asks for IPv4. This is what unblocked Part 3's walkthrough after four parts of "the browser could not reach the dev server".

**Browser walkthrough.** Prepare a deck with at least eight cards covering all three kinds, at least one with an image, one with KaTeX and one with a markdown list.

- [ ] 1. On `/decks/:id` the Mock test tile is enabled, its icon is `text-brand`, and its note describes the test — not "Arrives in part 5."
- [ ] 2. Clicking it goes to `/mock/<id>`. The header reads "Question 1 of 8", matching the deck's card count, with the clock near `0:00`.
- [ ] 3. No feedback machinery is on screen: no verdict banner, no streak badge, the action reads **Submit** not "Check", and the hint offers only "Enter to submit" (plus "1–9 to choose" on multiple choice).
- [ ] 4. On multiple choice, `3` highlights the third option and Enter submits and advances in one press, with **no verdict in between**. The counter and progress bar advance.
- [ ] 5. **Hold Enter** with nothing selected — nothing happens, no request fires. Select an option and hold Enter — exactly **one** card advances and Network shows exactly **one** `POST /answer`.
- [ ] 6. On a short answer, type text containing a **digit and a space** (`k means 3 clusters`) — every character lands; no digit selects anything, no space triggers anything.
- [ ] 7. On a flashcard there is **no "Show answer" button and no Again/Hard/Good/Easy buttons** — just a text input. Typing and Enter advances with no feedback.
- [ ] 8. Watch the clock tick, then reload mid-run: **the same card** is served, the question number is unchanged, the options are in the **same order**, and the clock continues rather than restarting.
- [ ] 9. **The leak check — DevTools Network, filter `/api/`.** Each `GET /next` card object has exactly `id`, `kind`, `prompt_md`, `image_path`, `choices`, and each choice exactly `id` and `text_md` — no `is_correct`, `answer_md`, `explanation_md` or `accepted`, on all three kinds. Each `POST /answer` response carries no `correct`, `expected`, `explanation_md`, `can_override` or `review_id`. **`GET /sessions/:id/results` does not appear at all** — not as a 200, not as a 409. Repeat after a reload.
- [ ] 10. Answering the last card shows the results screen via `POST /finish` then `GET /results`, in that order, with no extra `/next` serve of a card already seen.
- [ ] 11. Results list all eight questions in answer order, each numbered, with the prompt, your answer, the correct answer, the explanation where one exists, and a right/wrong chip with an icon and a label. The tiles show answered, correct, accuracy and a total time consistent with the clock.
- [ ] 12. Squint at a right and a wrong row: the marker reads as a chip with a border stripe, not a barely-there tint — and it is still readable **with the colour ignored**, from the icon and label alone.
- [ ] 13. "I was right" on a wrong **short answer** row flips it to "Counted as correct" in place, and the correct count and accuracy update immediately without a reload or a re-order.
- [ ] 14. Same on a **flashcard** row. The flashcard note is present, so a flashcard-heavy run does not report a misleading score with no explanation.
- [ ] 15. Reload the results URL — results render again, complete, with the overrides still applied. (A second `POST /finish` fires; expected and harmless.)
- [ ] 16. Hand-edit the URL to `/session/<that mock id>` — you are redirected back to `/mock/<id>`. Start a practice session and hand-edit to `/mock/<practice id>` — redirected back to `/session/<id>`.
- [ ] 17. **Makka Pakka (light)**, then **Bibble (dark)**: repeat points 3, 11 and 12, looking specifically at the progress bar against its track, the chips, the border stripes, and KaTeX inside a results row.
- [ ] 18. Turn on **Reduce motion** and run a mock test: the clock still ticks — it is content, not decoration — and nothing else animates.
- [ ] 19. Start a test, answer two cards, click **End test early** — results show two questions.
- [ ] 20. A one-card deck: "Question 1 of 1", submit, straight to a one-row results screen. A zero-card deck still has the tile disabled with the "Add a card to this deck first" tooltip.
- [ ] 21. **A live mock cannot be probed.** Mid-run, POST `/api/reviews/<a guessed id>/override` and GET `/api/sessions/<id>/results` by hand — both 409, and neither body carries an expected answer or a verdict.

**Handover and spec updates:**
- [x] `docs/HANDOVER.md`: update the **Last updated** line and commit; add a Part 5 section to "Where things stand"; replace "Next up" with Part 6 (stats)
- [x] Record how each of the three Part 5 questions was resolved, replacing the "Three things Part 5 must resolve" block
- [x] Record the Zustand decline with its reason — a prior design doc predicted otherwise, and that prediction must be visibly overturned rather than silently ignored
- [x] Record the flashcard prose caveat and the advice to keep mock-test flashcard answers short
- [x] Record the accepted false-accept cost (`type i error` / `type ii error`) and that "mark wrong" is the follow-up if it annoys
- [x] Record the redundant `POST /finish` on results reload as a known-and-accepted minor
- [x] Record **375px phone width** as still never rendered — now across Parts 1, 2b, 2c, 3, 4 and 5
- [x] Master spec: amend the Study engine paragraph and the API list per the two amendments at the top of this plan
- [x] **Anything not actually clicked is recorded as not verified.** This document has never yet claimed a walkthrough that did not happen.

**Verify:** `cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build && python3 frontend/scripts/check-contrast.py && cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

```json:metadata
{"userGate": true, "tags": ["user-gate"], "modelTier": "standard"}
```

---

## Dependencies

```
Task 1 ─┐
Task 2 ─┼─> Task 3 ─> Task 4 ─> Task 5 ─> Task 6 ─> Task 7 ─> Task 8 ─> Task 9 ─┐
        │                                                                        │
Task 11 ─> Task 12 ─┐                                                            │
                    ├─> Task 13 ─┐                                               │
Task 10 <───────────┘             ├─> Task 15 ─> Task 16 ─> Task 17 <────────────┘
                    └─> Task 14 ─┘
```

Tasks 1 and 2 are independent of each other and of everything else. Tasks 3–8 all edit `backend/src/routes/sessions.rs`, so they are strictly sequential. Task 10 needs the backend contract settled (Task 9). Task 11 before Task 12 — both edit `SessionSummary.tsx`, and extracting after moving means doing it twice. Tasks 13 and 14 before Task 15, which renders both.

---

## Notes for whoever executes this

**Run one implementer at a time.** Git's index is not per-file: in Part 2a one agent's `git add`/`commit` swept another's staged work into a commit labelled as something else. Read-only reviewers can run alongside an implementer; two writers cannot. Tasks 3–8 all touch the same file, so this is not optional here.

**All cargo commands run from the repo root**, never from `backend/`. The cwd-relative `DATABASE_URL` default resolves to `data/` at the root; running from `backend/` silently creates `backend/data/`.

**`sqlx-cli` is installed but not on PATH.** `export PATH="$HOME/.cargo/bin:$PATH"` first.

**`tsc -b --noEmit`, never bare `tsc --noEmit`.** `frontend/tsconfig.json` is a solution file with `"files": []`, so a bare run finds zero files and exits 0 whatever the code says.

**pnpm, not npm.** If a `package-lock.json` appears, delete it.

**CLAUDE.md is enforced, mechanically.** No comments in code — the `mix64` and `mock_order` explanations in Task 1 belong in the design doc, not in the file. No abbreviated identifiers. No `any`: `typescript/no-explicit-any` is `error` and oxlint is in the gate. `strict`, `noUnusedLocals` and `noUnusedParameters` are all on.

**`frontend/src/components/ui/` is vendored shadcn.** Part 5 does not touch it, including `lib/utils.ts`.

**Mutating a SQL string needs the sqlx cache regenerated first.** `query!` checks against the offline cache, so editing a query to prove it is load-bearing fails to *compile* under `SQLX_OFFLINE=true` rather than running and going red — which reads as "the mutation did nothing". Back up `.sqlx/`, run `cargo sqlx prepare` against the scratch database with the mutation in place, run the test, then restore both. This cost a confusing round on Task 3.

**Demand mutation evidence, one change at a time.** Part 3's mutation pass found six tests that could not fail. "I removed X and the test went red" is only evidence for X if X was the sole change. Every mutation table in this plan names a single edit for a reason.

**Give reviewers both sides of a seam.** A per-task review structurally cannot see an API-to-client mismatch. Tasks 10 and 15 are the seam; review them against Tasks 4, 6 and 7, not alone.

**The nightly occasionally throws a self-recovering incremental-compilation ICE** (`unstable fingerprints for evaluate_obligation`) and drops `rustc-ice-*.txt` files. Gitignored, harmless, cleared by `cargo clean`. Every build still completes. Do not confuse it with the twenty-error cascade that means the sqlx macros went online — that one is fixed by `export SQLX_OFFLINE=true`.

**Three leaks are easy to reintroduce**, and none of them is `/answer`: `correct_count` on the mock serve (Task 4), `/reveal` in a mock session (Task 5), and the override endpoint during a live mock run (Task 8). Design doc §5 and §8b. If a later change touches any session endpoint, re-read those two sections before assuming the leak tests cover it.
