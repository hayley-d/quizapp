# Part 3: Practice Mode — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use mitis:subagent-driven-development (recommended) or mitis:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A keyboard-first practice runner: pick decks at `/study`, answer weighted-sampled cards at `/session/:id`, graded server-side against `accepted.normalised`, with an "I was right" override and every answer recorded in `reviews`.

**Architecture:** Two pure Rust modules carry the logic with no database access — `grading.rs` (per-kind verdicts) and `practice.rs` (weighting and selection, randomness injected as a `roll: f64`). One route module `routes/sessions.rs` exposes six endpoints and holds all the SQL. Session state lives *only* in `reviews`: the weights, the staleness, the no-repeat window and the progress counts are all derived from it, so the browser holds no queue and a reload resumes correctly. The frontend adds two pages over the existing `request()` wrapper and reuses `<Markdown>` and `<CardImage>` rather than adding rendering paths.

**Tech Stack:** Rust/axum 0.8/sqlx 0.8 (SQLite, offline macro cache at repo-root `.sqlx/`), `rand` 0.8 (new direct dependency), React 19 + TypeScript + Vite, Tailwind v4, shadcn/Radix primitives.

**Design doc:** [`../specs/2026-08-27-part3-practice-mode-design.md`](../specs/2026-08-27-part3-practice-mode-design.md) — contracts, rulings and rationale. **Read it before Task 4.** The master spec [`../specs/2026-08-26-quiz-study-app-design.md`](../specs/2026-08-26-quiz-study-app-design.md) has been amended where Part 3 changed it.

**User decisions (already made):**
- Card order is the spec's **weighted sampling**, not `cards.position`. `position` plays no part in practice mode.
- Flashcard self-grades get their own column — migration `0003`, `reviews.self_grade`, with `correct` derived.
- Both screens ship: `/study` (mode + deck picker) and `/session/:id` (the runner).
- Part 2c's outstanding browser walkthrough and its five known defects are **out of scope** and stay on the handover's outstanding list. In particular `strict` stays off in `frontend/tsconfig.app.json`.
- `CLAUDE.md` gains a no-`any` rule *first*, so the runner is written under it.

**Two spec amendments this plan implements** (both in the design doc, §3 and §4):
- `POST /api/sessions/:id/answer` takes `{card_id, given | choice_id | self_grade, ms?}`. The spec's `{given}` could not identify the card, express a choice, or express a self-grade.
- `POST /api/sessions/:id/reveal` is new, flashcard-only, and `/next` carries no answer content for **any** kind.

---

## File Structure

| Path | Responsibility |
|---|---|
| `CLAUDE.md` | **done** — rule 3, never use `any` in TypeScript |
| `backend/Cargo.toml` | **done** — `rand = "0.8"` |
| `backend/migrations/0003_review_self_grade.sql` | **done** — `reviews.self_grade`, nullable, CHECK-constrained |
| `backend/src/grading.rs` | **new** — pure per-kind grading, no database |
| `backend/src/practice.rs` | **new** — pure weighting, selection, row folding, no database |
| `backend/src/lib.rs` | register both new modules |
| `backend/src/routes/sessions.rs` | **new** — six endpoints, all the SQL |
| `backend/src/routes/mod.rs` | `pub mod sessions;` + one `.merge()` |
| `backend/tests/sessions.rs` | **new** — integration coverage, including the leakage rule |
| `frontend/src/lib/api.ts` | session types and the five calls |
| `frontend/src/pages/StudyPage.tsx` | **new** — mode + deck picker |
| `frontend/src/pages/SessionPage.tsx` | **new** — the runner |
| `frontend/src/components/session/` | **new** — the per-kind answer inputs and the verdict panel |
| `frontend/src/App.tsx` | `/study` replaces its stub; `/session/:id` added |
| `docs/HANDOVER.md` | record what shipped |

Grading and weighting live in separate modules from the routes because the spec requires it — "pure functions in their own Rust modules with no database access… they form the bulk of the test suite" — and because that is what makes the never-seen dominance proof and the window non-starvation theorem testable without a database.

---

## Task 1: `CLAUDE.md` rule 3, never use `any` — **COMPLETE**

**Goal:** The no-`any` rule is written down before any of Part 3's TypeScript exists.

**Files:** Modify `CLAUDE.md`

**Acceptance Criteria:**
- [x] `CLAUDE.md` has a numbered rule 3 between rule 2 and Accepted short forms
- [x] It bans bare `any`, `any[]`, `Array<any>`, `Promise<any>`, `Record<string, any>`, `as any`, `as unknown as T` laundering, `any` as a generic argument or type-parameter default, and `@ts-ignore`/`@ts-expect-error` used to hide a typing problem
- [x] It names `unknown` as the correct escape hatch and cites the existing `catch (error: unknown)` → `error instanceof ApiError` path
- [x] It exempts `frontend/src/components/ui/` as vendored shadcn code
- [x] The codebase still has zero `any` usages

**Notes:** Prose-only by decision. `typescript/no-explicit-any` was **not** added to `frontend/.oxlintrc.json`, and `pnpm lint` is not in the verification gate, so nothing mechanically enforces the rule yet. Adding both is a two-line follow-up if wanted.

```json:metadata
{"files": ["CLAUDE.md"], "verifyCommand": "grep -q 'Never use `any` in TypeScript' CLAUDE.md && cd frontend && ! grep -rE ': any|as any|<any>|any\\[\\]' src --include='*.ts' --include='*.tsx'", "acceptanceCriteria": ["CLAUDE.md has a numbered rule 3 between rule 2 and Accepted short forms", "It bans bare any, any[], Array<any>, Promise<any>, Record<string, any>, as any, as unknown as T laundering, any as a generic argument, and @ts-ignore/@ts-expect-error used to hide typing problems", "It names unknown as the correct escape hatch", "It exempts frontend/src/components/ui/ as vendored shadcn code", "The codebase still has zero any usages"], "modelTier": "mechanical"}
```

---

## Task 2: `rand` and migration `0003` — **COMPLETE**

**Goal:** The self-grade column exists and `rand` is a declared dependency, without adding a second major version to the tree.

**Files:** Create `backend/migrations/0003_review_self_grade.sql`; modify `backend/Cargo.toml`

**Acceptance Criteria:**
- [x] `reviews.self_grade TEXT CHECK (self_grade IN ('again','hard','good','easy'))`, nullable
- [x] A NULL `self_grade` is accepted (the two auto-graded kinds have none)
- [x] `'good'` is accepted; `'medium'` is rejected by the CHECK
- [x] `0001` and `0002` are untouched — editing an applied migration changes its checksum and sqlx then refuses to run
- [x] `rand = "0.8"` in `backend/Cargo.toml`, resolving to the already-locked 0.8.8
- [x] `cargo test` passes with no regressions

**Verify:** `cargo test && SQLX_OFFLINE=true cargo build` — 119 tests pass, build clean.

