# Part 5: the mock test — design decisions

**Date:** 2026-08-28
**Author:** Hayley Dodkins (design session with Claude)
**Status:** decided; implementation in
[`../plans/2026-08-28-part5-mock-test.md`](../plans/2026-08-28-part5-mock-test.md)

Companion to [`2026-08-26-quiz-study-app-design.md`](2026-08-26-quiz-study-app-design.md),
which is the master record and has been amended where this document changes it. Read this
one for *why*; read the master for *what the app is*.

Part 5 is build step 5: mock test mode and the results screen. Not in scope: stats (step 6),
SM-2 (7), the bundle and phone pass (8). Part 5 neither reads nor writes `schedule`.

It also closes the three questions left open for it — the two recorded in
[`2026-08-27-part3-practice-mode-design.md`](2026-08-27-part3-practice-mode-design.md) §5 and
"Open questions for later parts", and the third created by the `e09e76d` styling pass.

---

## 1. One deck, and the whole deck

The master spec defined a mock test as "`target_count` cards sampled uniformly at random" and
left two things to this part: what the pool is scoped to, and how long the test is.

**Scope: one deck.** The `e09e76d` styling pass moved session starting onto `/decks/:id`, so
the only entry point is a deck you are already looking at. A mock test standing in for the
COS781 test is arguably more plausibly a whole module, and the backend has accepted a
`module_id` since Part 3 — but a module-wide mock needs a picker that does not exist, and the
deck page's three-button grid is the shape the app now has. Mock tests match the button that
starts them.

**This is a UI decision, not an API one**, exactly as the handover framed it. The backend does
**not** reject `module_id` or a multi-deck list for a mock session: that would delete a tested
capability to enforce a choice about which buttons exist. A mock test is "the whole pool",
which for a single-deck session is "the whole deck".

**Length: the whole deck, every card exactly once.** No length picker, no presets.
`target_count` is a *sampling* parameter and sampling is the wrong model here: a revision deck
is already the set of things you decided are worth knowing, so a mock test over a strict
subset of it tests a random half of your own syllabus. Answering every card once is both the
simpler product and the more useful one.

`target_count` is therefore **computed by the server** as the pool size at creation, not
supplied by the client. A client-supplied `target_count` is still rejected, matching Part 3's
"reject rather than ignore" ruling — no client should come to rely on a value the server
overwrites.

This narrows the master spec's definition: the count is now always the pool size, so the
sampling collapses to a permutation. The spec has been amended to say so.

---

## 2. The stable serve needs no new storage

Part 3 §5 established that `/next` re-rolling on reload is *correct* for practice, and flagged
that "Part 5 must revisit this. In mock test mode `target_count` makes each serve
consequential, so re-rolling would be a real defect there."

Serving every card exactly once dissolves the problem rather than solving it. **The next card
is the first card, in a fixed order, that has no `reviews` row for this session.** Both halves
derive from data already stored: the order from a function of `(session.id, card_id)`, the
answered set from `reviews`.

So a reload re-serves the same card, because nothing about the decision changed. There is no
serve to lose and no reconciliation policy for an abandoned one.

This matters because Part 3 §3 considered and rejected the obvious alternative — having
`/next` record a "served" row — on three grounds: it makes a GET a write, it needs a table
with no other purpose, and it contradicts the architecture in which `reviews` is the *only*
session state. All three objections still stand, and this approach is subject to none of them.
It adds no table and no column. It also *strengthens* Part 3's "resumes from `reviews`" claim
from "every input is derived from it" to "the output is identical".

### 2a. Rank-by-hash, not a shuffle

The natural implementation is a seeded Fisher–Yates over the list of pool ids. **That is
wrong**, and the reason is the case §11 cares about: a Fisher–Yates permutation is a function
of *the list*, so archiving one card mid-test reshuffles every remaining card. A reload after
an archive would then serve a different question — reintroducing exactly the defect this
section exists to remove.

Instead each card gets its own sort key and the pool is ordered by it:

```
key(card_id) = mix64(seed ^ mix64(card_id))        seed = session.id
order        = pool ids sorted by (key, card_id)
```

Three properties follow, all unit-testable, none of which a shuffle has:

- **Input-order independence.** The result does not depend on the order the rows arrived in,
  so the `ORDER BY` in the serve query is belt-and-braces rather than load-bearing.
