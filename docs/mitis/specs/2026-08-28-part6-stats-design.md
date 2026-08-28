# Part 6: stats — design decisions

**Date:** 2026-08-28
**Author:** Hayley Dodkins (design session with Claude)
**Status:** decided; implementation plan to follow

Companion to [`2026-08-26-quiz-study-app-design.md`](2026-08-26-quiz-study-app-design.md),
which is the master record and is amended by this part where this document changes it — those
amendments are tasks in the implementation plan. Read this one for *why*; read the master for
*what the app is*.

Part 6 is build step 6: statistics. Not in scope: SM-2 (step 7), the bundle and phone pass
(8). Part 6 neither reads nor writes `schedule` — that table stays Part 7's, untouched.

It inherits two gifts from Part 5, both noted in the handover: `GET
/api/sessions/:id/results` is gated on session state rather than mode, so practice sessions
already return a per-question record; and `sessions.mode` is now a meaningful filter rather
than a column nothing reads.

---

## 1. The screen's job is to send you back into a deck with a reason

The COS781 test is on 11 September 2026. Two weeks out, the question worth answering is not
"am I improving" — that needs a run of sessions this history does not yet have — and not
"where do I stand", which reports without concluding. It is **what do I study next**.

Everything below follows from that. Weakness is the headline; accuracy exists to rank it;
coverage exists because a card never once attempted is the sharpest gap the app can see. A
trend chart, a session history list and a per-card sparkline were all considered and all cut:
each is a thing you look at, not a thing you act on.

---

## 2. Stats live on the deck page; `/stats` is deleted

The master spec gave stats their own screen and their own nav entry. Part 6 moves them onto
`/decks/:id` and removes the screen entirely, along with its nav link and
`frontend/src/pages/StubPage.tsx` — a grep confirms the stub has exactly one caller, so it
leaves with the route.

The reason is that **the deck page already lists every card**. A global "weakest cards" table
would be a second rendering of a list that exists a few pixels away, kept in sync by hand and
free to disagree with it. Annotating the rows that are already there costs one badge and
produces the same ranking, next to the card it judges, in the place you would act on it.

The cost is real and accepted: with several COS781 decks you cannot see at a glance which deck
is worst without opening each one. A deck league table on `/stats` was designed and rejected —
it answers "which deck do I open" once, and the decks list is short enough to walk. If that
stops being true, the table is a small addition on top of this endpoint, not a redesign of it.

This amends the master spec in two places: the screen table's `/stats` row, and the endpoint
table's `GET /api/stats?deck_ids=`.

---

## 3. Practice accuracy and mock accuracy are different numbers and are shown as two

Practice mode deliberately over-serves the cards you are weak on — that is what
`MISS_RATE_WEIGHT` in `backend/src/practice.rs` is for. A consequence is that **any aggregate
over practice reviews is pessimistic by construction**: the pool is weighted towards your
weaknesses, so the average is dragged below your actual command of the deck. A mock test
serves every card exactly once, so its accuracy is an unbiased sample.

Pooling them yields a third number that is neither, and is not the average of the two, because
the weighting decides how many reviews each side contributes. So:

- **Deck-level and overall accuracy are split**, shown as `Mock 71% (42)` and
  `Practice 58% (137)` side by side, each with its review count so the figures can be weighed.
- **Per-card miss rate pools both modes.** A card's own hit rate is not biased by how often it
  was served — being asked a question ten times rather than twice changes the confidence in
  the number, not the number's meaning. Splitting it would halve the sample for no gain.

A mode with no reviews reads `—`, never `0%`. "I have never sat a mock test on this deck" and
"I sat one and scored nothing" are opposite facts and the strip must not blur them. A deck with
no reviews at all shows one muted line — "No sessions yet" — rather than a strip of dashes.

---

## 4. Weakness is `weighted_miss_rate`, the function practice mode already acts on

`backend/src/practice.rs` already defines weakness: `weighted_miss_rate` over a card's last
`RECENT_REVIEW_LIMIT` (10) reviews, each older review discounted by `RECENCY_DECAY` (0.7). It
is a pure function with unit tests, and it is the input practice mode uses to decide what to
serve.

Part 6 reuses it rather than defining a second notion of "weak". Two definitions in one
codebase drift, and the drift is invisible: the screen would rank one card worst while the
practice run served another, with nothing to say which was wrong. Reusing the function makes
the badge a window onto the decision the app is already making.

Raw lifetime accuracy was rejected because it treats a miss from week one as equal to one from
last night — precisely the card you have since fixed. A Wilson lower bound was rejected as a
number that cannot be explained to yourself at a glance, which for a personal revision tool is
disqualifying.