**Evidence captured:** all 119 tests green; `rand 0.8.8` compiled (no second major version); the CHECK's three behaviours (NULL accepted, `'good'` accepted, `'medium'` rejected with `CHECK constraint failed`) proved directly against a scratch database built from the three migrations in order.

```json:metadata
{"files": ["backend/migrations/0003_review_self_grade.sql", "backend/Cargo.toml"], "verifyCommand": "cargo test && SQLX_OFFLINE=true cargo build", "acceptanceCriteria": ["reviews.self_grade TEXT CHECK constrained to the four grades, nullable", "A NULL self_grade is accepted", "'good' is accepted and 'medium' is rejected by the CHECK", "0001 and 0002 are untouched", "rand = 0.8 resolves to the already-locked 0.8.8", "cargo test passes with no regressions"], "modelTier": "mechanical"}
```

---

## Task 3: Spike — can sqlx type `json_each(?)` in a CTE beside a window function?

**Goal:** Prove the candidate query's shape is macro-checkable **before** anything is built on it. This is a gate, not ceremony: every query in Tasks 6–9 depends on it.

**Files:** No permanent files. A throwaway query in a scratch binary or a temporary test, deleted afterwards.

**Steps:**
- [ ] Write a throwaway `sqlx::query!` using the exact shape from the design doc §10 — a `WITH pool AS (SELECT … WHERE deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?)))` CTE, a second CTE with `ROW_NUMBER() OVER (PARTITION BY … ORDER BY …)`, and a `LEFT JOIN` with a bound rank limit
- [ ] Run `cargo sqlx prepare --workspace` from the repo root (needs `export PATH="$HOME/.cargo/bin:$PATH"`)
- [ ] Record which of the three fallbacks, if any, is needed
- [ ] Delete the throwaway

**Acceptance Criteria:**
- [ ] It is known and written into this task whether `json_each(?)` types cleanly inside a CTE
- [ ] If it does not, the chosen fallback is recorded here before Task 6 starts

**Fallbacks, in order of preference:**
1. `WHERE EXISTS (SELECT 1 FROM json_each(?) AS deck_element WHERE CAST(deck_element.value AS INTEGER) = cards.deck_id)`
2. Pass the deck ids as a delimited string and match with `instr` — ugly, keeps macro checking
3. `sqlx::query_as::<_, CandidateRow>` with `.bind()` — **last resort**, loses compile-time checking, and must be flagged in the handover if used

**Verify:** `export PATH="$HOME/.cargo/bin:$PATH" && cargo sqlx prepare --workspace && SQLX_OFFLINE=true cargo build`

```json:metadata
{"files": [], "verifyCommand": "export PATH=\"$HOME/.cargo/bin:$PATH\" && cargo sqlx prepare --workspace && SQLX_OFFLINE=true cargo build", "acceptanceCriteria": ["It is known whether json_each(?) types cleanly inside a CTE alongside a window function", "If it does not, the chosen fallback is recorded in the plan before Task 6 starts", "The throwaway query is deleted and the tree builds clean"], "modelTier": "mechanical"}
```

---

## Task 4: `backend/src/grading.rs` — pure per-kind grading

**Goal:** One module that decides correctness for all three kinds, with no database access, and closes the empty-normalisation hole.

**Files:** Create `backend/src/grading.rs`; modify `backend/src/lib.rs` (`pub mod grading;`)

**Code:**

```rust
use crate::normalise::normalise;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfGrade {
    Again,
    Hard,
    Good,
    Easy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradableChoice {
    pub choice_id: i64,
    pub is_correct: bool,
}

pub fn parse_self_grade(raw: &str) -> Option<SelfGrade> {
    match raw {
        "again" => Some(SelfGrade::Again),
        "hard" => Some(SelfGrade::Hard),
        "good" => Some(SelfGrade::Good),
        "easy" => Some(SelfGrade::Easy),
        _ => None,
    }
}

pub fn self_grade_as_text(self_grade: SelfGrade) -> &'static str {
    match self_grade {
        SelfGrade::Again => "again",
        SelfGrade::Hard => "hard",
        SelfGrade::Good => "good",
        SelfGrade::Easy => "easy",
    }
}

pub fn correctness_of_self_grade(self_grade: SelfGrade) -> bool {
    !matches!(self_grade, SelfGrade::Again)
}

pub fn grade_multiple_choice(
    choices: &[GradableChoice],
    chosen_choice_id: i64,
) -> Option<bool> {
    choices
        .iter()
        .find(|choice| choice.choice_id == chosen_choice_id)
        .map(|choice| choice.is_correct)
}

pub fn grade_short_answer(given: &str, accepted_normalised: &[String]) -> bool {
    let comparison_key = normalise(given);
    if comparison_key.is_empty() {
        return false;
    }
    accepted_normalised
        .iter()
        .any(|candidate| *candidate == comparison_key)
}
```

The empty-key guard is the load-bearing line. An accepted answer of `"---"` passes `cards::validate` (the text is non-blank) yet normalises to `""`, so without it a blank submission would match and every such card would grade every blank answer correct.

**Unit tests** (in-file `#[cfg(test)] mod tests`), each with the mutation that reddens it:

| Test | Mutation |
|---|---|
| `parses_the_four_grades_and_rejects_anything_else` | a missing arm; a case-insensitive match |
| `self_grade_text_round_trips` | an arm returning the wrong string |
| `again_is_incorrect_and_the_other_three_are_correct` | the `matches!` negated, or `Hard` grouped with `Again` |
| `an_unknown_choice_id_grades_to_none_not_false` | `.map` replaced by `.is_some_and(...)`, which would grade a foreign id as wrong instead of rejecting it |
| `the_chosen_choice_decides_correctness` | returning the card's correct choice regardless of the id given |
| `short_answer_matching_is_normalised` — `"K-Means!"` vs accepted `"k means"` | comparing raw text |
| `an_empty_or_punctuation_only_answer_is_incorrect_even_when_an_accepted_key_is_empty` | the empty-key guard removed |
| `short_answer_matching_is_equality_not_substring` — `"k"` must not match `"k means"` | `.contains` instead of `==` |
| `any_accepted_key_matches_not_just_the_first` | checking only `accepted_normalised[0]` |

**Acceptance Criteria:**
- [ ] `grading.rs` imports nothing from `sqlx`, `axum` or `routes`
- [ ] `grade_multiple_choice` returns `None` for a choice id not on the card, so the caller can 422 rather than grade it wrong
- [ ] `grade_short_answer` normalises via the existing `normalise()`, never its own copy
- [ ] An empty normalised answer is incorrect **without consulting** `accepted`
- [ ] Matching is equality on the normalised key, never substring
- [ ] All nine unit tests pass

**Verify:** `cargo test grading && cargo clippy --all-targets -- -D warnings`