- **Stability under pool change.** Removing a card leaves the relative order of the rest
  untouched; adding one inserts it at its own rank. A card archived mid-mock does not reorder
  what is left.
- **Uniformity** over the seed space — a random-key sort — so the spec's "uniformly at random"
  is still honoured.

The `(key, card_id)` tuple rather than `key` alone makes the order total even on a hash
collision, so the function is deterministic without relying on sort stability.

### 2b. `session.id` is an adequate seed, and no new column

`id` is unique, immutable, already stored, and already the identity the client holds. A stored
`seed` column would buy nothing: it does not pin the *algorithm*, and it would be a second
store of derived state with no reader — the regression Part 3 §3 and the handover's "session
state lives only in `reviews`" both warn about.

The one caveat is that ids are adjacent small integers, and adjacent seeds must not produce
correlated orders. The inner `mix64(card_id)` before the xor is what decorrelates them, and a
test pins it.

### 2c. Not `rand::StdRng` — a local `mix64`

`rand 0.8` is already a dependency and `StdRng::seed_from_u64` compiles today with no manifest
change, so this is a choice rather than a constraint. It is still the wrong tool:

- **`rand` documents `StdRng`'s algorithm as unstable across minor releases.** A routine
  `cargo update` could silently reorder an in-progress mock test — the precise failure this
  whole section is built to prevent.
- **It fights the module convention.** `practice.rs` and `grading.rs` are pure, with
  randomness injected (`roll: f64`). A six-line SplitMix64 finaliser in `backend/src/mock.rs`
  is stable forever, dependency-free, and testable with no RNG at all.

`rand` stays exactly where it is, shuffling practice-mode choices.

### 2d. Choice order is deterministic in mock mode too

`/next` shuffles a card's options on every serve so the student learns the answer rather than
its position. Under a stable serve that becomes a defect: a reload would re-serve the same
question with its options rearranged — the card stable, the screen not, and a student
mid-question losing their place for no reason.

In mock mode the choices are ordered by the same rank-by-hash function, seeded from
`session.id` and `card_id` together. Two gains beyond the obvious: the whole `/next` payload
becomes byte-identical across reloads, which is a far sharper resume test than "the same card
id"; and the leakage control the shuffle exists for — never serving in `position` order — is
preserved.

Practice keeps `thread_rng()`. There, every serve is a fresh serve, and per-serve reshuffling
is the point.

---

## 3. A flashcard in a mock test is typed, and auto-graded

The second question Part 3 left open: "what a flashcard means in a mock test, where there is
no feedback during the run but self-grading structurally needs the answer."

The tension is real. Part 3 §4 made flashcard reveal its own endpoint precisely because
self-grading requires seeing the answer before submitting a grade. In a mock test that is
disqualifying: revealing the answer *is* feedback, and a self-graded question in an exam is
not a question.

**Decision: in mock mode a flashcard takes typed text, graded server-side against
`answer_md`.** No reveal step, no self-grade buttons.

The consequences are worth stating:

- **Practice-mode flashcards are completely unchanged** — reveal, then one of four
  self-grades, stored in `reviews.self_grade`. This is not politeness about backwards
  compatibility: Part 7 (SM-2) is the first consumer of `self_grade` and needs the four
  levels. Auto-grading flashcards everywhere would have quietly removed SM-2's input.
- **`reviews.self_grade` is NULL for a mock flashcard**, and `correct` carries the verdict.
  That is exactly the shape migration `0003` was designed for. **Part 7 must therefore map
  mock flashcard reviews through `correct`, not through the grade table.**
- **The stored `given` is the raw trimmed text**, not the normalised key, so the results screen
  can show what you actually wrote and the override has a real wording to work with.
- **The same card grades differently in the two modes.** Deliberate: the two modes ask
  different questions of the same card. Practice asks "did you know it"; a mock test asks
  "can you produce it".

### The caveat, recorded rather than hidden

`answer_md` is markdown prose. Card validation requires a flashcard to have one and trims it
non-blank, but nothing constrains it to be short — and the whole reason to author a flashcard
rather than a short-answer card is that the answer was not reducible to a key.

**A flashcard whose answer is a sentence will auto-grade wrong nearly every time**, because you
will not retype it verbatim. No distance metric fixes this; it is what grading free text
against prose costs. No authoring restriction is added — the card model is not the problem.

Two mitigations, and neither is a cleverer matcher:

