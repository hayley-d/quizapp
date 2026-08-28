# Part 7 — SM-2 scheduling: design decisions

**Date:** 2026-08-28
**Status:** approved, pre-implementation
**Build step:** 7 of 8

Part 7 is build step 7: SM-2 scheduling. Not in scope: the bundle, LAN binding and phone
layout pass (step 8). Part 7 is the first code in this repository to read or write
`schedule`, and the first consumer of `reviews.self_grade`.

## 1. Why this part needs no migration

Every earlier part was sequenced to leave this one ready, and the groundwork is unusually
complete:

- **`schedule` has existed since Part 1** (`backend/migrations/0001_init.sql:81-89`), one row
  per card, written at card creation (`backend/src/routes/cards.rs:488-494`). That INSERT is
  the *only* reference to the table anywhere in `backend/src` — the table is otherwise dead.
  The master spec's "Schedule exists from day one" section says why: it avoids a migration
  over cards that have already been hand-written.
- **`sessions.mode` already permits `'sm2'`** in the DDL, and `MODES` in
  `backend/src/routes/sessions.rs:20` already lists it. One `else if` at `sessions.rs:54-56`
  refuses it with "Only practice and mock modes are available yet". That branch is the gate
  this part removes.
- **`reviews.self_grade`** (migration `0003`) exists precisely to be SM-2's quality input.
  Part 3 added it early rather than let the four levels be discarded as a bare bit at the
  moment they were made.
- **The frontend slot exists.** `DeckPage.tsx:57-63` renders a disabled "Spaced repetition"
  tile noted "Arrives in part 7.", and `SESSION_ROUTE_BY_MODE.sm2` already points at
  `/session`. Part 5's mode-mismatch redirect was written to give `sm2` its slot.

So Part 7 adds **no migration, no new table and no new column**.

## 2. The quality mapping is not ours to invent

The master spec fixes it:

| Outcome | Quality |
| --- | --- |
| Auto-graded correct | 4 |
| Correct via override | 4 |
| Auto-graded wrong | 2 |
| Flashcard: again / hard / good / easy | 1 / 3 / 4 / 5 |

**There is no `overridden` parameter**, even though the spec's table lists "correct via
override" as its own row. The override endpoint's whole effect is to set `reviews.correct` to
1, so by the time the replay reads the row it is indistinguishable from an answer that was
right first time — and it *should* be, because that is what the override asserts. Passing
`overridden` separately would be a second way to say the same thing, free to disagree with the
column. The row in the table is a statement about the outcome, not a demand for an argument.

`quality_for(correct, self_grade)` is **total**: a `self_grade` of `None` falls
to the auto-graded branch. That totality is the resolution of the constraint Part 5 recorded
— "Part 7 must map mock flashcard reviews through `correct`, not through the grade table" —
rather than a special case bolted on for it. In practice an SM-2 flashcard always carries a
self-grade, because SM-2 uses the practice reveal-and-self-grade path, but the function does
not depend on that being true.

## 3. The pure core: `backend/src/scheduler.rs`

A new module with no database access, alongside `practice.rs`, `mock.rs` and `grading.rs`.
The master spec's Testable core section names SM-2 as belonging here.

```rust
pub const INITIAL_EASE: f64 = 2.5;
pub const MINIMUM_EASE: f64 = 1.3;
pub const FIRST_INTERVAL_DAYS: f64 = 1.0;
pub const SECOND_INTERVAL_DAYS: f64 = 6.0;
pub const PASSING_QUALITY: u8 = 3;

pub struct ScheduleState { pub interval_days: f64, pub ease: f64, pub reps: i64, pub lapses: i64 }

pub fn initial_state() -> ScheduleState;
pub fn quality_for(correct: bool, self_grade: Option<SelfGrade>) -> u8;
pub fn apply(state: &ScheduleState, quality: u8) -> ScheduleState;
pub fn replay(qualities: &[u8]) -> ScheduleState;
```

`initial_state` mirrors the DDL defaults (`0.0`, `2.5`, `0`, `0`) rather than restating them
independently, so a card that has never been reviewed and a card replayed from an empty
history agree.

