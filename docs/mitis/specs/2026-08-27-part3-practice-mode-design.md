# Part 3: practice mode — design decisions

**Date:** 2026-08-27
**Author:** Hayley Dodkins (design session with Claude)
**Status:** decided; implementation in
[`../plans/2026-08-27-part3-practice-mode.md`](../plans/2026-08-27-part3-practice-mode.md)

Companion to [`2026-08-26-quiz-study-app-design.md`](2026-08-26-quiz-study-app-design.md),
which is the master record and has been amended where this document changes it. Read this
one for *why*; read the master for *what the app is*.

Part 3 is build step 3: the session runner, grading, the "I was right" override, and
`reviews` rows. Not in scope: the Bibble theme (step 4), mock test (5), stats (6), SM-2 (7).
Part 3 neither reads nor writes `schedule`.

---

## 1. Card order: weighted sampling, not authored order

`docs/HANDOVER.md` said practice mode "should read [`cards.position`] rather than inventing
an order of its own". The master spec says practice mode is "weighted sampling, biased
toward weakness". These are different orderings and the spec wins.

`cards.position` is the *authoring* order — the sequence you want when reading a deck like a
document, which is what the deck screen does. Practice mode's entire value is that it does
**not** play the deck in order: it surfaces what you are weak on. Using `position` would
make practice a linear read-through, which the deck screen already provides.

`position` therefore plays no part in practice mode. It is not even a tiebreak: equal-weight
candidates under weighted random selection are already uniform, so ordering them changes
nothing.

## 2. Flashcard self-grades need a column

Flashcards are self-graded on four levels (`again` / `hard` / `good` / `easy`), but
`reviews.correct` is one bit. The master spec only maps the four levels to numbers under
SM-2, which is build step 7 — so without a column, every flashcard grade between now and
then would be recorded as a bare bit and the levels lost at the moment they are made.

**Decision: migration `0003` adds `reviews.self_grade TEXT CHECK (self_grade IN
('again','hard','good','easy'))`, nullable.** `correct` stays `NOT NULL` and is derived on
write: `again` → 0, the rest → 1.

Two columns rather than one because they answer different questions. "Did I get it" is
queried by every statistic and wants a bit; "how hard was it" is needed only by the
scheduler. NULL passes the CHECK (`NULL IN (…)` is NULL, and SQLite accepts a CHECK that is
not false), which is what lets the two auto-graded kinds share the table — verified rather
than assumed, along with the rejection of an out-of-range value.

Considered and rejected: storing the grade in the existing nullable `given` column. Zero
migration cost, but `given`'s meaning would become kind-dependent, and `given` already has a
job for the other two kinds. The database was throwaway debris at this point, so the
migration was nearly free.

## 3. `POST /answer` must identify the card

The master spec's body was `{given}`. That cannot work: `/next` is a GET that writes
nothing, so the server has no memory of which card it served, and grading is impossible.
`given` also cannot express a choice selection (an id smuggled into a string is a type lie)
or a self-grade.

**Decision: `{card_id, given | choice_id | self_grade, ms?}` — three mutually exclusive
answer fields, one per kind.**

Considered and rejected: having `/next` record a "served" row. It makes a GET a write, needs
a table with no other purpose, and demands a reconciliation policy for every abandoned
serve. It also contradicts the spec's own architecture — "session state is not held in
browser memory" is satisfied by `reviews` being the *only* session state, so a second state
table is the regression, not the fix. Under the chosen design an abandoned serve writes
nothing at all, so it consumes no no-repeat-window slot and needs no cleanup.

**The trust boundary is the card pool, not the serve order.** The server validates that
`card_id` is a non-archived card in a deck belonging to this session; that is what stops a
client grading arbitrary cards. It deliberately does *not* verify that `/next` served that
card, because enforcing that requires exactly the state table rejected above, and this is a
single-user LAN app. Answering a card currently inside the no-repeat window is likewise
allowed: the window is a serving policy, not a validity rule.

## 4. Flashcard reveal is its own endpoint

Self-grading structurally requires seeing the answer *before* submitting a grade, which
looks like a breach of the spec's "the correct answer is never sent to the client before the
student answers".

The first draft of this design put `answer_md` on `/next` for `flashcard` kind only, on the
grounds that a self-graded card has no key to protect. That reasoning is sound but the
implementation is weak: it forces the shared serve struct to *own* an `answer_md` field, so
the leakable field exists on the path used by all three kinds and only a runtime test keeps
it away from the two that are auto-graded.

**Decision: `POST /api/sessions/:id/reveal`, flashcard only, and `/next` carries no answer
content for any kind.** The serve struct then has no field capable of holding a key, and the
serve SQL never selects the key columns. Leakage becomes impossible at three layers instead
of merely absent at one:

1. `NextCardResponse` / `NextChoiceResponse` are distinct types from `cards::CardResponse` /
   `cards::ChoiceResponse`, which do carry `is_correct`, `answer_md`, `explanation_md` and
   `accepted`. `routes/sessions.rs` must not import from `routes::cards`, and must not use
   `#[serde(flatten)]` on this path.