- **The override** (§8) is the correction path, and it is why the override had to be extended.
- **The results screen says so.** When a mock run contains any flashcard, it carries a
  one-line note that flashcards are matched against their written answer and an invitation to
  mark the ones you got right. Without it, a 30% on a deck of prose flashcards reads as either
  a broken feature or a bad night's revision, and it is neither.

The practical advice, which belongs in the handover: for cards you intend to sit a mock test
on, keep flashcard answers short and keyword-ish, or author them as short-answer cards, which
have an `accepted` list built for exactly this.

---

## 4. Spelling tolerance: the rule, its cost, and where it stops

Normalisation does the safe half at zero risk. `normalise()` — NFKC, lowercase,
non-alphanumerics to spaces, whitespace collapsed and trimmed — already discards casing and
punctuation, so `  K-MEANS!  ` and `k-means` are the same key, as Part 3's walkthrough
confirmed for short answers.

On top of that, a bounded edit-distance fallback. `n` is the character count of the
**normalised expected answer** — the card sets the tolerance, not the student's typing:

```
tolerance(n) = min(n / 8, 2)
fuzzy applies only while n <= 120 and the submission is also <= 120
```

| n | edits forgiven | worked example |
| --- | --- | --- |
| 0–7 | 0 | `ridge` / `bridge` is distance 1 — different words, must not match |
| 8–15 | 1 | `clustering` absorbs `clusterng`; `maximise` / `minimise` is distance **2**, correctly rejected |
| 16–23 | 2 | `information gain` absorbs `informaton gain` |
| 24+ | 2 (capped) | a 40-character answer gets 5% tolerance — near-exact, which is the intent |

**The divisor is 8 because the errors are asymmetric.** A false *reject* is one click from
fixed — "I was right" is on every wrong row. A false *accept* is permanent, because there is
no opposite button (see below). So where the rule is uncertain it should refuse to forgive.

A divisor of 6 was tried first and rejected on a concrete case: `bridge` is six characters, so
it landed in the one-edit bucket and graded `ridge` correct. Six- and seven-character words one
edit apart are very often different words, not typos. Divisor 8 pushes the first forgiven edit
out to eight characters, which rejects `ridge`/`bridge` while still absorbing `clusterng`,
`overfiting`, `precison` and `informaton gain`.

The measured cost of that choice: `entropy` (7) now gets no tolerance, so `entrpy` grades
wrong. That is a false reject, and therefore one click from fixed. Accepted deliberately.

**No tolerance below eight characters.** At that length an edit is usually a different answer
rather than a typo. Forgiving one edit in a three-letter answer would make `cat`, `cot`, `car`
and `bat` mutually interchangeable, which is worse than strict.

**A hard cap of 2 edits**, because distance grows with length but so does the chance that a
third edit changes the meaning. Capping keeps long answers effectively exact.

**A length guard at 120 normalised characters, above which only an exact match counts.** With
the tolerance capped at 2 the fuzzy branch is already semantically meaningless up there, so
the guard changes almost nothing about verdicts — its real job is the cost cliff. Levenshtein
is O(n·m), and a multi-kilobyte prose `answer_md` would be tens of millions of cells per
answer.

Two pieces of the function are worth labelling correctly, because a mutation pass showed that
the obvious reading of each is wrong:

- **The exact-match early return is not merely an "exact first" optimisation.** The fuzzy
  branch handles an exact match on its own — distance 0 clears any tolerance — so below the
  length guard the early return is redundant. Its real and only load-bearing job is *above* the
  guard, where the fuzzy branch returns false unconditionally: without it, a long prose answer
  retyped perfectly would grade **wrong**.
- **The length-difference prefilter before the DP is a pure optimisation.** Removing it changes
  no verdict, and a test asserts exactly that by staying green.
- **Only the whole empty-key guard is provable, not either half.** With one side empty the
  prefilter already rejects, so each half alone is redundant; the case that needs the guard is
  *both* sides normalising to empty, where the exact-match return would otherwise call it
  correct. Mutate it as one unit or the mutation proves nothing.

### The accepted cost, recorded so it is a choice and not a surprise

**Fuzzy matching can only ever mark a wrong answer right, and there is no reverse override.**
`type i error` and `type ii error` are distance 1 within a tolerance of 1, so one grades as the
other — a realistic case for a Data Mining test. No divisor fixes this: the two terms differ by
exactly one character, so any tolerance at all accepts them, and a tolerance of zero at that
length would forgive nothing anywhere. A false accept is currently unfixable from
the UI, because "I was right" has no opposite.