**Staleness stays out of it.** `staleness_fraction` is also in `practice.rs` and also feeds the
serving weight, but it answers *when to revisit*, not *how well you know it*. Folding it into a
badge labelled "missed" would make a card you know perfectly and have not seen in a week look
weak.

`fold_candidate_rows` is reused alongside it: it already returns `review_count`,
`recent_review_outcomes` and `seconds_since_last_review` per card, which is exactly the badge's
input.

---

## 5. A dedicated endpoint, and why not the other two shapes

```
GET /api/decks/:id/stats  →
{
  "summary": {
    "card_count": 42,
    "unseen_count": 11,
    "mock_accuracy": 0.71,
    "mock_review_count": 42,
    "practice_accuracy": 0.58,
    "practice_review_count": 137,
    "last_answered_at": "2026-08-28T09:14:02Z"
  },
  "cards": [
    { "card_id": 7, "attempt_count": 6, "miss_rate": 0.42 }
  ]
}
```

Both accuracies and the summary's `last_answered_at` are nullable; `cards` may be empty.

A per-card `last_answered_at` was drafted and dropped. The badge does not render it, and it is
the one field the candidate query cannot supply — `fold_candidate_rows` returns
`seconds_since_last_review`, not a timestamp — so carrying it would mean a second query for a
value nothing displays.

**Rejected: folding the per-card figures into `GET /api/decks/:id/cards`.** One fewer request,
but it pushes review aggregation into a query the card editor and every card list also pay for,
and `CardSummary` is shared across four screens with no use for a miss rate.

**Rejected: the master spec's `GET /api/stats?deck_ids=`.** It was written before stats had a
home. With no global screen it has exactly one caller passing exactly one id — a general
endpoint with no general use. This is the same kind of amendment Part 5 made to the `/answer`
response shape: the spec described a shape that the implementation found to be the wrong one.

**Chosen: a separate endpoint,** fired alongside the existing `getDeck` and `listCards` calls.
Stats change on every answer; cards change only on edit, so they have different cache
lifetimes. Keeping them apart means a stats failure degrades to "no badges", not "no deck".

### Two rules the payload obeys

**Archived cards are excluded** from `card_count`, `unseen_count` and `cards`. They will never
be served again, so counting them would put coverage permanently out of reach. This is
consistent with the `pool` CTE in `load_candidates`, which also filters `archived = 0`.

**`overridden` is honoured** — a review you overrode counts as correct, because
`reviews.correct` is the column the override endpoint flips. Reading `correct` is therefore
already the right thing; the rule is recorded so a later change does not "fix" it by excluding
overridden rows.

**Cards with no reviews are omitted** from `cards` rather than sent with `attempt_count: 0`.
Absence *is* the unseen signal, and the array stays small.

---

## 6. The SQL is not shared with practice mode

`load_candidates` in `backend/src/routes/sessions.rs` is a close cousin of the query Part 6
needs — a `pool` CTE, `ROW_NUMBER()` ranked recent reviews, a counts join. It is not
generalised to serve both.

It is session-scoped across a deck *list*; the stats query is scoped to one deck and
additionally needs the mode split, which means a join to `sessions` that practice mode has no
use for. Generalising would couple card-serving — where a broken query breaks a live study
session — to a read-only garnish on a page whose real job is the card list. The pure folding
functions are the part worth sharing; the SQL is not.

New module: `backend/src/stats.rs` for the query and aggregation, with the handler on
`backend/src/routes/decks.rs` alongside the deck's other routes.

---

## 7. The deck page

**The summary strip** sits directly under the three mode buttons, inside the existing `pl-11`
column so it lines up with the card list:

```
31 of 42 answered  ·  Mock 71% (42)  ·  Practice 58% (137)  ·  Last studied 2 hours ago
```

Coverage leads, per §1. `last_answered_at` goes through the existing relative formatter in
`frontend/src/lib/format.ts`.

**The per-row badge** goes on `CardRow`, in the row's existing metadata area, with three
states:

- **Unseen** — the card is absent from `cards`. Muted, no number.
- **A miss rate** — `42% missed · 6`, the count being attempts, so the percentage can be
  weighed. At or above 40% the badge takes an emphasised variant; below it, muted.
- **Archived rows get no badge.** They are outside the payload, and an "Unseen" badge on an
  archived card would be a claim about outstanding work that is not outstanding.

**The card list does not become sortable by weakness.** It is drag-reorderable by `position`,
and a second ordering that silently overrides the one you arranged by hand is how two lists
end up disagreeing. The badges make weak cards findable without taking the ordering away.