```json:metadata
{"files": ["backend/src/grading.rs", "backend/src/lib.rs"], "verifyCommand": "cargo test grading && cargo clippy --all-targets -- -D warnings", "acceptanceCriteria": ["grading.rs imports nothing from sqlx, axum or routes", "grade_multiple_choice returns None for a choice id not on the card", "grade_short_answer normalises via the existing normalise()", "An empty normalised answer is incorrect without consulting accepted", "Matching is equality on the normalised key, never substring", "All nine unit tests pass"], "modelTier": "mechanical"}
```

---

## Task 5: `backend/src/practice.rs` — pure weighting and selection

**Goal:** The spec's weighted sampling as a pure module, with never-seen dominance a derived theorem and the small-deck window rule proved by exhaustive test.

**Files:** Create `backend/src/practice.rs`; modify `backend/src/lib.rs` (`pub mod practice;`)

**Code:**

```rust
use std::collections::HashMap;

pub const BASE_WEIGHT: f64 = 1.0;
pub const MISS_RATE_WEIGHT: f64 = 60.0;
pub const STALENESS_WEIGHT: f64 = 20.0;
pub const MAXIMUM_REVIEWED_WEIGHT: f64 = BASE_WEIGHT + MISS_RATE_WEIGHT + STALENESS_WEIGHT;
pub const NEVER_SEEN_HEADROOM: f64 = 1.0;
pub const NEVER_SEEN_WEIGHT: f64 = MAXIMUM_REVIEWED_WEIGHT + NEVER_SEEN_HEADROOM;
pub const RECENT_REVIEW_LIMIT: i64 = 10;
pub const RECENCY_DECAY: f64 = 0.7;
pub const STALENESS_HALF_LIFE_SECONDS: f64 = 172_800.0;
pub const NO_REPEAT_WINDOW: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutcome {
    Correct,
    Incorrect,
}

#[derive(Debug, Clone)]
pub struct CandidateCard {
    pub card_id: i64,
    pub review_count: i64,
    pub recent_review_outcomes: Vec<ReviewOutcome>,
    pub seconds_since_last_review: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CandidateRow {
    pub card_id: i64,
    pub review_count: i64,
    pub correct: Option<bool>,
    pub recency_rank: Option<i64>,
    pub age_seconds: Option<i64>,
}

pub fn weighted_miss_rate(outcomes: &[ReviewOutcome]) -> f64 {
    if outcomes.is_empty() {
        return 0.0;
    }
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (position, outcome) in outcomes.iter().enumerate() {
        let coefficient = RECENCY_DECAY.powi(position as i32);
        denominator += coefficient;
        if *outcome == ReviewOutcome::Incorrect {
            numerator += coefficient;
        }
    }
    numerator / denominator
}

pub fn staleness_fraction(seconds_since_last_review: Option<i64>) -> f64 {
    match seconds_since_last_review {
        None => 0.0,
        Some(seconds) => {
            let elapsed = seconds.max(0) as f64;
            1.0 - 0.5_f64.powf(elapsed / STALENESS_HALF_LIFE_SECONDS)
        }
    }
}

pub fn weight_for(candidate: &CandidateCard) -> f64 {
    if candidate.review_count == 0 {
        return NEVER_SEEN_WEIGHT;
    }
    BASE_WEIGHT
        + MISS_RATE_WEIGHT * weighted_miss_rate(&candidate.recent_review_outcomes)
        + STALENESS_WEIGHT * staleness_fraction(candidate.seconds_since_last_review)
}

pub fn fold_candidate_rows(rows: Vec<CandidateRow>) -> Vec<CandidateCard> {
    let mut ordered_card_ids: Vec<i64> = Vec::new();
    let mut review_counts: HashMap<i64, i64> = HashMap::new();
    let mut ranked_reviews: HashMap<i64, Vec<(i64, bool, i64)>> = HashMap::new();

    for row in rows {
        if review_counts.insert(row.card_id, row.review_count).is_none() {
            ordered_card_ids.push(row.card_id);
        }
        if let (Some(rank), Some(correct), Some(age)) =
            (row.recency_rank, row.correct, row.age_seconds)
        {
            ranked_reviews
                .entry(row.card_id)
                .or_default()
                .push((rank, correct, age));
        }
    }

    ordered_card_ids
        .into_iter()
        .map(|card_id| {
            let mut reviews = ranked_reviews.remove(&card_id).unwrap_or_default();
            reviews.sort_by_key(|(rank, _, _)| *rank);
            CandidateCard {
                card_id,
                review_count: review_counts.get(&card_id).copied().unwrap_or(0),
                seconds_since_last_review: reviews.first().map(|(_, _, age)| *age),
                recent_review_outcomes: reviews
                    .iter()
                    .map(|(_, correct, _)| {
                        if *correct {
                            ReviewOutcome::Correct
                        } else {
                            ReviewOutcome::Incorrect
                        }
                    })
                    .collect(),
            }
        })
        .collect()
}

pub fn effective_no_repeat_window(eligible_card_count: usize) -> usize {
    NO_REPEAT_WINDOW.min(eligible_card_count.saturating_sub(1))
}

pub fn excluded_card_ids(
    recent_review_card_ids: &[i64],
    eligible_card_count: usize,
) -> Vec<i64> {
    let window = effective_no_repeat_window(eligible_card_count);
    let mut excluded: Vec<i64> = Vec::new();
    for card_id in recent_review_card_ids.iter().take(window) {
        if !excluded.contains(card_id) {
            excluded.push(*card_id);
        }
    }
    excluded
}

pub fn select_card(
    candidates: &[CandidateCard],
    recent_review_card_ids: &[i64],
    roll: f64,
) -> Option<i64> {
    if candidates.is_empty() {
        return None;
    }
    let excluded = excluded_card_ids(recent_review_card_ids, candidates.len());
    let included: Vec<&CandidateCard> = candidates
        .iter()
        .filter(|candidate| !excluded.contains(&candidate.card_id))
        .collect();

    let total: f64 = included.iter().map(|candidate| weight_for(candidate)).sum();
    let target = roll.clamp(0.0, 1.0) * total;
    let mut cumulative = 0.0;
    for candidate in &included {
        cumulative += weight_for(candidate);
        if target < cumulative {
            return Some(candidate.card_id);
        }
    }
    included.last().map(|candidate| candidate.card_id)
}
```

Three deliberate absences, each because the alternative would be untestable code:
- **No `.min(MAXIMUM_REVIEWED_WEIGHT)` clamp** on the weight — the bound is proved algebraically (design doc §6), so a clamp could never fire.
- **No "if `included` is empty, fall back to all candidates"** — `effective_no_repeat_window` caps exclusions at `count − 1` *distinct* ids, so at least one candidate always survives. If it somehow did not, `total` would be `0.0` and `included.last()` would return `None` naturally; no branch is needed.
- **`select_card` takes `roll`** rather than calling `rand` — the handler supplies `rand::random::<f64>()`, which is `[0, 1)`; the trailing `last()` covers `1.0` and float drift.

**Unit tests** (in-file), each with its mutation — these are the bulk of Part 3's suite:

| Test | Mutation |
|---|---|
| `a_never_seen_card_outweighs_the_worst_possible_reviewed_card` — grid over miss patterns × ages {0, 1, 1d, 2d, 10y, `i64::MAX`} | never-seen branch removed; `NEVER_SEEN_HEADROOM = 0.0` |
| `never_seen_weight_exceeds_the_sum_of_every_reviewed_term` | `NEVER_SEEN_WEIGHT` hard-coded so it stops tracking the sum |
| `never_seen_is_decided_by_review_count_not_the_outcome_list` — `review_count = 50`, empty outcomes | the guard changed to `recent_review_outcomes.is_empty()` |
| `recent_misses_outweigh_old_misses` | `RECENCY_DECAY = 1.0`; outcomes read oldest-first |
| `a_full_miss_rate_beats_maximum_staleness` | the two term weights swapped |
| `staleness_breaks_a_miss_rate_tie` | `STALENESS_WEIGHT = 0.0` |
| `staleness_reaches_one_half_at_the_half_life`, monotonic, always `<= 1` | sign flip; wrong half-life; `powf` → `powi` |
| `a_card_just_reviewed_gets_no_staleness_bonus` — `Some(0)` | an additive offset; `None` and `Some(0)` conflated |
| `every_weight_is_positive_and_finite` — empty outcomes, `i64::MAX`, negative seconds | `BASE_WEIGHT = 0.0`; empty-slice NaN; `max(0)` removed |
| `folding_orders_outcomes_by_recency_rank` — rows shuffled within a card | the fold trusting row arrival order |
| `folding_takes_the_age_of_the_most_recent_review_only` — ranks 1..3, ages 10/1000/9999 → `Some(10)` | `MAX(age)`, or last-row-wins |
| `folding_produces_a_never_seen_candidate_from_a_row_with_no_reviews` | the `LEFT JOIN` NULL case mishandled |
| `folding_preserves_the_query_order_of_cards` | iterating the `HashMap` instead of `ordered_card_ids` |
| `selection_is_deterministic_for_a_given_roll` — 100 calls, one answer | hidden RNG or `HashMap` iteration leaking in |
| `a_roll_of_zero_selects_the_first_included_candidate`, `…just_below_one_selects_the_last` | reversed iteration; cumulative off by one |
| `a_roll_of_exactly_one_still_returns_a_candidate` | the `last()` fallback removed |
| `a_roll_outside_zero_to_one_is_clamped` | the clamp removed |
| `selection_frequency_tracks_the_weights` — sweep `roll` over 10 000 steps with ~100:1 weights, ratio within 2% | weights computed then ignored; `total` summed over all candidates instead of the included ones |
| `the_window_never_starves_the_selector` — **exhaustive** over pool sizes 1..=12 × every history prefix drawn from that pool | `count − 1` → `count`; `saturating_sub` → `-`; unconditional 8-card exclusion |
| `the_window_is_eight_for_a_large_pool` | `NO_REPEAT_WINDOW` changed |
| `a_single_card_pool_repeats_that_card` | any unconditional previous-card exclusion |
| `a_three_card_pool_excludes_the_previous_two_only` | `min`/`max` confusion; window not truncated |
| `exclusion_deduplicates_and_ignores_ids_outside_the_pool` | exclusion by slot count rather than id set |
| `an_empty_candidate_list_selects_nothing` | division by a zero total; `unwrap` on empty |

**Acceptance Criteria:**
- [ ] `practice.rs` imports nothing from `sqlx`, `axum`, `rand` or `routes` — only `std`
- [ ] `NEVER_SEEN_WEIGHT` is *derived* from `MAXIMUM_REVIEWED_WEIGHT`, not a literal
- [ ] Never-seen is decided by `review_count == 0`, never by an empty outcome list
- [ ] `effective_no_repeat_window` never exceeds `count − 1`, and `saturating_sub` handles a count of 0
- [ ] Selection is deterministic given `(candidates, history, roll)`
- [ ] There is no unreachable clamp and no unreachable fallback
- [ ] All ~24 unit tests pass, and the exhaustive window test covers pool sizes 1..=12

**Verify:** `cargo test practice && cargo clippy --all-targets -- -D warnings`

```json:metadata
{"files": ["backend/src/practice.rs", "backend/src/lib.rs"], "verifyCommand": "cargo test practice && cargo clippy --all-targets -- -D warnings", "acceptanceCriteria": ["practice.rs imports nothing from sqlx, axum, rand or routes", "NEVER_SEEN_WEIGHT is derived from MAXIMUM_REVIEWED_WEIGHT, not a literal", "Never-seen is decided by review_count == 0, never by an empty outcome list", "effective_no_repeat_window never exceeds count minus 1 and handles a count of 0", "Selection is deterministic given candidates, history and roll", "There is no unreachable clamp and no unreachable fallback", "All unit tests pass including an exhaustive window test over pool sizes 1 to 12"], "modelTier": "standard"}
```

---

## Task 6: `POST /api/sessions`

**Goal:** Create a practice session, expanding a module to its decks, and refuse at creation rather than producing an empty runner.

**Files:** Create `backend/src/routes/sessions.rs`; modify `backend/src/routes/mod.rs`; create `backend/tests/sessions.rs`

**Request:** `{mode, deck_ids?, module_id?, target_count?}`, `#[serde(deny_unknown_fields)]`, exactly one of `deck_ids` / `module_id`.

**Validation, in this order** — all 422 with the envelope's `fields`:

| Condition | Field | Message |
|---|---|---|
| `mode` not one of the three schema values | `mode` | mode must be practice, mock or sm2 |
| `mode` is `mock` or `sm2` | `mode` | Only practice mode is available yet |
| both `deck_ids` and `module_id` | `deck_ids` | Choose either decks or a module, not both |
| neither | `deck_ids` | Choose at least one deck or a module |
| `deck_ids` present and empty | `deck_ids` | Choose at least one deck |
| a deck id not found | `deck_ids` | That deck does not exist |
| `module_id` not found | `module_id` | That module does not exist |
| `target_count` present and non-null | `target_count` | Practice sessions have no target count |
| resolved pool has zero non-archived cards | whichever was supplied | Those decks have no cards to practise |

`deck_ids` are de-duplicated and sorted before `serde_json::to_string`, so the stored JSON is canonical and safe to feed straight to `json_each`. No transaction — the checks are reads and the write is one statement; a deck deleted in between yields a session that `/next` reports as 409.

**Response 201:** `{id, mode, deck_ids, target_count, started_at, ended_at, pool_count, answered_count}`

**Integration tests:**
- [ ] `creates_a_practice_session_from_deck_ids` — 201, `deck_ids` canonical sorted and deduped, `pool_count` correct
- [ ] `expands_a_module_into_its_decks`
- [ ] `rejects_mock_and_sm2_modes_for_now` / `rejects_an_unknown_mode_value` — two distinct messages, checked in the right order
- [ ] `rejects_both_deck_ids_and_module_id` / `rejects_neither` / `rejects_an_empty_deck_id_array`
- [ ] `rejects_an_unknown_deck_id` / `rejects_an_unknown_module_id` — correct field each
- [ ] `rejects_a_target_count_on_a_practice_session`
- [ ] **`refuses_to_create_a_session_with_no_eligible_cards`** — a deck whose only card is archived → 422 **and** `SELECT COUNT(*) FROM sessions` is still 0. Mutation: drop the `archived = 0` filter.