This is recorded in a test that pins the behaviour rather than engineered around, so the
choice is visible. The alternative — a "mark wrong" action on the results screen — is a new
endpoint that mutates a `reviews` row in a second way, and Part 3 deliberately left the
override as the only such write. It is the obvious follow-up if the cost proves annoying in
practice.

### Short answers are untouched

Fuzzy matching applies to typed flashcards only. `grade_short_answer` is shared with practice
mode, so making it tolerant would silently change practice grading, which nobody asked for.

The asymmetry is principled: a short-answer card has an author-curated `accepted` list — the
mechanism for "these wordings are also right" — which the override grows over time. A
flashcard has one prose answer and no accepted list, so it is the kind that actually needs the
tolerance.

**If mock short-answers should also be spelling-tolerant, that is a separate decision**, and a
separate change, because it changes practice too.

### The empty-key edge

`answer_md` is guaranteed present and non-blank by card validation, but it can still
*normalise* to empty — `---` is markdown, not alphanumerics. The empty-key guard therefore
applies to **both** sides: an empty normalised expectation never matches anything, and an
empty normalised submission never matches. Grade incorrect, mark the review overridable, and
do not 500 on a data shape the schema permits.

---

## 5. No feedback during the run — three leaks, not one

The master spec: "In mock test mode the response withholds the verdict until `/finish`." The
obvious reading is that `/answer` needs a branch. Three endpoints leak, and only one of them
is `/answer`.

### 5a. `/answer` returns a shape incapable of holding a verdict

The weak implementation is one response type with its answer-bearing fields blanked in mock
mode. Part 3 §4 argued at length against exactly that shape for `/next`: the protection there
is that the serve type *has no field capable of holding a key*, so leakage is impossible at
the type level rather than absent by care.

`/answer` gets the same treatment: an untagged union of the existing practice response and a
mock response carrying `mode`, `answered_count` and `pool_count` — no `correct`, no `expected`,
no `explanation_md`, no `can_override`, and no field that could grow one. A future edit that
leaks the verdict has to add a field to a struct whose name says it must not.

`review_id` is omitted too, since the runner shows no verdict and the override happens from the
results screen. That is honest minimalism and **not** a security control: review ids are
sequential integers and therefore guessable. The control is §8's state gate.

Each variant carries an explicit `mode` field rather than relying on the client sniffing which
fields are present. `#[serde(untagged)]` is a *deserialisation* feature — on the way out it
just flattens the variant, so without `mode` there is no discriminator on the wire and the
client is left inferring one from absence. `SummaryResponse` already carries `mode`, so this is
the existing convention.

`#[serde(flatten)]` is used nowhere on this path, per Part 3 §4 layer 1.

### 5b. `correct_count` on `/next` is a running score

`NextResponse` carries `correct_count`. In mock mode a student compares it across two serves
and learns whether the previous answer was right — live per-question feedback, straightforwardly
against the spec. **The mock serve variant omits `correct_count` structurally**, the same way
the mock answer variant omits `correct`.

`pool_count` keeps meaning "cards in the pool" and stays constant across the run. Reporting the
*unanswered* count instead would make the runner read "3 of 7", then "4 of 6" — a progress bar
whose denominator shrinks as you work. Mock progress is `answered_count` against
`target_count`.

`/next` also gains `mode`, which is what lets each runner confirm it is the right runner
(§9). It carries no answer content already, for any kind, and that is unchanged.

### 5c. `/reveal` is a naked answer oracle

`/reveal` checks only that the card is a flashcard. In a mock session it would hand over
`answer_md` **and** `explanation_md` before the answer, on request.

Part 3 §4's honest note — "`/reveal` is a contract boundary, not a security boundary… for a
self-graded card nothing could do better, since the student is the grader" — **no longer
applies**, because in a mock test the server is the grader. So the boundary becomes real:
**`/reveal` refuses any mock session.**

The mode is checked **before** the kind, so a mock multiple-choice card and a mock flashcard
produce an identical refusal. Checking kind first would let the error message be used to probe
what kind a card is.

---

## 6. Answered exactly once is a contract, so it is enforced

Part 3 §3 deliberately allows `/answer` for any non-archived card in the session's decks,
without verifying that `/next` served it: "the window is a serving policy, not a validity
rule."