2. The serve projections select only `id, kind, prompt_md, image_path` and `id, text_md`.
   The key columns never enter the process. Shuffling needs no knowledge of which choice is
   correct; grading resolves `is_correct` by id at answer time.
3. `choices` is a `Vec`, not an `Option`, so it serialises to `[]` for the other two kinds —
   there is no kind-conditional branch that could later grow an answer field.

**Restated invariant**, which is what the leakage test encodes: *no endpoint returns
correctness data or answer content for a card except in response to a request that is itself
an act on that card* — submitting an answer, or explicitly asking to reveal a flashcard.

Stated honestly: `/reveal` is a **contract boundary, not a security boundary**. A client may
call it immediately. For a self-graded card nothing could do better, since the student is
the grader. What it buys is that `/next` is uniformly key-free and testable as a single
assertion across all three kinds.

Cost: one extra round trip when the student reveals a flashcard. Imperceptible on a LAN.

## 5. `/next` re-rolls on reload, and that is correct

The spec requires "Reloading mid-session resumes from `reviews`; session state is not held
in browser memory." Weighted random selection means a reload serves a *different* card.

This is not a compromise. What "resumes from `reviews`" guarantees is that every **input** to
the next decision derives from `reviews` — the weights, the staleness, the no-repeat window,
the progress counts. A reload can neither reset the window nor re-serve a card you just
answered. A practice session has no ordered position to resume *to*, because "the session
has no end", and an unanswered serve wrote no row, so discarding it loses nothing.

The sharp version of that claim is a test, not a paragraph: answer three cards, discard the
client, call `/next` repeatedly — those three are still excluded.

**Part 5 must revisit this.** In mock test mode `target_count` makes each serve
consequential, so re-rolling would be a real defect there.

Two related rulings:

- **The no-repeat window is over `reviews`, not serves.** This is what makes an abandoned
  serve free — it consumes no slot because it wrote no row.
- **The window is session-scoped.** "So it does not feel like a loop" is about the felt
  experience of the current run. Cross-session exclusion would fight the never-seen and
  staleness terms at the start of every short session.

## 6. Never-seen dominance is derived, not tuned

The spec orders practice weighting: never-seen highest, then recent miss rate, then
staleness. Encoding that as strict lexicographic tiers would be wrong — any fully-correct
card would become unreachable while a single missed card existed, which is the looping
failure the no-repeat rule exists to prevent. So the three signals are additive terms with a
3:1 miss-rate-to-staleness ratio, expressing "then" as relative influence.

Both variable terms are normalised to `[0, 1]`, which lets "never-seen is highest" be an
algebraic consequence rather than a lucky choice of constants:

```
MAXIMUM_REVIEWED_WEIGHT = BASE_WEIGHT + MISS_RATE_WEIGHT + STALENESS_WEIGHT
NEVER_SEEN_WEIGHT       = MAXIMUM_REVIEWED_WEIGHT + NEVER_SEEN_HEADROOM
```

1. `weighted_miss_rate ≤ 1` — every numerator term is ≤ its denominator term, since
   `missed ∈ {0,1}` and the decay factor is positive.
2. `staleness_fraction ≤ 1` — it is `1 − 0.5^x` for `x ≥ 0`.
3. So any reviewed card weighs at most `MAXIMUM_REVIEWED_WEIGHT`.
4. `NEVER_SEEN_WEIGHT` exceeds that by construction. ∎

Step 4 references the *sum*, so all three term weights can be retuned freely without
breaking the invariant. A future fourth term must be added to `MAXIMUM_REVIEWED_WEIGHT` in
the same edit or the invariant test fails — which is the point of deriving it.

Deliberately **no** `.min(MAXIMUM_REVIEWED_WEIGHT)` clamp: no mutation of the formula can
make the clamp fire, so it would be untestable code. The bound is enforced by a
`const _: () = assert!(NEVER_SEEN_WEIGHT > MAXIMUM_REVIEWED_WEIGHT);` — a **compile-time**
failure rather than a test failure, so a constant cannot be retuned into breaking the
invariant even temporarily. Same reasoning kills the "if the window empties, fall back to
the full pool" safety net (§7), the override's "already overridden" branch (§8), and a
`roll.clamp(0.0, 1.0)` in the selector, which mutation testing showed to be equally dead:
a negative roll makes the target negative, so the first candidate's cumulative already
exceeds it, and a roll above one makes the target exceed the total, so it falls through to
the trailing `last()`. Both paths already produce exactly what the clamp would have forced.
The real guarantee — that *any* roll, including out of range, infinite or NaN, selects a
card from the included set — is asserted directly instead.

`BASE_WEIGHT` is non-zero so a perfectly-known card stays *reachable*, and so no candidate
can have zero weight — which is what lets the cumulative selection scan never skip one.

## 7. The no-repeat window shrinks on small decks

A fixed window of 8 would empty the candidate set on any deck of 8 or fewer cards.

```
effective_window = min(NO_REPEAT_WINDOW, eligible_count − 1)
```