**Failure is quiet.** If the stats request fails the page renders as it does today, minus the
strip and the badges. No toast: a red toast over a working deck page would be the loudest
thing on screen for the least important reason.

**The badge adds no new contrast pairs, and that is a finding rather than a convenience.**
The obvious choice for the emphasised state was shadcn's `destructive` badge variant, which
is a 10%-alpha tint of `--destructive` under `--destructive` text. Measured against the row's
`bg-card` backdrop it comes to **3.64:1 in light and 3.60:1 in dark — both below AA**. The
variant is vendored shadcn and stays untouched per the CLAUDE.md carve-out, so the emphasised
badge instead uses the solid pair, `bg-destructive` under `--destructive-foreground`: 5.04:1
light, 5.52:1 dark, and already an ENFORCED row in `check-contrast.py` as "verdict
destructive". The muted state uses the `secondary` variant, also already ENFORCED at 7.65:1
and 10.0:1 as "choice unselected".

So `check-contrast.py` gains nothing and the gate already covers both states. The tinted
variant's failure is recorded here so it is not reintroduced later as a cosmetic tweak — the
handover notes a 2.14:1 failure that survived three parts unnoticed, and this is the same
shape of defect caught before it shipped rather than after.

---

## 8. Tests

`backend/tests/stats.rs`, on the existing `tests/common` harness:

- A deck with no reviews: `unseen_count == card_count`, both accuracies null, `cards` empty,
  `last_answered_at` null.
- Archived cards fall outside `card_count`, `unseen_count` and `cards`.
- Mock and practice accuracy are computed from their own reviews only — the fixture is built
  so that pooling the two modes gives a third, different number, or the test proves nothing.
- An overridden review counts as correct.
- Recency weighting is load-bearing: a card missed early then answered right three times ranks
  **below** a card answered right early then missed three times, the two having identical raw
  accuracy. Swapping `weighted_miss_rate` for unweighted accuracy turns this red.
- Cards with no reviews are omitted from `cards`, not sent at zero.
- Reviews on another deck's cards do not appear.
- 404 on an unknown deck id.

Per the handover's rule, each test gets a mutation check: change the one thing it claims to
prove, confirm it goes red. Part 3 found six tests that could not fail; roughly one per task
was hollow.

The frontend is covered by the existing gate — `tsc -b --noEmit`, `pnpm build`, `oxlint` —
plus the contrast script gaining the badge's pairs.

---

## 9. Explicitly out of scope

Stated so it is not quietly absorbed during implementation:

- No trend-over-time chart and no session history list.
- No cross-deck, module-level or global view; `/stats` is deleted, not deferred.
- No sorting the card list by weakness.
- No sparklines or per-card review history — the miss rate already carries recency inside it.
- Nothing touching `schedule`.

## 10. Open question left to Part 7

SM-2 will introduce a third mode whose reviews land in the same table. §3 splits the
aggregates by mode on the argument that the *sampling* differs; SM-2 samples by due date,
which is a third sampling rule again. Part 7 should decide whether the strip grows a third
figure or whether SM-2 reviews fold into one of the existing two — and the answer should be
recorded here, not discovered in the strip.

### Answered, in Part 7

**The strip grows a third figure, `SM-2 nn% (n)`.** This extends this section's own argument
rather than contradicting it: the split above exists because the *sampling* differs — practice
over-serves your weaknesses and is pessimistic by construction, a mock test is an unbiased
sample — and due-date sampling is a third rule, biased a third way: it serves what the
scheduler believes you are about to forget. Folding SM-2 into practice would produce a number
describing neither sampling rule, which is precisely what §3 refused to do when it split mock
from practice in the first place.

`DeckStatsSummary` gained four fields, of which only the first two are strip figures:
`sm2_accuracy` and `sm2_review_count` are the third figure, a third `mode = 'sm2'` bucket
alongside the existing two in `load_summary`; `due_count` and `next_due_at` are **not** strip
figures — they exist so the deck's Spaced-repetition tile can be enabled, disabled and
labelled with the next due date, and belong to the tile rather than the strip. A mode with no
reviews still reads `—`, never `0%`, exactly as for practice and mock.

`load_card_stats`, the per-card miss rate, needed no change: it already pools all modes on
this section's own argument that a card's own hit rate is not biased by how often it was
served, so SM-2 reviews join that pool for free.

The cost, accepted: the strip is now three accuracy figures plus coverage plus last-studied,
on a wrapping flex row that has never been rendered at 375px in any part of this project. See
Part 7's design record, [`2026-08-28-part7-sm2-design.md`](2026-08-28-part7-sm2-design.md) §9,
for the full reasoning.