In mock mode that reasoning inverts for one specific case. "Every card exactly once" is a
promise the results screen depends on — a double-posted answer after a flaky reload would
produce two `reviews` rows, push `answered_count` past `target_count`, and duplicate a row in
the results list.

**In mock mode, a second answer for the same card is a 409.** Not a 422: it is a session-state
conflict, not a bad field. The guard runs *before* grading, so a repeat neither grades nor
writes.

This does not reopen §3's trust-boundary ruling, which was about serve *order*. Order is still
unenforced: answering out of turn harms nobody, and the check is derived from `reviews` alone,
so no new state is introduced.

---

## 7. Results are a GET, and cover every question

**Amendment to the master spec.** It says "`/finish` returns a score and every missed card with
its expected answer and explanation". Two changes.

**The data comes from a new `GET /api/sessions/:id/results`.** Four reasons a separate GET
beats extending `/finish`:

1. **Reload.** A results page rendered from a POST response has to re-POST to re-render.
   `/finish` is idempotent so it would work, but back/forward caching and double-submit
   protection make POST-to-render wrong — and the answer key would arrive as the response to a
   state-changing request.
2. **Practice does not want it.** `/finish` is shared, and the practice summary needs six
   numbers, not N questions.
3. **The leak boundary becomes one assertion on one endpoint** — `ended_at IS NULL` → 409 —
   rather than a conditional inside the terminal write.
4. **`/finish` stays byte-for-byte as it is**, so its nine existing tests are untouched.

**It is gated on `ended_at`, not on mode.** One rule, mode-independent: practice sessions get a
per-question record too. Nothing breaks, because the practice summary only renders after
`/finish` anyway, and Part 6 gets a head start.

**This is not the `GET /api/sessions/:id` Part 3 refused**, and the point deserves meeting
head-on rather than leaving for a reviewer to find. That refusal was about an endpoint
returning session state *during* a session — "the natural place for a future leak". `/results`
is the post-terminal record: unobtainable until `/finish`, which under Part 3's restated
invariant is itself an act on the session. While a session is live it returns nothing at all.

**It carries every question, not only the missed ones**, in answer order. Seeing what you got
right is part of rehearsing, and a list that omits the correct answers cannot double as a
review sheet.

Ordering is by `answered_at` then `id`, both ascending. Timestamps have one-second resolution,
so ties are the normal case, and the handover's rule is that a tiebreak must mirror the sort
direction.

Recorded honestly: **the `id` half of that tiebreak is not provable by test.** With several
reviews sharing an `answered_at`, SQLite returns them in rowid order whether or not the
tiebreak is written, so a mutation that removes it changes nothing observable. It is kept
because the resulting order is only *incidentally* correct — nothing guarantees rowid order for
an unqualified `ORDER BY`, and a future index or query-plan change could reorder a results
screen silently. The sort *direction* is proven, by a deliberately constructed tie. This
mirrors the decks list query, where the handover already documents which half of a tiebreak is
provable and why the other half stays.

Each entry carries the prompt, the image, what you gave, the expected answer, the explanation,
`correct`, `overridden`, `ms` and `answered_at`. Four fields are less obvious:

- **`review_id`.** The override takes a *review* id, and a results row otherwise carries only
  `card_id`. There is no way to derive one from the other on the client, so without this field
  §8 is dead code.
- **`can_override`**, which practice computes at answer time so the client never reimplements
  the eligibility rule. A mock run showed no verdict, so the flag travels with the question.
- **`self_grade`**, so `/results` reads correctly for a practice session too.
- **`expected` as a `Vec<String>`**, one shape for all three kinds — the correct choice texts,
  the accepted wordings primary-first, or `answer_md` — matching Part 3 §11.

**A results row carries no `choices` and no choice ids**, deliberately. All three kinds are
therefore rendered by one kind-agnostic component showing your text against the expected text,
differing only in a kind badge. So the multiple-choice list is reused verbatim during the run
and is simply not involved afterwards — which is the right outcome rather than a shortcut, as
a second choice renderer with correctness styling is exactly what the master spec's "one
rendering path" rule exists to prevent.

The summary is **nested** under its own key rather than flattened, per Part 3 §4 layer 1, and
the statistics query is extracted so `/finish` and `/results` share one cached query.

---

## 8. The override extends to flashcards, and gains a state gate

Part 3 §8 restricted the override to short-answer reviews: it does two things — flip the
review, and add the typed wording to `accepted` — and only short-answer cards have an
`accepted` list.