| Eligible cards | Window | Behaviour |
| --- | --- | --- |
| 1 | 0 | the one card repeats — the only possible behaviour |
| 3 | 2 | `A,A` and `A,B,A` impossible; `A,B,C,A` allowed |
| 9+ | 8 | full spec behaviour |

At most `count − 1` **distinct** ids are excluded from `count` candidates, so at least one
always survives. That is a theorem, so non-starvation is proved by an exhaustive test over
pool sizes 1..=12 rather than guarded by an unreachable fallback. Computing the window from
`candidates.len()` inside the selector also means a pool that shrinks mid-session (a card
archived while you study) shrinks the window automatically.

## 8. Which reviews can be overridden

Only an incorrect `short_answer`. Both exclusions are deliberate:

- **`mc_single`** — its key is unambiguous. "I was right" about a radio button is a
  card-authoring bug, fixed in the editor, not a grading injustice.
- **`flashcard`** — the student is already the grader and can simply grade again.

There is **no separate already-overridden branch**: `overridden = 1` always implies
`correct = 1`, so the already-correct check subsumes it, and an unreachable branch would be
untestable.

The override inserts its `accepted` row with `is_primary = 0` always — the primary wording
belongs to the author, and a second primary would break the one-primary invariant that
`cards::validate` enforces and no database constraint does.

The insert carries its own duplicate guard as a single statement
(`INSERT … SELECT … WHERE NOT EXISTS`), which closes the issue HANDOVER flagged as "Part 3's
grading lookup will meet it": duplicate rows normalising to the same key were already
permitted, and the override must not add to the pile. One statement means no read-then-write
race, and it uses `idx_accepted_card_normalised` directly.

## 9. Grading edges worth recording

**`normalise()` can return the empty string.** An accepted answer of `"---"` passes card
validation (non-blank text) yet normalises to `""`. A blank or punctuation-only submission
would then silently match it. `grade_short_answer` therefore returns false on an empty
normalised input **without consulting `accepted` at all`**.

**Duplicate normalised accepted rows are harmless to grading** — the check is set
membership, not a fetch — which is why §8's guard is on the override rather than a new
unique index on a table that already has real rows.

**Choice shuffling is a leakage control, not presentation.** Served in `position` order, an
author's habit of typing the correct option first *is* the answer key. It belongs with the
leakage tests.

**`ORDER BY answered_at DESC, id DESC`** — the `id` tiebreak is mandatory, not decorative.
Timestamps are one-second resolution and ties are the *normal* case in rapid-fire practice;
without it, recency ranking is non-deterministic.

## 10. Recency is a fixed count, not a time window

The spec says "recent reviews weighted above old ones" without saying what recent means.

**Decision: the last 10 reviews per card, with positional decay (0.7), not a time window.**
A time window empties after a week away from the app, and a card missed twice a month ago is
still the weakest card in the deck. `RECENCY_DECAY = 0.7` gives the newest review 1.0 and
the tenth ≈ 0.040, a 25:1 spread — enough that a recent miss visibly beats an old one,
without the oldest reviews being numerically dead, which would make the limit of 10 a lie.

Implemented with `ROW_NUMBER() OVER (PARTITION BY card_id …)`, since SQLite has no `LATERAL`
and a correlated `LIMIT` per card is worse. **Cost, stated plainly:** the CTE materialises
every review for the pool's cards before the rank filter applies at join time, so the scan
is O(total reviews in these decks), not O(10 × pool). Microseconds at this app's scale; the
fix if it ever matters is to apply the filter inside a wrapping subquery so it precedes the
join.

`json_each` unpacks the stored `sessions.deck_ids` JSON array directly, which keeps a
variable-length `IN` list inside a macro-checkable literal query rather than a `format!`.

## 11. Small contract decisions

- **`target_count` on a practice session is rejected**, not stored and ignored, so no client
  comes to rely on a value the server discards. Practice "has no end" — a target is
  self-contradictory.
- **`/finish` is idempotent with 200**, via `WHERE id = ? AND ended_at IS NULL`, preserving
  the original `ended_at`. Finishing is a terminal-state assertion; a reload that
  double-posts must not show the student an error.
- **`accuracy` is `null` for a session with no answers**, not `0.0`, which would claim you
  got everything wrong.
- **`can_override` is computed server-side** so the button's precondition is testable in one
  place instead of duplicated in the client.
- **`expected` is a `Vec<String>`** — one shape for all three kinds: the correct choice's
  text, every accepted wording primary-first, or `answer_md`.
- **No `GET /api/sessions/:id`.** `/next` carries the progress counts a resuming client
  needs, and a session-detail endpoint would be the natural place for a future leak.
- **A per-card weakest-cards breakdown is Part 6's job**, not the practice summary's.

## Open questions for later parts

- Part 5 (mock test) must replace `/next`'s re-roll with something stable, since
  `target_count` makes each serve consequential (§5).
- Part 5 must also decide what a flashcard means in a mock test, where there is no feedback
  during the run but self-grading needs the answer.
- Part 7 (SM-2) is the first consumer of `reviews.self_grade` and of `schedule`, which Part
  3 leaves untouched.