**Verify:** `cargo test --test sessions`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/src/routes/mod.rs", "backend/tests/sessions.rs"], "verifyCommand": "cargo test --test sessions", "acceptanceCriteria": ["POST /api/sessions creates a practice session and returns 201 with canonical sorted deduped deck_ids", "module_id expands to its decks at creation and is stored denormalised", "mock and sm2 are rejected with a different message from an unknown mode, checked in that order", "Exactly one of deck_ids or module_id is required", "An unknown deck or module id is 422 on the right field", "target_count on a practice session is rejected, not ignored", "A deck whose only card is archived is refused at creation and writes no sessions row"], "modelTier": "standard"}
```

---

## Task 7: `GET /next` and `POST /reveal` — the leakage pair

**Goal:** Serve a weighted-sampled card that structurally cannot carry an answer key, and give flashcards their own reveal.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/sessions.rs`

**Response types — these are the whole point of the task:**

```rust
#[derive(Serialize)]
pub struct NextChoiceResponse {
    pub id: i64,
    pub text_md: String,
}

#[derive(Serialize)]
pub struct NextCardResponse {
    pub id: i64,
    pub kind: String,
    pub prompt_md: String,
    pub image_path: Option<String>,
    pub choices: Vec<NextChoiceResponse>,
}

#[derive(Serialize)]
pub struct NextResponse {
    pub card: NextCardResponse,
    pub pool_count: i64,
    pub answered_count: i64,
}
```

**Three rules that keep leakage structurally impossible:**
1. These types are distinct from `cards::CardResponse` / `cards::ChoiceResponse`, which *do* carry `is_correct`, `answer_md`, `explanation_md` and `accepted`. **`sessions.rs` must not import from `routes::cards`, and must not use `#[serde(flatten)]` on this path.**
2. The serve projections select only `SELECT id, kind, prompt_md, image_path FROM cards WHERE id = ?` and `SELECT id, text_md FROM choices WHERE card_id = ? ORDER BY position`. The key columns never enter the process. Shuffling needs no knowledge of which choice is correct; grading resolves `is_correct` by id at answer time.
3. `choices` is a `Vec`, not an `Option`, so it serialises to `[]` for the other two kinds — no kind-conditional branch that could later grow an answer field.

**Handler flow:** load the session (404 `session`; 409 "This session has ended" when `ended_at` is set) → the candidate query (design doc §10) → the window query → `fold_candidate_rows` → `select_card(&candidates, &window, rand::random::<f64>())` → `None` means the pool emptied mid-session → 409 "This session has no cards left to practise", **never a 500**.

Shuffle per serve in the handler: `choices.shuffle(&mut rand::thread_rng())`. Shuffling is a **leakage control, not cosmetics** — served in `position` order, an author's habit of typing the correct option first *is* the key.

**`POST /api/sessions/{id}/reveal`** — request `{card_id}`, response `{card_id, answer_md, explanation_md}`. Flashcard only; any other kind is 409 "Only a flashcard can be revealed". Card must be in the pool (422 `card_id` "That card is not in this session"). Ended session 409. **Writes nothing.**

**Integration tests:**
- [ ] **`next_never_returns_answer_data_for_any_kind`** — one deck with all three kinds, 30 serves; assert the `card` object's key set is **exactly** `{id, kind, prompt_md, image_path, choices}`, each choice's exactly `{id, text_md}`, and the serialised body contains none of `is_correct`, `answer_md`, `explanation_md`, `accepted`, `expected`, nor any actual answer text. Mutation: add any answer field to either struct, or reuse `cards::ChoiceResponse`.
- [ ] **`next_shuffles_the_choices`** — a 4-choice card, 30 serves, ≥2 distinct id orders **and** a constant id set. False-failure probability `(1/24)^29`.
- [ ] `next_serves_an_unseen_card_before_a_known_one` — 1 never-seen against 5 with 3 correct reviews each; the unseen card appears within 20 serves
- [ ] **`next_does_not_repeat_a_card_inside_the_window`** — 12-card deck, 9 answered in a row, no card twice
- [ ] **`the_no_repeat_window_survives_a_reload`** — answer 3, then 15 bare `/next` calls; none of the 3 appears. Mutation: derive the window from in-process memory rather than `reviews`.
- [ ] `a_three_card_deck_still_serves_a_card` — 40 consecutive serve+answer cycles, all 200
- [ ] `a_one_card_deck_serves_the_same_card_repeatedly`
- [ ] `next_conflicts_when_every_pool_card_is_archived_mid_session` — 409, not 500
- [ ] `next_on_a_finished_session_is_409` / `next_on_an_unknown_session_is_404`
- [ ] `next_reports_the_progress_counts_from_reviews`
- [ ] **`refuses_to_reveal_a_graded_card`** — `mc_single` and `short_answer` → 409, and the body leaks neither `is_correct` nor accepted text. The gate that stops reveal becoming a universal key oracle.
- [ ] `revealing_a_flashcard_returns_its_answer` / `refuses_to_reveal_a_card_outside_the_session` / `revealing_writes_nothing`

**Verify:** `cargo test --test sessions`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/sessions.rs"], "verifyCommand": "cargo test --test sessions", "acceptanceCriteria": ["NextCardResponse and NextChoiceResponse are distinct types from cards::CardResponse and cards::ChoiceResponse, and sessions.rs does not import from routes::cards", "The serve projections select only id, kind, prompt_md, image_path and id, text_md", "choices is a Vec so it serialises to [] for non-multiple-choice kinds", "A served card's key set is exactly id, kind, prompt_md, image_path, choices for all three kinds", "Choices are shuffled per serve with a constant id set", "A never-seen card is favoured over well-known ones", "No card repeats inside the no-repeat window, and the window survives a client reload", "A three-card and a one-card deck both keep serving without starving", "An emptied pool is 409 not 500; ended session 409; unknown session 404", "POST /reveal returns a flashcard answer, 409s for the two graded kinds, and writes nothing"], "modelTier": "standard"}
```

---

## Task 8: `POST /api/sessions/{id}/answer`

**Goal:** Grade server-side per kind and append exactly one `reviews` row, without touching `schedule`.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/sessions.rs`

**Request:** `{card_id, given?, choice_id?, self_grade?, ms?}`, `deny_unknown_fields`.

Card lookup and pool membership in one query — `WHERE id = ? AND archived = 0 AND deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))`. A miss is **422** field `card_id` "That card is not in this session" (422 not 404, because it is a body field).