Auto-graded flashcards need the first without the second. §3's caveat is only tolerable
because there is a correction path.

### 8a. Eligibility is (mode, kind, state)-aware

| session mode | `ended_at` | kind | outcome |
| --- | --- | --- | --- |
| any | — | `mc_single` | 409 — the correct option is authored data |
| **practice** | any | `flashcard` | **409 — grade it again instead** |
| **mock** | **NULL** | any | **409 — submit the test first** |
| mock | set | `flashcard` | 200: flip only, `accepted_added: false`, `expected` from `answer_md` |
| mock | set | `short_answer` | 200, unchanged, including the `accepted` insert |
| practice | any | `short_answer` | 200, unchanged |

**Practice flashcards stay non-overridable.** Making the override merely kind-aware would
delete a decided invariant: in practice the student is already the grader and can simply grade
again. The existing test that refuses a practice flashcard override asserts only the status, so
it passes if and only if the gate is on (mode, kind) — it is the canary for getting this wrong.

**Multiple choice stays non-overridable in both modes.** "I was right" about which of four
authored options is correct is a claim about the card, not the answer.

### 8b. The mock-active gate is the sharpest finding in this design

`POST /api/reviews/:id/override` returns `expected`, and it distinguishes an already-correct
review with a specific 409. Extended to flashcards and left ungated, it becomes a **per-card
answer-and-correctness oracle usable during a live mock run** — the exact thing §5 closed on
three other endpoints. Review ids are sequential integers, so omitting `review_id` from the
mock answer response is not a control.

**Override refuses while the review's session is a mock with `ended_at IS NULL`**, the same
boundary as `/results`.

The check order matters as much as the check. The mock-active gate runs **before** the kind
check and **before** the already-correct check, so a live mock gets one identical refusal
regardless of kind or verdict. Either check running first would leak through its own message.

### 8c. The honest cost

Overriding a mock flashcard fixes the row but does **not** teach the card, unlike short-answer,
whose override inserts an `accepted` row that grades correct next time. Flashcard grading
compares against `answer_md`, and card validation forbids `accepted` rows on a flashcard, so
wiring flashcards into `accepted` would be a card-model change. Out of scope, and stated rather
than discovered.

---

## 9. Mock gets its own page, and `mode` is what makes the route safe

`/mock/:id` and `MockSessionPage.tsx`, rather than a mode branch inside `SessionPage.tsx`.

The reason is not line count. `SessionPage` holds five pieces of state a mock run must never
enter — the verdict, the revealed answer, the override flags and the streak — and every one is
read by the render tree. A mode branch would have to prove five negatives on every render, in
a runner with **no test coverage at all**: the frontend has no test framework, deliberately, so
the only thing between a refactor and a broken practice runner is a human clicking through it.
A separate file proves those negatives by absence.

**A separate route is not a mode guarantee, and it is worth being precise about that.** The
route tells the page what the *URL claims*. Session ids are sequential integers and the URL is
hand-editable, so `/mock/7` where 7 is a practice session would otherwise render a mock runner
over a practice session: no reveal on flashcards, `given` posted where the server wants
`self_grade`, and a results fetch that never succeeds.

The authority is `mode` on the serve payload, which is the second reason §5b added it. Each
runner redirects on its first serve if the mode is not its own — mock to `/session/:id`,
practice to `/mock/:id`. So the route lets the page optimistically render the right chrome and
`mode` makes it correct. This is the **only** change Part 5 makes to `SessionPage.tsx`, and it
gives Part 7's `sm2` its slot.

### 9a. The state machine starts with `/next`, not `/results`

On mount the page GETs `/next`. A 200 runs the test; a 409 — which covers both "the pool is
done" and "this session has ended" — POSTs `/finish` and then GETs `/results`. Every answer
GETs `/next` again, so there is one terminator and one path to the results screen.

Probing `/results` first and reading its 409 as "still running" reads better as a state machine
and is worse in the one way that matters: **it issues a request to the endpoint that carries
every answer, every expected string and every explanation, while the run is live.** It would be
safe only because the server chooses to refuse. "Never asked" is stronger than "asked and was
refused", and the stronger property is what this app's session contract is built on — the
master spec requires that no session endpoint returns correctness data before an answer is
submitted, and Part 3 proved it by watching the wire. Probing also degrades that check: a
reviewer in DevTools would have to reason about a `/results` request appearing mid-run instead
of simply confirming it never appears.