`apply` is standard SM-2:

- **Ease:** `ease + (0.1 - (5 - quality) * (0.08 + (5 - quality) * 0.02))`, clamped below at
  `MINIMUM_EASE`.
- **On a lapse (`quality < PASSING_QUALITY`):** `reps = 0`,
  `interval_days = FIRST_INTERVAL_DAYS`, `lapses += 1`, and **the ease factor is left
  unchanged**.
- **Otherwise:** `reps += 1`; interval is `FIRST_INTERVAL_DAYS` at `reps == 1`,
  `SECOND_INTERVAL_DAYS` at `reps == 2`, and `round(previous_interval * new_ease)` after that.

### 3a. The lapse leaves the ease factor alone, deliberately

This is a real fork in the algorithm, and a majority of the SM-2 implementations in
circulation take the other branch. The original SuperMemo-2 description is explicit: if the
quality response was lower than 3, *start repetitions for the item from the beginning without
changing the E-Factor*. The reasoning is that the E-Factor is a property of the item's
intrinsic difficulty, and a single failure is already punished by the interval reset — taking
the ease down as well double-counts one bad night and drives easy-but-forgotten cards toward
the 1.3 floor they do not belong at.

Recorded here because it is invisible in the code (it is an *absence* — a line not written)
and would be "fixed" by a later reader who knows the more common variant. It gets its own
unit test, and that test is on the mutation list.

### 3b. `apply` and `replay` must agree

`replay` is a fold of `apply` from `initial_state`, and the property that applying reviews one
at a time equals replaying the whole list gets a test. If the two ever disagree, the override
path silently contradicts the answer path, and the disagreement would only ever surface as a
schedule that quietly changed after an unrelated action.

## 4. Serving: due-ordered, and reload-stable for free

An SM-2 session serves, from the session's decks: non-archived cards whose schedule is due,
each exactly once, ordered by **`due_at` ascending, then `card_id`**. Most overdue first; the
id tiebreak makes the order total, the same reasoning as `mock.rs`'s `(hash, card_id)` tuple.

The next card is the first card in that order with **no `reviews` row for this session**, so a
reload re-serves the same card and "session state lives only in `reviews`" stays exactly true.

Unlike mock, this needs **no hash and no `mock.rs`-style trick**. Mock needed rank-by-hash
because a shuffle is a function of *the list*, so archiving one card mid-test would reorder
the rest. `due_at` is already a per-card property, so ordering by it is a function of each
card and has the stability property natively.

`target_count` is set by the server to the due count at creation and stored, on Part 5's
argument: it is the denominator the student was promised at the start, and recomputing it on
read would silently rewrite history. A client-supplied `target_count` stays a 422, with an
sm2-specific message.

**Missing schedule rows count as due.** The query uses a `LEFT JOIN` and treats a NULL
`due_at` as due now. Every card gets a row at creation, so this should be unreachable — but
there is no honest reason to hide a card from study because a bookkeeping row is missing, and
an INNER JOIN would hide it silently.

### 4a. A lapsed card is not re-served in the same session

Its new `due_at` is tomorrow, and it has a `reviews` row for this session either way.

Considered and rejected: Anki-style relearning steps that bring a failed card back later in
the same run. That would mean serving a card twice in one session, which breaks the serve rule
above — the rule that buys reload stability — and it is a larger change than "standard SM-2",
which is what the spec asked for.

## 5. Nothing due refuses at creation

Starting an SM-2 session on a deck with nothing due is refused, with a message naming the next
due date. This matches the master spec's existing Error handling rule: *a session with no
eligible cards fails at creation with a clear message rather than producing an empty runner*.

Considered and rejected: serving the soonest-due cards anyway when nothing is due. It never
blocks studying, but it defeats the scheduler completely — every session becomes "review
everything", and the intervals stop meaning anything the moment they become advisory. Practice
mode already exists for "I want to study now regardless", and it is one tile away.

The deck page is given the due count and the next due date so it can **disable the tile and say
when the next card is due**, rather than offering a button whose only outcome is a refusal.

## 6. Day-granular due dates