| Kind | Required | Forbidden | Grading |
|---|---|---|---|
| `mc_single` | `choice_id` | `given`, `self_grade` | `grade_multiple_choice`; `None` → 422 `choice_id` "That option is not on this card" |
| `short_answer` | `given` | `choice_id`, `self_grade` | `grade_short_answer` against the card's `accepted.normalised` |
| `flashcard` | `self_grade` | `given`, `choice_id` | `correctness_of_self_grade`; unparseable → 422 `self_grade` "self_grade must be again, hard, good or easy" |

Negative `ms` → 422 `ms` "ms must not be negative".

**`reviews.given` storage policy** — not obvious, so it is spelled out:
- `short_answer` → the raw submitted text, trimmed. Normalisation is for comparison only; the override needs the student's actual wording.
- `mc_single` → the chosen choice's `text_md` as a **snapshot**. An id would dangle once the card's choices are edited, and a history view needs words.
- `flashcard` → `NULL`, with `self_grade` carrying the signal.

One `INSERT INTO reviews (card_id, session_id, given, correct, self_grade, ms) … RETURNING id`. No transaction: one statement, one table, and **`schedule` is not touched**.

**Response 200:** `{review_id, correct, expected, explanation_md, can_override}`. `expected` is a `Vec<String>` — one shape for all three kinds: the correct choice's text, every accepted wording primary-first, or `answer_md`. `can_override` is `kind == "short_answer" && !correct`, computed server-side so the button's precondition is testable in one place.

**Integration tests:**
- [ ] `grades_a_correct_multiple_choice_answer` / `…an_incorrect_one_and_returns_the_expected_choice`
- [ ] **`grades_a_short_answer_by_normalised_match`** — `"K-Means!"` against accepted `"k means"` → correct. Mutation: compare raw text.
- [ ] **`an_empty_short_answer_is_incorrect_even_when_an_accepted_row_normalises_to_empty`** — accepted `"---"`, given `"  "`. Mutation: drop the empty-key guard.
- [ ] `a_flashcard_self_grade_of_again_is_incorrect` / `hard_good_and_easy_are_correct`, with `reviews.self_grade` persisted
- [ ] `stores_the_submitted_wording_verbatim_for_a_short_answer` — mutation: store the normalised form, which would break the override's accepted text
- [ ] `stores_the_chosen_choice_text_for_a_multiple_choice_answer`
- [ ] **`rejects_a_card_from_another_deck`** (422 `card_id`) — the core trust-boundary guard
- [ ] `rejects_an_archived_card` / `rejects_a_choice_id_from_another_card` / `rejects_the_wrong_answer_field_for_each_kind` (3 cases, exact field names) / `rejects_a_negative_ms`
- [ ] **`answering_does_not_touch_the_schedule_table`** — assert `due_at`, `reps`, `lapses`, `interval_days`, `ease` all identical before and after. Locks the spec's "practice ignores schedule". Mutation: add a schedule update.
- [ ] `answering_writes_exactly_one_review_row`
- [ ] `can_override_is_true_only_for_an_incorrect_short_answer` — 4 cases
- [ ] `answer_on_a_finished_session_is_409` / `on_an_unknown_session_is_404`

**Verify:** `cargo test --test sessions`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/sessions.rs"], "verifyCommand": "cargo test --test sessions", "acceptanceCriteria": ["Card lookup enforces pool membership and archived = 0, returning 422 on field card_id", "Each kind requires its own answer field and rejects the other two", "Short answer grading goes through normalise and rejects an empty normalised answer without consulting accepted", "A flashcard self_grade is persisted and correct is derived with again mapping to 0", "reviews.given stores raw trimmed text for short_answer, the chosen choice text for mc_single, and NULL for flashcard", "Exactly one reviews row is written per answer", "The schedule table is completely untouched by answering", "can_override is true only for an incorrect short answer", "Ended session is 409 and unknown session is 404"], "modelTier": "standard"}
```

---

## Task 9: `POST /api/reviews/{id}/override` and `POST /api/sessions/{id}/finish`

**Goal:** The only write in the app that mutates a `reviews` row, and an idempotent session end.

**Files:** Modify `backend/src/routes/sessions.rs`, `backend/tests/sessions.rs`

### Override

No request body — the accepted text comes from `reviews.given`, so there is nothing to send. Follows the bodiless `POST /api/cards/{id}/archive` style.

| Condition | Status | Message |
|---|---|---|
| no row | 404 | review not found |
| kind is not `short_answer` | 409 | Only a short-answer review can be overridden |
| `correct` already true | 409 | That answer was already marked correct |
| `given` NULL, or normalises to empty | 409 | There is no answer to accept |

`mc_single` is excluded because its key is unambiguous — "I was right" about a radio button is a card-authoring bug. `flashcard` is excluded because the student is already the grader. **No separate already-overridden branch**: `overridden = 1` always implies `correct = 1`, so the already-correct check subsumes it, and an unreachable branch would be untestable.

One transaction, two statements, the insert carrying its own duplicate guard:

```sql
INSERT INTO accepted (card_id, text, normalised, is_primary)
SELECT ?, ?, ?, 0
 WHERE NOT EXISTS (SELECT 1 FROM accepted WHERE card_id = ? AND normalised = ?);

UPDATE reviews SET correct = 1, overridden = 1 WHERE id = ?;
```

`accepted_added` is `rows_affected() == 1`. `is_primary` is **always 0** — the primary wording belongs to the author, and a second primary would break the one-primary invariant that `cards::validate` enforces and no database constraint does. The `UPDATE` targets **only** the addressed row: other reviews of the same card keep their verdict, because a bulk flip would rewrite history. Overriding works whether or not the session has ended.

**Response 200:** `{review_id, correct: true, overridden: true, accepted_added, expected}`

### Finish

No body. 404 first if the session does not exist, then:

```sql
UPDATE sessions SET ended_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
 WHERE id = ? AND ended_at IS NULL