The cost, named rather than hidden: **reloading a finished test re-POSTs `/finish`.** Harmless,
since `/finish` is idempotent and preserves the original `ended_at`, and a much smaller price
than a request to the answer-key endpoint at the start of every run.

One optimisation is explicitly rejected: the client could notice it has answered
`target_count` questions and skip to `/finish` without provoking the 409. That would create a
second path to the results screen executing only on the last question of a run — the
least-exercised path in the feature — to save one round trip per test. The 409 stays the single
terminator, which is also what §11 requires once archiving has shrunk the pool.

### 9b. No Zustand, reversing Part 4 §7

Part 4's design doc deferred Zustand and named Part 5 as "the intended home", expecting mock
mode to have "a stable serve order, a `target_count`, no per-question feedback, and a resume
story that practice mode explicitly does not have — genuinely cross-component state with a real
problem for a store to solve".

**All four turned out to be server properties**, and the fourth is self-cancelling: the resume
story predicted to need a store is precisely what §2's stable serve order removed the need for.
A reload re-fetches `/next` and lands on the same card. There is nothing to persist.

Everything left is either a server-derived cache with one consumer or the form state of one
control; nothing spans sibling subtrees. A store would be a second source of truth for the
current serve — which is the client-side queue the handover's "session state lives only in
`reviews`" invariant forbids by name.

Recorded explicitly, with the prediction it reverses, so it is not re-litigated in Part 6.
Revisit only when two sibling subtrees genuinely need the same mutable value.

### 9c. A count-up clock, and where its state lives

Elapsed time only. No countdown, no auto-submit — decision 4, and early submission stays the
student's call, so `/finish` gains no completeness guard.

**The ticking state lives in the timer leaf**, not the page. Lifting it up would re-render the
prompt's markdown — `react-markdown` with KaTeX — once per second, which is worst on the 100+
card COS781 deck the handover already flags as an unverified responsiveness worry.

Four smaller rules, each a bug if reversed: the baseline is the server's `started_at`, not
mount time, so a reload continues the clock rather than restarting it (this is the third reason
`started_at` joined the serve payload); each tick recomputes from the current time rather than
incrementing, so a throttled background tab cannot drift; the value is floored and clamped at
zero, because one-second `started_at` resolution can otherwise show a negative; and it is
`aria-hidden` with no live region, because a per-second announcement is a screen-reader
firehose — the accessible progress information is the "Question 7 of 32" text.

`usePrefersReducedMotion` is deliberately **not** used here. A count-up clock is content, not
decoration, and suppressing it would remove information. No transition is added on question
change either: the global reduced-motion rule would neutralise it anyway, and "no feedback" is
easiest to honour with no motion at all.

### 9d. Enter does two jobs, which is a double-submit hazard

In practice, Enter alternates — submit while ungraded, advance once graded — so two keydowns
in one tick degrade to "advance", harmlessly. In mock, one Enter submits *and* advances, and
the in-flight flag is React state, so two keydowns in the same tick can both read it as false
and both post. Holding Enter is exactly what a keyboard-first runner invites, and Part 4's
walkthrough does it deliberately.

The guard is a ref checked and set **synchronously**, alongside the state that drives the UI.
It carries its own acceptance criterion.

### 9e. Digits are gated by kind, not by a typing check

Practice guards its digit handling with "is the event target an input". Mock does better:
digits are only ever wanted for multiple choice, and **a mock multiple-choice card has no text
input mounted at all**. Gating on the card kind makes typing interference structurally
impossible rather than defensively guarded.

The Space-to-reveal branch is **omitted entirely** rather than relying on that guard. In mock a
flashcard is a text input, so a Space branch would eat spaces out of a typed answer; practice
is one line of distance from that defect and mock should not inherit it.

### 9f. No new colour tokens

`--success` and `--destructive` exist in both palettes with their `-foreground` partners, and
both pairs are already in the contrast script's enforced tier. Reusing them means **nothing is
added to either palette and nothing is added to `check-contrast.py`** — the script should
report the same row count before and after Part 5, and an unchanged count is the evidence that
no new pair crept in.

Two things are therefore forbidden on the results screen:

- **Alpha tints** (`bg-success/10` and friends). An alpha-composited colour's contrast depends
  on the surface beneath it — precisely the class of bug Part 4 fixed, where a 70%-alpha band
  rendered white text at 2.14:1 in light mode and had survived three parts unnoticed. Light
  mode's three-layer stack also has only about 1.2:1 between adjacent surfaces, so a 10% tint
  on the card colour is invisible in Makka Pakka. And it would create a new pair needing
  hand-chosen values and new enforced rows in both themes.