`due_at` is written at **midnight UTC of the due day** — `date(<base>, '+N days')` — not at
`<base> + N * 86400` seconds. A card answered at 21:00 with a one-day interval must be due when
you sit down at 08:00 the next morning; a seconds-exact interval would make it due at 21:00,
and a morning study session would find an empty deck for reasons no student would guess. The
flip side of the same rounding: a card answered late at night gets a near-zero effective
interval — `date(answered_at, '+1 days')` for a card answered at 23:50 is due ten minutes
later. That is the unavoidable cost of rounding to the day rather than the second, and the
right call stands; it is simply worth naming alongside the upside above.

`interval_days` stays `REAL`, as the DDL has it — the intervals SM-2 produces here are whole
days, but the column does not need narrowing to say so.

## 7. The answer write is one transaction

In SM-2 mode, `/answer` inserts the `reviews` row **and** updates `schedule` in a single
transaction. The master spec names this exact pair under Error handling: *writes touching
multiple tables (card + children, answer + schedule) run in a single transaction*.

The current handler (`sessions.rs:959-973`) does a bare insert with no transaction, so this is
a change to that code path rather than an addition beside it.

`due_at` is computed from the new review's `answered_at`, not from `now`, so the review row and
the schedule row cannot disagree about when the answer happened.

Practice and mock **do not touch `schedule`**. This is already guarded by the canary test
`mock mode must leave the sm-2 schedule alone` (`backend/tests/mock.rs:896`), which must stay
green untouched.

## 8. The override replays the card's schedule

`POST /api/reviews/:id/override` gains one step, taken only when the overridden review's
session mode is `'sm2'`: after flipping `correct`/`overridden`, recompute that card's
`schedule` by replaying every SM-2 review for the card, in `answered_at, id` order, through
`replay`.

Three things make this the right shape rather than an in-place adjustment:

- **The spec already claims it.** "A scheduling bug therefore cannot destroy history — in the
  worst case `schedule` is recomputed from `reviews`." A replay makes that a real, exercised,
  tested code path instead of a claim nobody has run.
- **SM-2's ease update is not cleanly invertible** once later reviews have landed on top, so an
  inverse adjustment would drift silently — the worst failure mode available here, because
  nothing on any screen would look wrong.
- **`due_at` is based on the last review's `answered_at`**, not on `now`. An override performed
  the next day must not push the card a further day out; the replay is time-honest because it
  reconstructs from when the answers actually happened.

The flip and the recompute go in the same transaction as the existing `accepted` insert.

### 8a. One thing deliberately not changed