```

`AND ended_at IS NULL` makes finish **idempotent with 200**, preserving the original `ended_at`. Not 409 — finishing is a terminal-state assertion, and a reload that double-posts must not show an error.

**Response 200:** `{id, mode, started_at, ended_at, answered_count, correct_count, overridden_count, distinct_card_count, accuracy, total_ms}`. **`accuracy` is `null` when `answered_count == 0`**, not `0.0`, which would claim you got everything wrong. A per-card weakest-cards breakdown is Part 6's job, not this summary's.

**Integration tests:**
- [ ] `overriding_flips_the_targeted_review_and_adds_an_accepted_row` — `correct = 1`, `overridden = 1`, `accepted` +1, new row `is_primary = 0`, `normalised == normalise(given)`
- [ ] **`overriding_does_not_add_a_duplicate_normalised_accepted_row`** — two reviews whose `given` differ only by punctuation/case; `accepted` +1 total and the second response has `accepted_added: false`. Mutation: drop the `NOT EXISTS` guard. This is the regression test for the known issue.
- [ ] **`overriding_leaves_other_reviews_of_the_same_card_alone`** — mutation: widen `WHERE id = ?` to `card_id`
- [ ] **`the_overridden_wording_is_accepted_on_the_next_answer`** — override, then submit the same wording → correct. The end-to-end proof.
- [ ] `overriding_never_creates_a_second_primary_accepted_row` — `COUNT(is_primary = 1) == 1`
- [ ] `overriding_inserts_no_new_review_row`
- [ ] `refuses_to_override_a_multiple_choice_review` / `a_flashcard_review` / `an_already_correct_review` / `a_review_with_no_usable_given` (flashcard NULL, and a `"---"`-only wording — no `accepted` row written)
- [ ] `overriding_twice_is_409_and_adds_nothing`
- [ ] `overriding_after_the_session_finished_still_works` — 200; mutation: a spurious session-ended check
- [ ] `overriding_an_unknown_review_is_404` with `error: "not_found"`
- [ ] `finishing_sets_ended_at_and_returns_the_summary`
- [ ] **`finishing_twice_returns_the_same_ended_at`** — 200 both times, identical timestamp. Mutation: drop `AND ended_at IS NULL`.
- [ ] `accuracy_is_null_for_a_session_with_no_answers`
- [ ] `the_summary_counts_overrides_as_correct` — answer wrong, override, finish → `correct_count = 1`, `overridden_count = 1`
- [ ] `finishing_an_unknown_session_is_404`

**Verify:** `cargo test --test sessions && cargo clippy --all-targets -- -D warnings`

```json:metadata
{"files": ["backend/src/routes/sessions.rs", "backend/tests/sessions.rs"], "verifyCommand": "cargo test --test sessions && cargo clippy --all-targets -- -D warnings", "acceptanceCriteria": ["Override flips correct and overridden on the addressed row only and adds one accepted row with is_primary = 0", "The accepted insert uses WHERE NOT EXISTS so a duplicate normalised key adds nothing and reports accepted_added false", "The overridden wording grades correct on a subsequent answer", "Override refuses multiple choice, flashcard, already-correct, and empty-given reviews with 409", "Override works after the session has finished and inserts no new review row", "Finish is idempotent with 200 and preserves the original ended_at", "accuracy is null for a session with no answers, not 0.0", "The summary counts an overridden review as correct"], "modelTier": "standard"}
```

---

## Task 10: `frontend/src/lib/api.ts` — session types and calls

**Goal:** Typed access to the six endpoints, with no `any`, over the existing `request()` wrapper.

**Files:** Modify `frontend/src/lib/api.ts`

New types: `SessionMode`, `SelfGrade`, `Session`, `NextChoice`, `NextCard`, `NextResponse`, `RevealedAnswer`, `AnswerResult`, `OverrideResult`, `SessionSummary`.

New calls on the existing `api` object: `createSession`, `nextCard`, `revealCard`, `submitAnswer`, `overrideReview`, `finishSession`.

Note `NextCard` deliberately has **no** `answer_md`, `explanation_md`, `is_correct` or `accepted` field — mirroring the backend's structural guarantee on the client side, so a component cannot reach for a key that is not there.

**Acceptance Criteria:**
- [ ] Every new type is concrete — no `any`, per `CLAUDE.md` rule 3
- [ ] `NextCard` and `NextChoice` carry no answer or correctness fields
- [ ] `submitAnswer` accepts exactly one of `given` / `choice_id` / `self_grade` in its parameter type
- [ ] All six calls go through the existing private `request<Result>()`, so `ApiError` and `.byField()` work unchanged
- [ ] `pnpm exec tsc --noEmit` is clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit`

```json:metadata
{"files": ["frontend/src/lib/api.ts"], "verifyCommand": "cd frontend && pnpm exec tsc --noEmit", "acceptanceCriteria": ["Every new type is concrete with no use of any", "NextCard and NextChoice carry no answer or correctness fields", "submitAnswer's parameter type admits exactly one of given, choice_id or self_grade", "All six calls go through the existing request wrapper so ApiError and byField work unchanged", "pnpm exec tsc --noEmit is clean"], "modelTier": "mechanical"}
```

---

## Task 11: `/study` — the picker

**Goal:** Pick a mode and decks, see how many cards that is, and start a session.

**Files:** Create `frontend/src/pages/StudyPage.tsx`; modify `frontend/src/App.tsx`; add the shadcn `checkbox` component

**Steps:**
- [ ] `pnpm dlx shadcn@latest add checkbox` — not currently installed (present: badge, button, card, dialog, input, label, radio-group, select, sonner, switch, textarea). It lands in the vendored `components/ui/`, which `CLAUDE.md` exempts from both the naming and no-`any` rules.
- [ ] Mode: a `RadioGroup` with Practice selected; Mock test and SM-2 rendered **disabled** with "arrives in part 5" / "part 7", so the screen does not need rebuilding later
- [ ] Decks: a multi-select list grouped by module, reusing the grouping shape `DecksPage` already builds
- [ ] A live count of selected cards, summing `Deck.card_count` (which the API already computes excluding archived), so **Start** disables at zero and the student never meets the server's zero-eligible error in the normal case
- [ ] Start → `createSession` → `navigate('/session/' + id)`
- [ ] Errors follow the established pattern: `ApiError` → `byField()` inline, empty `fields` → `toast.error(error.message)`, otherwise `toast.error('Could not reach the server')`