- **Colour as the only signal.** Every correctness marker pairs an opaque chip with an icon and
  a text label, reusing the verdict wording so the two screens agree. Both palettes are warm —
  light `--success` is gold and `--destructive` is brick — and colour-only encoding fails for
  colour-blind users at any contrast ratio.

A left border stripe carries the at-a-glance scannability a tint was reaching for, and a border
colour carries no text, so it needs no contrast row.

---

## 10. Small contract decisions

- **`/finish` is unchanged**, including its idempotence and its lack of a completeness guard.
- **`ms` stays client-supplied** and is still rejected when negative. The clock is
  presentation; `SUM(ms)` over `reviews` remains the authority on total time.
- **Mock coverage lives in a new `backend/tests/mock.rs`**, sharing the existing test harness
  unchanged. `tests/sessions.rs` is already 1700 lines; only the two amendments below go there.
- **Two existing tests are amended, deliberately**: the one refusing mock at creation narrows
  to sm2 with reworded copy, and the practice serve's exact-key-set assertion gains `mode`.
  Neither is relaxed to a field-name check — the handover records that exact mistake ("an error
  assertion that checked only the field name while two different messages used that field").
- **Six existing tests are canaries** that must stay green untouched, each guarding a specific
  way to get this wrong: the practice-flashcard override refusal (guards §8a's mode gate), the
  per-kind field rejection (guards the practice messages), the persisted practice self-grade
  (guards Part 7's input), the practice `target_count` refusal, the
  override-after-finish test (guards against writing §8b's gate as "any active session"), and
  the `/finish` summary family (guards the shared-query extraction from changing behaviour).
- **The results assembly is a pure function** over the three result sets, mirroring
  `fold_candidate_rows`, so ordering, `expected` selection and `can_override` are unit-testable
  without a database.
- **No migration, no new column, no new table, no new dependency.** `sessions.mode` already
  permits `'mock'` and `sessions.target_count` already exists, both from `0001_init.sql`.

---

## 11. Archiving mid-test, and why `target_count` is stored

`target_count` is frozen at creation; the live pool is not. Archive a card during a test and
the pool shrinks under it, so a test can legitimately end with 31 answered out of 32. Add one
and the run can exceed it.

**Ruling: `target_count` is a record of the pool at creation, not an authority over serving.**
`/next` serves from the live pool and 409s when the live unanswered set is empty; `/results`
reports what actually happened. Using it as a serving bound would starve the run — hanging
forever on a question that no longer exists — or over-serve it.

This is also why it is stored rather than computed on read: it is the denominator the student
was promised at the start, and recomputing it would silently rewrite history to make every
abandoned-by-archiving test look complete.

§2a's rank-by-hash ordering is what makes this survivable rather than merely handled: the
remaining cards keep their relative order, so an archive mid-test shortens the run without
reshuffling it.

---

## Open questions for later parts

- **Part 6 (stats)** is the first consumer of `sessions.mode` as a filter. Practice and mock
  reviews sit in the same table, and a mock test's accuracy is a fairer measure of knowledge
  than practice's, which is weighted towards your weaknesses by construction. Whether the stats
  screen separates them is Part 6's call. `/results` already works for practice sessions, which
  gives Part 6 a per-question record for free.
- **Part 7 (SM-2)** remains the first consumer of `reviews.self_grade` and `schedule`. Mock
  flashcard reviews have `self_grade IS NULL`, so SM-2 must map them from `correct` or skip
  them.
- **A "mark wrong" action**, the missing opposite of "I was right". §4 records that fuzzy
  matching can only produce false accepts and that they are currently unfixable from the UI.
  The obvious follow-up if that proves annoying; it needs a second write path to a `reviews`
  row, which Part 3 deliberately avoided.
- **Spelling tolerance for short answers.** §4 leaves it out because `grade_short_answer` is
  shared with practice. A separate decision, not an oversight.
- **A module-wide mock test** stays unbuilt and unreachable from the UI, with the backend path
  intact and tested (§1). If COS781 revision wants one deck per lecture and a mock over all of
  them, this is the thing to build, and it is a picker rather than an API change.
- **375px phone width** is unverified across every part so far, this one included. It belongs
  to build step 8.