`sessions.rs:1054-1058` already refuses to override a flashcard outside mock mode — "Grade the
flashcard again instead of overriding it" — and its condition (`kind == "flashcard" && mode !=
"mock"`) already covers `sm2` with no edit at all. The refusal is kept: in SM-2 a flashcard is
self-graded, so the student's verdict *is* the grade, and overriding your own verdict is
incoherent.

The *message* becomes slightly misleading under SM-2, because the card will not come back later
in this session. Recorded as a known minor rather than changed — see §11.

## 9. Stats: the answer to Part 6 §10

Part 6's design left exactly one question open:

> SM-2 will introduce a third mode whose reviews land in the same table. §3 splits the
> aggregates by mode on the argument that the *sampling* differs; SM-2 samples by due date,
> which is a third sampling rule again. Part 7 should decide whether the strip grows a third
> figure or whether SM-2 reviews fold into one of the existing two.

**Decision: the strip grows a third figure, `SM-2 nn% (n)`.**

This extends Part 6 §3's own argument rather than contradicting it. The split exists because
the *sampling* differs — practice over-serves your weaknesses and is pessimistic by
construction, a mock test is an unbiased sample — and due-date sampling is a third rule, biased
a third way: it serves what the scheduler believes you are about to forget. Folding SM-2 into
practice would produce a number describing neither sampling rule, which is precisely what Part
6 refused to do when it split mock from practice in the first place.

`DeckStatsSummary` gains four fields, of which only the first two are strip figures:

- `sm2_accuracy` / `sm2_review_count` — the third figure. A third `mode = 'sm2'` bucket in the
  `load_summary` query (`stats.rs:68-75`). The existing buckets are explicit string
  comparisons, so SM-2 reviews are currently invisible to both rather than polluting either;
  this is purely additive.
- `due_count` / `next_due_at` — **not strip figures.** They exist so the deck tile can be
  enabled, disabled and labelled (§5). They belong to the tile, not the strip.

A mode with no reviews still reads `—`, never `0%`, per Part 6 §3.

`load_card_stats` is **unchanged**: the per-card miss rate already pools all modes, on Part 6's
argument that a card's own hit rate is not biased by how often it was served. SM-2 reviews join
that pool with no code change, which is the behaviour we want.

The cost, accepted: the strip is now three accuracy figures plus coverage plus last-studied, on
a wrapping flex row that **has never been rendered at 375px** in any part of this project.

## 10. The runner is `SessionPage.tsx`, reused

Part 5 gave mock its own page, and the reasoning there is worth restating because it does *not*
apply here. `SessionPage` holds five pieces of state a mock run must never enter — the verdict,
the revealed answer, the two override flags and the streak — and a mode branch would have to
prove five negatives on every render in a runner with no test coverage at all. A separate file
proved them by absence.

SM-2 needs **all five**. It is the practice loop with a different pool: same verdict, same
reveal, same four self-grades (which is the whole reason `self_grade` exists), same override,
same streak. A separate page would be ~378 duplicated lines whose only job is to stay
identical, and the first divergence would be a bug in one of them.

So: `served` widens to `PracticeNextResponse | Sm2NextResponse`, and the header strip becomes
mode-aware — `n of m due` for SM-2 against `n in the pool` for practice. Part 5's existing
redirect (`if (response.mode === 'mock')`) already does the right thing for `sm2`, which falls
through it. `MockSessionPage`'s inverse redirect (`!== 'mock'`) already bounces an sm2 session
to `/session`. Neither needs an edit — this is the slot Part 5 said it was leaving.

**No new colour token.** `check-contrast.py` must still report **16 ENFORCED rows with an empty
RECORDED tier**; an unchanged count is the evidence that no pair crept in.

## 11. Known minors, recorded rather than fixed

- The flashcard override refusal message ("Grade the flashcard again instead of overriding
  it") is misleading under SM-2, where the card will not come back in the same session. The
  refusal itself is correct (§8a); only the wording is imprecise.
- With the COS781 test on **11 September 2026**, spaced repetition will barely get to prove
  itself — the master spec says so directly. A 1/6/16-day interval sequence gets through
  roughly two steps before the test. It is built because it makes the app worth keeping for the
  rest of the module, and it is sequenced last so it never blocks studying.
- `pool_count` means something different in sm2 than in the other two modes. It is set from
  `count_due(...)`, which counts cards due *right now*, so in sm2 it shrinks as the session
  progresses rather than staying a fixed denominator. Nothing reads it that way today —
  `target_count` is always present for sm2, so the frontend only reaches for `pool_count` as a
  dead fallback — but the field invites a future reader to use it as a denominator that counts
  down toward zero.
- A long-lived sm2 session can, in principle, exceed its promised `target_count`: the serve
  query re-evaluates "due now" on every call, so a session resumed after a card matures mid-way
  through can serve a card beyond the count it announced at creation. Low probability, since
  the deck page always creates a fresh session, and bounding the serve to the cards due at
  creation is a bigger change than it is worth now.
- "Next due" has two definitions in this codebase that happen to agree only because both are
  read exclusively when nothing is due: `sessions::next_due_at` uses an INNER JOIN with no due
  filter, while the stats query uses `MIN(due_at)` over a LEFT-JOINed CTE that includes
  already-overdue cards. Both actually compute "earliest `due_at`", not "next *future* due".

## 12. Explicitly out of scope

- Any migration, new table or new column.
- Re-serving lapsed cards within a session (§4a).
- A global "cards due across all decks" view. The deck page is where sessions start, and Part 6
  already ruled against a second rendering of a list that lives a few pixels away.
- Changing practice or mock grading, sampling or storage in any way.
- Anything in build step 8: the bundle, LAN binding, and the phone layout pass.