**Acceptance Criteria:**
- [ ] `/study` renders the real page, not `StubPage`
- [ ] Decks are grouped by module, with unparented decks in their own group
- [ ] The selected-card count updates live and Start is disabled at zero selected
- [ ] Mock test and SM-2 are visible but disabled, labelled with the part they arrive in
- [ ] A server-side validation error renders inline on the right field
- [ ] `pnpm exec tsc --noEmit` and `pnpm build` are clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build`

```json:metadata
{"files": ["frontend/src/pages/StudyPage.tsx", "frontend/src/App.tsx", "frontend/package.json"], "verifyCommand": "cd frontend && pnpm exec tsc --noEmit && pnpm build", "acceptanceCriteria": ["/study renders the real page rather than StubPage", "Decks are grouped by module with unparented decks in their own group", "The selected-card count updates live and Start is disabled at zero", "Mock test and SM-2 are visible but disabled and labelled with their part", "A server-side validation error renders inline on the right field", "tsc --noEmit and pnpm build are clean"], "modelTier": "standard"}
```

---

## Task 12: `/session/:id` — the runner

**Goal:** A keyboard-first practice loop: serve, answer, verdict, advance — all three kinds, no mouse required.

**Files:** Create `frontend/src/pages/SessionPage.tsx` and components under `frontend/src/components/session/`; modify `frontend/src/App.tsx`

**Reuse, do not rebuild:** `<Markdown>` for every prompt, choice, answer and explanation — it is the app's only rendering path and the runner is its third consumer. `<CardImage>` for the prompt image and its lightbox. The abort-controller-plus-`inFlight`-ref fetch guard from `DeckPage`. The container-level `onKeyDown` convention from `CardEditorPage` — there are no `window` listeners anywhere in this app and this task must not add the first.

**Per kind:**
- `mc_single` — shuffled choices lettered A/B/C…, number keys `1`–`9` select, `Enter` submits. The spec says `1`–`4`; the schema caps nothing, so `1`–`9` with letters shown covers real decks without inventing a limit.
- `short_answer` — autofocused `Input`, `Enter` submits. **Number keys must not be intercepted while focus is in the input.**
- `flashcard` — prompt, then "Show answer" (`Space`/`Enter`) which calls `POST /reveal` (the answer is deliberately not on the serve response), then Again / Hard / Good / Easy on keys `1`–`4`.

**Verdict:** correct/incorrect banner, `expected` and `explanation_md` through `<Markdown>`. When `can_override`, an **"I was right"** button calls `overrideReview` and flips the banner in place. `Enter` advances.

**Other:** `ms` from a `useRef` stamped when the card is served. Header shows answered/correct from `NextResponse`, so a reload does not reset it. "End session" → `finishSession` → a summary panel with links back to `/study` and `/decks`. The runner ships **unthemed** — the sparkle burst and streak flourish are Part 4.

**Acceptance Criteria:**
- [ ] All three kinds render and submit, each with its own answer control
- [ ] `1`–`9` select a multiple-choice option; `Enter` submits; `Enter` again advances
- [ ] Number keys do **not** hijack typing in the short-answer input
- [ ] A flashcard reveals via `POST /reveal` before its four grade buttons appear
- [ ] The verdict shows `expected` and the explanation when present
- [ ] "I was right" appears only when the server says `can_override`, and flips the banner in place
- [ ] The header's answered/correct counts survive a browser reload
- [ ] `ms` is sent and is never negative
- [ ] End session shows a summary whose numbers match the run
- [ ] No `window` event listener is added; keyboard handling is container-level
- [ ] `pnpm exec tsc --noEmit` and `pnpm build` are clean

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build`, then the browser walkthrough in Task 13.

```json:metadata
{"files": ["frontend/src/pages/SessionPage.tsx", "frontend/src/components/session/", "frontend/src/App.tsx"], "verifyCommand": "cd frontend && pnpm exec tsc --noEmit && pnpm build", "acceptanceCriteria": ["All three kinds render and submit with their own answer control", "1 to 9 select a multiple-choice option, Enter submits, Enter again advances", "Number keys do not hijack typing in the short-answer input", "A flashcard reveals via POST /reveal before its four grade buttons appear", "The verdict shows expected and the explanation when present", "I was right appears only when the server says can_override and flips the banner in place", "The header's answered and correct counts survive a browser reload", "ms is sent and is never negative", "End session shows a summary matching the run", "No window event listener is added; keyboard handling is container-level", "tsc --noEmit and pnpm build are clean"], "modelTier": "standard"}
```

---

## Task 13: Full gate, browser walkthrough, handover

**Goal:** The whole verification gate passes, the runner is actually driven in a browser, and the record is updated.

**Files:** Modify `docs/HANDOVER.md`

**Automated gate**, from the repo root:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo sqlx prepare --workspace
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

`--all-targets` matters — plain `cargo clippy` skips test targets, which once left ~370 lines of test code unlinted for all of Part 1.

**Browser walkthrough.** The Chrome extension is connected, so unlike Parts 1–2c this can genuinely be driven rather than deferred.

```bash
cargo run                    # API on http://127.0.0.1:3000
cd frontend && pnpm dev      # UI  on http://localhost:5273   (5273, not 5173)
```

- [ ] `/study`: select two decks, the card count updates, Start
- [ ] A `mc_single` card: press `2`, press `Enter` — verdict shows, `Enter` advances. Check the request body sends `choice_id`, not a stringified id in `given`
- [ ] Reload mid-session — the header's answered/correct count survives
- [ ] A `short_answer` card: type a near-miss (`k means` for `k-means`) — grades **correct**, proving `normalise()` is the shared key
- [ ] A `short_answer` card: type a genuinely different correct wording, get marked wrong, press "I was right" — the banner flips, and the same wording auto-grades correct when the card returns
- [ ] A `flashcard`: "Show answer" from the keyboard, grade Good
- [ ] ~12 answers in a small deck with no back-to-back repeat
- [ ] End session — the summary numbers match the run
- [ ] **DevTools Network:** no `/next` response, for any of the three kinds, contains `is_correct`, `answer_md`, `accepted` or `explanation_md`. The flashcard answer arrives only from `/reveal`
- [ ] The runner at 375px width

The leakage point is also asserted in the integration tests, which is where it really belongs; the browser check is a backstop against a serialisation surprise.

**Handover updates:**
- [ ] Move Part 3 from "Next up" into "Where things stand"
- [ ] Record the six endpoints, migration `0003`, `rand`, and `CLAUDE.md` rule 3
- [ ] Record whether the `json_each` spike needed a fallback, and which
- [ ] Note that the runner is **unthemed** pending Part 4
- [ ] Carry forward Part 2c's still-outstanding walkthrough and its five known defects
- [ ] Add the two Part 5 follow-ups from the design doc: `/next` needs a stable serve under `target_count`, and a flashcard in a no-feedback mock test is an open question

```json:metadata
{"files": ["docs/HANDOVER.md"], "verifyCommand": "export PATH=\"$HOME/.cargo/bin:$PATH\" && cargo sqlx prepare --workspace && cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build && cd frontend && pnpm exec tsc --noEmit && pnpm build", "acceptanceCriteria": ["cargo test passes with all new tests included", "cargo clippy --all-targets -- -D warnings is clean", "SQLX_OFFLINE=true cargo build succeeds and .sqlx is regenerated and committed", "tsc --noEmit and pnpm build are clean", "The browser walkthrough passes, including the no-leakage Network check for all three kinds", "docs/HANDOVER.md records the six endpoints, migration 0003, rand, CLAUDE.md rule 3, the spike outcome, and the Part 5 follow-ups"], "modelTier": "standard"}
```

---

## Notes for whoever executes this

**Run one implementer at a time.** Part 2a dispatched two agents concurrently on disjoint file lists; git's index is not per-file, and one agent's `git add`/`commit` swept the other's staged work into a mislabelled commit. Read-only reviewers can run alongside an implementer; two writers cannot.

**Every new sqlx query needs `cargo sqlx prepare --workspace`** from the repo root, or the build fails against the committed `.sqlx/` cache. Tasks 6–9 each add queries.

**Give reviewers both sides of any seam.** A per-task review structurally cannot see an API-to-client mismatch, so Tasks 10–12 should be reviewed against Tasks 6–9's contracts, not in isolation.

**Demand mutation evidence.** Every test in this plan names the mutation that should redden it. "I removed X and the test went red" is only evidence when X was the sole change — and a test that cannot fail is not a test.

**Code rules are not optional** (`CLAUDE.md`): no comments in code, no abbreviated identifiers, and now no `any` in TypeScript. SQL migrations are the one place this repo carries explanatory comments, since DDL cannot express *why* a column is nullable.
