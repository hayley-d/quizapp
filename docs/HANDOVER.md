# Handover

Read this first if you are picking up this project without the conversation that built it.

**Last updated:** 2026-08-30, on `feat/part8-embed-lan`.
**Build step 8 is implemented and gated green, on a branch, unmerged.** The app is now a
single binary: the React bundle is compiled in, it binds to the LAN, and it needs no
internet. **This is the last step in the spec's build sequencing — after it, the build
plan is finished.** See "Part 8" under Where things stand, and "Part 8 — verification
status" under Outstanding for what is proven and what is not.

**The standing browser gap changed hands.** Hayley has taken the Part 5, 6 and 7
walkthroughs on herself and will report defects as she finds them; agents on this machine
still cannot drive a browser (the Chrome extension is not connected — checked again on
2026-08-30). Step 8 was built alongside that rather than waiting for it, which was her
explicit call.

**Part 7 (SM-2) is done and merged.** Tasks 1-8 of the plan are complete and the branch has
landed on `main`; `docs/PART-7-HANDOVER.md` — a temporary mid-execution handover, superseded
by this document — was deleted as part of that merge. Part 7 reached
a green automated gate (below) with an **undriven** browser walkthrough — the Chrome
extension is still not connected on this machine, so all nine points of the walkthrough at the
brief's Step 5 are outstanding. See "Part 7 — verification status" under Outstanding for the
itemised list.

**Part 6 (stats) is done and merged** — `feat/part6-stats` landed on `main`, and there is no
other feature branch outstanding besides `feat/part7-sm2` above. It merged on a green automated
gate and an undriven browser walkthrough — the Chrome extension is still not available on this
machine, so the eight-point walkthrough at the end of
[`mitis/plans/2026-08-28-part6-stats.md`](mitis/plans/2026-08-28-part6-stats.md) has not been
performed. The numbers are proven by ten integration tests and two unit tests, each
mutation-checked; what is *not* proven is that they look right on screen.

**Part 5 (the mock test) is done and merged** — `feat/part5-mock-test` landed as `f19da1b`.

**Part 5 was merged on a green automated gate and an unperformed browser walkthrough**, and
that is the one thing to understand before you touch anything. Its plan's Task 17 — a
twenty-one-point walkthrough — was never driven, because the Chrome extension is not
available on this machine; Hayley reviewed the position and chose to merge anyway. Four of
those points (3, 7, 9 and 21) are answer-leak checks, and point 9 in particular can only be
settled in a browser's Network tab: the tests prove `/results` refuses mid-run, but only the
Network tab proves the client never asks. **So mock mode has never been used.** See "Part 5
— verification status" under Outstanding for the itemised list of what is and is not known,
and treat the first real mock test on a COS781 deck as the walkthrough.

Part 4 (the Bibble theme pass) is done and merged; so is Hayley's `e09e76d` styling pass,
which is a structural change as much as a styling one — it deleted the `/study` screen and
moved session starting onto the deck page, and in doing so built Part 5's entry point
before Part 5 existed. Read "The `e09e76d` styling pass" under Where things stand before
touching the frontend.

Part 2c's nine-point walkthrough is still outstanding, and 375px phone width has never
been rendered in any part — now across Parts 1, 2b, 2c, 3, 4, 5, 6 **and 7** (see
Outstanding). Part 7 adds a third figure to a stats strip that has never been seen at phone
width, which is a specific, outstanding risk rather than a hypothetical one.

## What this is

A self-hosted quiz app for exam revision, replacing Quizlet. The driving deadline is the
COS781 (Data Mining) test on **11 September 2026**. Full design: [`mitis/specs/2026-08-26-quiz-study-app-design.md`](mitis/specs/2026-08-26-quiz-study-app-design.md) — that
document is the record of what the app is meant to be, and it is kept current.

## Where things stand

Parts 1, 2a and 2b of the spec's build sequencing are **done**, and so is the deck card list
redesign that followed them ("Part 2c" below). Parts 3, 4, 5, 6 and 7 are merged to `main`;
**Part 8 is complete on `feat/part8-embed-lan` and unmerged**, and is the last step there is.
Part 7 (SM-2) landed from `feat/part7-sm2` at `85368ab`, reviewed and gated green, on an
**undriven** browser walkthrough. Concretely, working today:

### Part 8: one binary, LAN, no internet

**The app is now a single process.** `cargo build` compiles the React bundle into the Rust
binary; `QUIZAPP_BIND=0.0.0.0:3000 ./target/release/quizapp` serves the UI, the API and the
uploaded images from one origin, reachable from a phone on the same wifi, with no Vite and
no internet. This closes the deployment model the spec has described since Part 1.

- **`backend/build.rs` runs `pnpm build`.** Chosen over a cargo feature flag and over a
  committed `dist` placeholder, so there is **one build shape** and the embedded bundle can
  never be stale relative to the source. The cost is real and was accepted deliberately: a
  TypeScript error now fails `cargo build`, and a cold `cargo test` needs node and pnpm.
  `rerun-if-changed` keeps that off the common path — a backend-only edit does not pay for a
  Vite build, which was measured rather than assumed.
- **The embed reads `OUT_DIR`, not `frontend/dist` — and that indirection is the fix for a
  real failure, not architecture for its own sake.** rust-embed resolves `#[folder]` at
  macro-expansion time. Pointed straight at `frontend/dist`, deleting that directory without
  touching a watched input leaves cargo's fingerprint fresh, so the build script never reruns
  to rebuild it, and the derive fails with three screens of unrelated trait-resolution errors
  naming `icu_provider` and `SliceIndex`. **This was hit during implementation, not
  theorised.** The build script now mirrors `dist` into `OUT_DIR`, which cargo preserves
  between runs, so the same deletion is survivable. Do not "simplify" the folder back.
- **`.fallback()` must stay above `.layer()` in `lib.rs`.** Axum applies a layer only to what
  was registered before it, so moving the layer up silently leaves every frontend response
  untraced and uncompressed — no error, no test failure, just missing behaviour. There is a
  comment on it in place because naming cannot carry that.
- **`/api` got its own 404 envelope first, and that ordering was the point.** Axum propagates
  an outer fallback into `nest`ed routers, so without `api_router().fallback(...)` every
  typo'd API path would answer with `index.html` and a **200**, and the client would try to
  parse HTML as JSON. `routes/mod.rs` now ends in a fallback returning
  `AppError::NotFound("endpoint")`. Pinned by
  `the_frontend_fallback_does_not_swallow_unknown_api_paths` in `backend/tests/frontend.rs`,
  which exists specifically as the regression test for that interaction.
- **A miss under `/assets/` is a 404, not the index.** Serving `index.html` for a missing
  bundle produces "Expected a JavaScript module but the server responded with a MIME type of
  text/html", which hides the actual cause. Same reasoning gave `ServeDir` a
  `not_found_service`, so a broken thumbnail is a JSON 404 rather than a page of HTML.
- **Fonts are vendored through npm, not hand-downloaded.** `@fontsource/quicksand` and
  `@fontsource/inter`, importing exactly the six `latin-<weight>.css` faces the old Google
  Fonts `@import` declared (Quicksand 500/600/700, Inter 400/500/600). This follows the
  precedent already set for KaTeX — fonts come from the package manager, never a CDN — and
  keeps them under the lockfile instead of committing binaries nobody can diff. **The
  `@import url('https://fonts.googleapis.com/...')` at `globals.css:2` is gone**, and
  `grep` over `dist/` finds no `fonts.googleapis` or `fonts.gstatic` reference at all.
- **woff2 only, by rewrite rather than by pruning.** KaTeX and Fontsource both ship woff and
  truetype alongside woff2; that was **876 KB of the 2.1 MB bundle** for formats no browser
  this app targets will ever request. A Vite plugin (`woff2OnlyFontFaces`) strips the trailing
  `src` entries at transform time, so a KaTeX upgrade cannot silently reintroduce them —
  post-processing `dist` would have been invisible and would have broken quietly. The leading
  comma in its pattern is load-bearing: a face whose *only* `src` is woff keeps it, so the
  rewrite can never leave a face with no source at all. `dist` is now 1.4 MB.
  **Correct framing, since the first one was wrong:** this is a binary-size win, not a
  load-time one. Font files are fetched per-glyph-family on demand, so a deck with no maths
  never downloaded the KaTeX fonts anyway.
- **The chunk split needed the Rolldown API, and this is the trap most worth recording.**
  Vite 8 here depends on `rolldown`, not Rollup — there is no Rollup in the tree. **The object
  form of `manualChunks` that every tutorial shows is not supported and is silently ignored**,
  which would have produced one big chunk, a green build, and a note in this document claiming
  a split that never happened. The live API is `build.rolldownOptions.output.codeSplitting`.
  Three groups: `react` at **priority 20**, `markdown` and `dnd` at 10. The priority is not
  cosmetic — `includeDependenciesRecursively` defaults to true, so without capturing React
  first the markdown group swallows it as a dependency of `react-markdown` and every other
  chunk ends up depending on markdown just to reach React. **Leave that option at its
  default**; turning it off is what causes the circular-chunk blank page.
- **Vite's 500 kB warning is gone because it is satisfied, not because it was raised.**
  Largest chunk is `markdown` at 389.69 kB (117.74 kB gzipped), against one 906.95 kB chunk
  before. KaTeX's CSS also split out into its own `markdown-*.css`, which was *not* expected —
  the prediction was that `cssCodeSplit` only separates async chunks. It did anyway.
- **Compression is on, and the exclusion matters.** `CompressionLayer` at
  `CompressionLevel::Fastest`, with woff2 and woff excluded — they are already compressed
  containers, so recompressing spends CPU for nothing. Verified against the running binary
  rather than reasoned about: the JS asset comes back `content-encoding: gzip` at 111 KB
  against 281 KB raw, `vary: accept-encoding` is emitted without needing an extra layer, and
  a woff2 comes back with **no** `content-encoding` at all.
- **Cache headers are split three ways**, because the filenames differ in kind: `/assets/*`
  is content-hashed and gets `immutable` for a year; `index.html` gets `no-cache`, since it
  *names* those hashed bundles and a stale copy is a white screen rather than a stale pixel;
  everything else gets `must-revalidate`.
- **LAN binding needed no configuration change** — `QUIZAPP_BIND` already existed and is
  passed straight to `TcpListener::bind`. What it needed was a startup log that prints an
  address a human can type. `configuration::reachable_url` resolves the machine's actual LAN
  address by connecting a UDP socket (which sends no packet and needs no internet, only a
  route) and logs `http://192.168.101.116:3000` instead of the unopenable
  `http://0.0.0.0:3000`. Verified: the logged address matched `ipconfig getifaddr en0` and
  served the app.
- **The default bind is still `127.0.0.1:3000`, deliberately.** The app has no
  authentication, so defaulting to every interface would expose it on every network the
  laptop ever joins. Binding wide is a per-run choice. **This is a one-line change in
  `configuration.rs` if that trade is not wanted** — it was a stated assumption, not a
  finding.
- **The frontend needed no API changes at all.** `api.ts` already fetched relative `/api`
  paths and the images route already returned relative `images/<name>`, so same-origin
  serving worked with nothing rewritten and no CORS anywhere.

### Part 7: SM-2 spaced repetition

**A third session mode, drawing from `schedule`, the table every card has had a row in since
Part 1 and that no code had touched until now.** `backend/src/scheduler.rs` is a new pure
module (no database access, alongside `practice.rs`, `mock.rs` and `grading.rs`) implementing
standard SuperMemo-2: `initial_state`, `quality_for(correct, self_grade)`, `apply(state,
quality)` and `replay(qualities)`. Design:
[`mitis/specs/2026-08-28-part7-sm2-design.md`](mitis/specs/2026-08-28-part7-sm2-design.md);
plan: [`mitis/plans/2026-08-28-part7-sm2.md`](mitis/plans/2026-08-28-part7-sm2.md). **No
migration, no new table, no new column, no new dependency, and no new colour token** — every
prerequisite (the `schedule` table, `sessions.mode` already permitting `'sm2'`,
`reviews.self_grade`, the frontend's disabled tile) was seeded by earlier parts specifically
so this one would need none of that.

- **Due-ordered serving, most overdue first.** An SM-2 session serves, from the session's
  decks, non-archived cards whose schedule is due, each exactly once, ordered by `due_at`
  ascending then `card_id` — the id tiebreak makes the order total, the same reasoning as
  `mock.rs`'s `(hash, card_id)` tuple. The next card is the first card in that order with no
  `reviews` row for this session, so a reload re-serves the same card and "session state lives
  only in `reviews`" stays exactly true. Unlike mock, this needed no hash trick: `due_at` is
  already a per-card property, so ordering by it is already a function of each card, and has
  the reload-stability property natively. A missing `schedule` row (which should be
  unreachable, since every card gets one at creation) counts as due via a `LEFT JOIN`, rather
  than being hidden by an `INNER JOIN`.
- **Nothing due refuses at creation**, naming the next due date, on this document's existing
  Error handling rule that a session with no eligible cards fails clearly rather than serving
  an empty runner. Serving the soonest-due cards anyway was considered and rejected — it would
  defeat the scheduler completely, turning every session into "review everything" and making
  the intervals purely advisory. Practice mode already exists for "study now regardless."
- **Day-granular `due_at`.** Written at midnight UTC of the due day (`date(<base>, '+N
  days')`), not a seconds-exact offset, so a card answered at 21:00 with a one-day interval is
  due at 08:00 the next morning, not at 21:00 the next day.
- **A lapse leaves the ease factor unchanged — deliberately, and it is a known minor rather
  than a bug.** On `quality < 3`, `repetitions` resets to 0, `interval_days` resets to one day,
  `lapses` increments, and **the ease factor is left exactly as it was.** This is the original
  SuperMemo-2 behaviour: the ease factor is a property of the card's intrinsic difficulty, and
  a single failure is already punished by the interval reset — also dropping the ease would
  double-count one bad night and drive easy-but-forgotten cards toward the 1.3 floor they do
  not belong at. **A majority of the SM-2 implementations in circulation take the other
  branch**, so this will read as a bug to a future maintainer who knows that variant. It is
  invisible in the code — an absence, a line not written — pinned by a unit test
  (`a_lapse_resets_the_repetitions_and_counts_itself` and the ease-unchanged assertion beside
  it in `scheduler.rs`), and it must not be "fixed."
- **`repetitions` in Rust and TypeScript, `reps` in the SQL column.** Part 7 ships no migration
  deliberately (see above), so the `schedule.reps` column — named before this project's
  never-abbreviate rule existed — keeps its name rather than forcing a migration over
  hand-written cards, which is exactly what "schedule exists from day one" was sequenced to
  avoid. Every Rust and TypeScript identifier is `repetitions`; the SQL query aliases at the
  boundary, `reps AS "repetitions!: i64"` (`backend/src/routes/sessions.rs:1126`). **Recorded
  as a known minor split so a later reader does not "fix" one half of it** — renaming the
  column now would be the very migration Part 7 was sequenced to avoid, and renaming only the
  Rust/TypeScript side back to `reps` would violate the project's own naming rule.
- **The answer write is one transaction**, in SM-2 mode: the `reviews` insert and the
  `schedule` update share a transaction, so a failed schedule write rolls back the review
  rather than leaving it recorded against a schedule that never advanced (pinned by
  `a_failed_schedule_write_rolls_back_the_review`, which drops the `schedule` table mid-write).
  `due_at` is computed from the new review's own `answered_at`, never `now`. **This transaction
  was incidentally widened to cover all three modes' `reviews` insert, not only sm2's** — the
  minimal way to make the sm2 write atomic without a second code path for it alone, accepted
  at review.
- **The override replays the schedule.** `POST /api/reviews/:id/override`, when the
  overridden review's session is `sm2`, follows the flip with a replay of every sm2 review for
  that card, in `answered_at, id` order, through `scheduler::replay` — recomputing `schedule`
  rather than leaving it at the lapse the override just corrected. `apply` and `replay` are
  tested to agree (a fold of `apply` from `initial_state` must equal one call to `replay`), so
  the answer path and the override path cannot silently diverge. `due_at` is based on the
  *last replayed review's* `answered_at`, not `now`, so an override performed the next day does
  not push the card a further day out.
- **An sm2 flashcard override stays refused, on the existing check, unedited.** The `answer`
  handler's inline `can_override` predicate (not `can_override_result`, which remains correct
  for `/results`) already refuses to override a flashcard outside mock mode, and its condition
  already covers `sm2` with no edit — `can_override_result` returns `true` for an incorrect
  flashcard and would break the pinned test at `backend/tests/sessions.rs:1023` if the handler
  used it instead. In SM-2 a flashcard is self-graded, so the student's own verdict *is* the
  grade, and overriding your own verdict is incoherent — you re-grade a flashcard, you do not
  override it. The refusal message ("Grade the flashcard again instead of overriding it")
  becomes slightly misleading under SM-2, since the card will not come back later in the same
  session; left as a known minor rather than reworded.
- **`schedule_for` was not replaced.** The plan's own text said to widen this test helper; doing
  so would have silently gutted `backend/tests/cards.rs:267`, which destructures its 2-tuple
  and asserts the row count. A separate `schedule_state_for` was added alongside it instead,
  and Part 5's mock canary (`backend/tests/mock.rs`, "mock mode must leave the sm-2 schedule
  alone") was pointed at the wider helper, so it now also proves `interval_days`, `ease`,
  `repetitions` and `lapses` are untouched in mock mode, not only that a row exists.
- **The stats strip grows a third figure — the answer to Part 6 §10, recorded there in
  place.** `DeckStatsSummary` gains `sm2_accuracy` / `sm2_review_count` (the third strip
  figure, a third `mode = 'sm2'` bucket alongside practice and mock) plus `due_count` /
  `next_due_at` (tile data, not strip figures — they enable, disable and label the
  Spaced-repetition tile). `load_card_stats`, the per-card miss rate, needed no change: it
  already pools all modes. A mode with no reviews still reads `—`, never `0%`.
- **`SessionPage.tsx` is reused, not forked, unlike mock.** Mock got its own page because a
  mock run must never enter five pieces of practice's state (verdict, revealed answer, two
  override flags, streak); SM-2 needs all five — same verdict, same reveal, same four
  self-grades, same override, same streak, just a different pool. `served` widens to
  `PracticeNextResponse | Sm2NextResponse`; the header becomes mode-aware (`n of m due` for
  SM-2 against `n in the pool` for practice). Part 5's mode-mismatch redirects already handle
  `sm2` falling through them with no edit — this is the slot Part 5 said it was leaving.
- **A decision made during execution, worth recording so it is not reverted:** the plan's own
  Task 7 code left the Spaced-repetition deck tile enabled while `due_count` was still loading,
  because `deck` and `deckStats` load in parallel and the tile's guard originally checked only
  `deck`. That contradicted the task's own acceptance criterion. Escalated, and the ruling was
  that the criterion governs: `DeckPage.tsx`'s `isDisabled` now treats the tile as disabled
  while `dueCount` is `null` (still loading) as well as when it is `0`.
- **A fragility worth naming, not yet a bug:** `NextResponse` (`backend/src/routes/sessions.rs`)
  is `#[serde(untagged)]` and derives only `Serialize`, so there is no ambiguity today. If
  `Deserialize` is ever added, the three variants still discriminate by required-field presence
  (Practice has no `target_count`; Mock requires `started_at`; Sm2 requires `correct_count`) —
  but untagged deserialization becomes a real hazard worth a second look at that point.
- **No new colour token.** `check-contrast.py` still reports 16 ENFORCED rows with an empty
  RECORDED tier — an unchanged count is the evidence that Part 7 introduced no new colour pair.
- **`pool_count` means something different in sm2.** `sessions.rs:731` sets it from
  `count_due(...)`, which counts cards due *right now*, so in sm2 it shrinks as the session
  progresses, unlike practice and mock where it is a fixed denominator. Nothing is broken —
  the frontend only reads it as a dead fallback, since `target_count` is always present for
  sm2 — but the field invites a future reader to treat it as a denominator that counts down
  toward zero.
- **A long-lived sm2 session can exceed its promised `target_count`.** `load_next_due_card_id`
  re-evaluates "due now" on every serve, so a session created one day with `target_count = 3`
  and resumed the next after a fourth card matured will serve that fourth card and read "4 of 3
  due". Low probability, since the deck page always creates a fresh session. Bounding the serve
  to the cards due at creation is a bigger change than it is worth now.
- **"Next due" has two definitions.** `sessions::next_due_at` uses an INNER JOIN with no due
  filter; the stats query uses `MIN(due_at)` over a LEFT-JOINed CTE that includes
  already-overdue cards. Both actually compute "earliest `due_at`", not "next *future* due".
  They agree only because both are read exclusively when nothing is due.
- **An open question for the browser walkthrough, which has not been driven.**
  `SessionPage.tsx:284-289` renders the sm2 header as `{answeredCount} answered · … ·
  {served.answered_count} of {target} due`, and `served.answered_count` moves in lockstep with
  `answeredCount` — so it reads "3 answered · 2 correct · 67% · 3 of 5 due", printing the same
  number twice. The mock page instead shows `Math.min(served.answered_count + 1,
  totalQuestions)`, the card you are *looking at*. The behaviour has been left unchanged — the
  UI has never been rendered and changing it blind risks making it wrong in the other
  direction — but it is worth a look when someone finally drives the walkthrough.

**The browser walkthrough is undriven** — see "Part 7 — verification status" under
Outstanding for the full itemised list; do not read the green gate below as covering it.

### Part 6: stats

**Deck statistics live on the deck page, and `/stats` is gone** — route, nav entry and
`frontend/src/pages/StubPage.tsx` all deleted. The deck page already lists every card, so a
global weakest-cards table would have been a second rendering of a list a few pixels away,
kept in sync by hand and free to disagree with it. The cost, accepted: with several decks you
cannot see at a glance which is worst without opening each. A league table is a small addition
on top of the endpoint if that becomes annoying.

**One endpoint, `GET /api/decks/:id/stats`**, in `backend/src/stats.rs` with the handler on
`routes/decks.rs`. It returns a `summary` (card count, unseen count, mock and practice accuracy
with their review counts, last answered timestamp) and a `cards` array of `{card_id,
attempt_count, miss_rate}`. `fetch_one` in `decks.rs` is what produces the 404, so the stats
query never has to tell "deck missing" from "deck empty".

**Mock and practice accuracy are two figures, deliberately.** Practice over-serves the cards
you are weak on — that is what `MISS_RATE_WEIGHT` is for — so any aggregate over practice
reviews is pessimistic by construction, while a mock test's is an unbiased sample. Pooling
them gives a third number describing neither. The per-card miss rate *does* pool both modes: a
card's own hit rate is not biased by how often it was served.

**Weakness is `practice.rs`'s `weighted_miss_rate`, reused rather than redefined** — the last
10 reviews, 0.7 recency decay. Two definitions of "weak" in one codebase would drift
invisibly: the screen would rank one card worst while the practice run served another, with
nothing to say which was wrong. `fold_candidate_rows` is reused with it. The *SQL* is not
shared: `load_candidates` is session-scoped across a deck list and has no use for the mode
split, and generalising it would couple card-serving to a read-only garnish.

**Three payload rules, each tested:** archived cards are excluded entirely (they will never be
served again, so counting them would put coverage permanently out of reach); `overridden`
reviews count as correct, since `reviews.correct` is the column the override endpoint flips;
and cards with no reviews are omitted from `cards` rather than sent at zero — absence *is* the
unseen signal.

Design record: [`mitis/specs/2026-08-28-part6-stats-design.md`](mitis/specs/2026-08-28-part6-stats-design.md).
Its §10 leaves Part 7 one question: SM-2 is a third sampling rule landing in the same table,
so does the strip grow a third figure or do SM-2 reviews fold into an existing one?


**Light and dark are now two separate visual identities, not one palette rendered twice.**
Dark mode is Bibble, unchanged. Light mode was repaletted from a pale-aqua rendering of the
same tokens into "Makka Pakka" — warm stone and sand neutrals with ochre-brown and clay
accents. Design record:
`.mitis/sdd/2026-08-28-part4-bibble-theme/makka-pakka-palette.md` — **which does not exist on this machine.** `.mitis/sdd/` is untracked and local, so this design record is either lost or never left the session that made it. The token values themselves are in `frontend/src/styles/globals.css` and mirrored in `frontend/scripts/check-contrast.py`; the reasoning survives only in the Part 4 bullets below.
The practical consequence: a new colour now needs a value in *both* identities, not one
shared value. A token that reads correctly in one theme may be meaningless or wrong in the
other — do not assume a value carries over.

- Cargo workspace: root manifest, Rust package in `backend/`, React app in `frontend/`
- All eight tables from the data model, in one migration (`backend/migrations/0001_init.sql`)
- `AppError` envelope on every failure, including malformed request bodies
- `GET|POST /api/modules`, `DELETE /api/modules/:id`; `GET|POST /api/decks`,
  `GET|PATCH|DELETE /api/decks/:id`, `GET /api/decks/:id/deletion-impact`
- `GET /api/decks` supports server-side name search (`q`), module filter (`module_id`) and
  date sort (`sort`)
- `GET|POST /api/cards`, `GET|PATCH|DELETE /api/cards/:id`, `POST /api/cards/:id/archive`,
  `POST /api/cards/:id/unarchive` — all three card kinds, per-kind validation in Rust, the
  card plus its children plus a `schedule` row written in one transaction, PATCH a full
  replace that clears both child tables first
- `accepted.normalised` computed on write (`backend/src/normalise.rs`): NFKC, lowercase,
  non-alphanumerics to spaces, whitespace collapsed and trimmed — the comparison key for
  short-answer grading
- `/decks` screen: flat card list with module badges, a search/filter/sort toolbar,
  debounced input and a stale-response guard
- `/decks/:id` screen: the deck as a list of flippable cards. Click or Enter on a card body flips
  it (a half-flip: rotate to edge-on, swap the single mounted face, rotate back — a two-faced 3D
  flip would need a fixed row height, which the unclamped markdown prompts rule out) and the
  answer is fetched per row on first flip via `GET /api/cards/:id`. Multiple-choice backs show a
  two-column grid whose options stay uniform until the eye button reveals the correct one. Rows
  drag to reorder by their grip (`@dnd-kit/sortable`, keyboard reorder included), and the order
  persists. Archiving is unchanged; the show-archived toggle was later removed by
  `e09e76d`. Design:
  [`mitis/specs/2026-08-27-deck-card-list-redesign-design.md`](mitis/specs/2026-08-27-deck-card-list-redesign-design.md)

  **Manual verification outstanding.** The nine-point browser walkthrough for this screen
  (flip, drag reorder, keyboard reorder, reduced-motion, the image lightbox) has **not been
  performed** — the dev-tools browser could not reach the dev server in this environment. The
  code is complete and the automated gate is clean, but nothing on this screen has actually
  been clicked. Do not assume it works until someone has driven it in a browser.
- `/cards/new?deck_id=` and `/cards/:id/edit`: a keyboard-first editor for all three kinds,
  with a `ChoicesEditor` and an `AcceptedEditor`
- `POST /api/images`: multipart upload, magic-byte type check (PNG/JPEG/WebP), 5 MiB cap,
  content-addressed filenames (`images/<16 hex>.<ext>`) written to `data/images/` and served
  read-only at `/images`. Standalone rather than card-scoped, deliberately — see the Part 2b
  spec §1. Orphan files from an abandoned upload are accepted and nothing sweeps them.
- `image_path` on card create and PATCH, validated against the shape the upload endpoint
  issues, under the existing cards full-replace rule
- One `<Markdown>` component (`react-markdown` + `remark-math` + `rehype-katex`, KaTeX fonts
  bundled locally) rendering the card list, the editor preview and, later, the session runner
- The deck's card list renders full multi-line markdown per row, with an image thumbnail that
  opens a lightbox
- The card editor uploads an image while you write, and toggles the whole form between Edit
  and Preview with `⌘/Ctrl+P`
- `cards.position`: a dense 0-based order per deck (migration `0002`), backfilled from the
  `created_at` ordering the list used before, with archived cards keeping their slots.
  `GET /api/cards` orders by it. `POST /api/cards/:id/move` takes `{"before": id|null}` — land
  immediately before that card, or at the end of the deck — and rewrites the deck's positions in
  one transaction without touching `updated_at`. It is relative rather than a whole-deck
  permutation because the deck screen can be filtered, so the client cannot honestly send a
  complete order.
- 317 backend tests as of the Part 6 merge (the count of all lib and integration test binaries
  combined; 229 before Part 5, and an earlier figure of 119 recorded here was stale twice
  over). **On `feat/part7-sm2` at `85368ab` the observed count is 364** — see "Part 7 —
  verification status" under Outstanding for the full per-suite breakdown; that count is what
  was actually run for this update, not carried forward from any other document. No frontend
  test framework — that is a deliberate spec decision, not an omission, and it is load-bearing
  in Part 5's design: it is *why* mock mode got its own page rather than a mode branch inside
  the practice runner.

- **Part 3, practice mode** — `grading.rs` and `practice.rs` (two pure modules, no database
  access, randomness injected as a `roll: f64`), `routes/sessions.rs` with six endpoints, and
  two screens. Design and rationale:
  [`mitis/specs/2026-08-27-part3-practice-mode-design.md`](mitis/specs/2026-08-27-part3-practice-mode-design.md);
  plan: [`mitis/plans/2026-08-27-part3-practice-mode.md`](mitis/plans/2026-08-27-part3-practice-mode.md).
  - `POST /api/sessions` — expands a module to its decks, refuses at creation when the pool
    is empty, rejects `target_count` rather than ignoring it
  - `GET /api/sessions/:id/next` — weighted sample, choices shuffled per serve, **no answer
    content for any kind**, plus `pool_count`/`answered_count`/`correct_count`
  - `POST /api/sessions/:id/reveal` — flashcard only; 409 for the two graded kinds
  - `POST /api/sessions/:id/answer` — `{card_id, given | choice_id | self_grade, ms?}`
  - `POST /api/sessions/:id/finish` — idempotent, `accuracy` null when nothing was answered
  - `POST /api/reviews/:id/override` — the only write that mutates a `reviews` row
  - `/study` picks mode and decks; `/session/:id` is the keyboard-first runner, **unthemed**
    pending Part 4. `/study` was deleted by `e09e76d` and its job moved onto `/decks/:id`;
    the runner is unaffected.
- `migrations/0003_review_self_grade.sql`: `reviews.self_grade`, nullable, CHECK-constrained
  to the four flashcard grades, with `correct` derived (`again` → 0, the rest → 1)
- `CLAUDE.md` rule 3, never use `any` in TypeScript. Prose-only —
  `typescript/no-explicit-any` is **not** in the oxlint config and `pnpm lint` is not in the
  gate, so nothing enforces it mechanically yet.

- **Part 4, the Bibble theme pass** — merged to `main` as `60575c8`. The automated gate is
  green (see The verification gate), and both palettes have since been driven in a browser
  (see The `e09e76d` styling pass below).
  - A Light/Dark/System theme toggle in the header, persisted to `localStorage`. The `.dark`
    class on `<html>` remains the single source of truth — `sonner.tsx` still observes it,
    unmodified. The inline script in `index.html` reads the stored preference before first
    paint to avoid a flash of the wrong theme; that duplicates logic already in
    `useTheme.ts`, deliberately, because a module import cannot run before first paint.
  - An opaque `--brand` token replaces the 70%-alpha `--deck-card-header` on the `brand`
    button variant. **This fixed a real WCAG failure**: white text on the button measured
    **2.14:1 in light mode** (AA needs 4.5:1, and 3:1 even for large text), because the
    alpha let the pale page background show through. It was 4.88:1 in both themes at the time
    (the Makka Pakka repalette below later split light `--brand` to its own value). The
    `brand` variant is the app's primary action everywhere — "Start practising", "Check",
    "Next card", the deck edit button, every card-row icon button — so in
    light mode the main call to action was close to illegible, and had been since Part 1. It
    went unnoticed because until Part 4 there was no way to switch themes without visiting
    macOS System Settings.
  - `frontend/scripts/check-contrast.py`, which computes these ratios from the token values
    and exits non-zero below 4.5:1. It is now in the gate. Verified able to fail:
    substituting the old rendered light-mode value reports 2.14:1 and exits 1.
  - A CSS sparkle burst on a correct answer and a wing-flutter on a streak of 3+, both pure
    `@keyframes` with no JS on the answer/advance path — so the spec's "neither blocks
    advancing to the next question" is structural rather than something an implementer must
    be careful about.
  - The streak is client-side React state and resets on reload, preserving the "session
    state lives only in `reviews`" invariant. An override extends the streak (because
    `correct_count` and the accuracy figure already treat it as correct) but does not replay
    the burst.
  - A global `@media (prefers-reduced-motion: reduce)` rule plus a shared
    `usePrefersReducedMotion` hook. Two layers deliberately: the CSS is a fail-safe net for
    anything a later part adds and forgets; the hook exists because `useFlip` needs a
    *different code path*, not a shorter duration — a zero-duration rotation would strand
    the card edge-on at 90° with no callback to finish the swap. `useFlip` is now also
    reactive to the setting, which it previously was not (it sampled `matchMedia` at flip
    time and never subscribed).
  - Card surfaces (`rounded-xl border bg-card p-N shadow-sm`) across the runner, `/study`
    (since deleted), the `/decks` toolbar and the card editor form.
  - `SessionPage.tsx` shed its summary and exhausted screens into `components/session/`
    (417 → 359 lines).
  - Two Part 2c defects fixed: markdown links inside a card no longer flip it (the mouse
    path had no target check while the keyboard path did), and the card row's accessible
    names now carry the prompt instead of announcing only "Show answer" or reading raw
    markdown syntax aloud.
  - **The Makka Pakka repalette** (commit `df3b138`, after Part 4's own gate had already gone
    green): light mode stopped being a pale-aqua rendering of the Bibble tokens and became its
    own warm stone/sand/ochre identity. Dark mode did not change. Design record:
    `.mitis/sdd/2026-08-28-part4-bibble-theme/makka-pakka-palette.md` — **which does not exist on this machine.** `.mitis/sdd/` is untracked and local, so this design record is either lost or never left the session that made it. The token values themselves are in `frontend/src/styles/globals.css` and mirrored in `frontend/scripts/check-contrast.py`; the reasoning survives only in the Part 4 bullets below.
    - `--brand` is now per-theme rather than one opaque value shared by both. This is not a
      reversal of the reasoning above — an opaque colour's contrast still does not depend on
      its backdrop, so one value still serves both themes *within an identity*. Two identities
      now means two values: light `--brand` is `oklch(0.47 0.075 68)` at 5.90:1, dark is
      unchanged at 4.88:1.
    - `--deck-card` was decoupled from `--primary` in light mode. It used to be
      `--deck-card: var(--primary)`; the repalette made that combination
      cream-text-on-tan-card at **1.22:1, invisible**, so light mode now has its own
      `--deck-card` values: a tan `#e3d5ca` body with a taupe `#d5bdaf` header band, matching
      `--card` so deck cards are not a one-off surface. Two new tokens,
      `--deck-card-foreground` and `--deck-card-chip-foreground`, carry the per-theme text
      colours. The light palette went through two revisions on 2026-08-28 after the user saw
      the numbers — body and band were inverted, then the whole stack was shifted one step
      warmer and darker. It settled as a three-layer stack: page `#f5ebe0`, card `#e3d5ca`,
      recessed `#d5bdaf`.
      `frontend/src/components/DeckCard.tsx` changed for the first time in this whole branch
      — exactly two swaps, `text-primary-foreground*` and `text-white` to those two new
      tokens. Nothing else in that file moved.
    - This also resolved a Part 4 finding rather than leaving it deferred: light
      `--primary` + `--primary-foreground` was 3.24:1 (see the removed entry under Part 4
      deferred minor findings, and The verification gate). Light `--primary` is now
      `oklch(0.47 0.075 68)` at 5.90:1, so `check-contrast.py`'s RECORDED/KNOWN tier is
      correspondingly empty and the pair moved to ENFORCED.
    - **This palette has now been looked at.** Hayley drove both Makka Pakka light and
      Bibble dark on 2026-08-28 and made `e09e76d` in response. The ratios above remain
      the only *proof* of legibility — arithmetic is still not observation, and a passing
      ratio says nothing about whether a palette looks good — but the palette is no longer
      unseen, and the surface-separation worry that used to be recorded here was the thing
      that pass went and checked.
  - `strict: true` and `typescript/no-explicit-any` are now enforced, both verified
    load-bearing with isolated probes, and `pnpm exec oxlint` was added to the gate — a lint
    rule the gate never runs would enforce nothing.

- **The `e09e76d` styling pass** — Hayley's own commit, made after the Part 4 merge while
  looking at the running app. Its message says "fix styling"; it is a structural change,
  and this bullet exists because nothing else in the repository records it.
  - **`/study` is gone.** `frontend/src/pages/StudyPage.tsx` was deleted, its route
    removed from `App.tsx` and its link removed from the `AppShell` nav. Every "Back to
    study" now reads "Back to decks" (`SessionPage.tsx`, `session/SessionExhausted.tsx`),
    and `session/SessionSummary.tsx` lost its "Study again" button, leaving one action.
    Any older note in this document or the spec that sends you to `/study` is stale.
  - **Session starting moved onto the deck page.** `/decks/:id` now opens with a
    three-button grid built from `TEST_TYPE_OPTIONS` in `frontend/src/pages/DeckPage.tsx`:
    Practice (live), Mock test (disabled at the time, "Arrives in part 5." — **enabled by
    Part 5**) and Spaced repetition (disabled, "Arrives in part 7."). `startSession` calls
    `api.createSession({ mode, deck_ids: [deckId] })` and navigates to `/session/:id`, or,
    since Part 5, to `/mock/:id` for a mock session.
    Buttons disable when `deck.card_count === 0`, which is the right guard —
    `card_count` already excludes archived cards (`backend/src/routes/decks.rs`).
  - **This meant Part 5's entry point already existed**, and Part 5 duly enabled the tile
    rather than inventing a screen. Same for SM-2 in build step 7.
  - **Multi-deck and module-wide sessions are no longer reachable from the UI.**
    `startSession` always sends exactly one deck. The backend and the client types still
    support both a list of decks and a whole module (`frontend/src/lib/api.ts`,
    `backend/src/routes/sessions.rs`), and that path is still tested — it is intact and
    unused, not removed. **Part 5 did not restore it** — a mock test is the one deck you
    started it from. A wider pool is a picker, not an API change; see Next up.
  - **The "Show archived" switch was removed** and the list fetch is hardcoded to
    `archived: 'false'`. Deliberate — see the entry under Known-and-accepted minors for
    what it costs.
  - **The theme toggle names the palettes.** Its options now read "Makka Pakka (light)"
    and "Bibble (dark)", drawn with two hand-made SVGs in
    `frontend/src/components/icons/` rather than lucide's `Sun` and `Moon`. `Monitor`
    still marks System.
  - Two manual layout offsets on the deck page, `pl-11` on the container and `-ml-7` on
    the card list, aligning the card rows' drag grips outside the content column.

- **Part 5, the mock test** — merged to `main` as `f19da1b`; the implementation is the
  single commit `f7f6c67`. **Merged without its browser walkthrough** — see Outstanding.
  Design: [`mitis/specs/2026-08-28-part5-mock-test-design.md`](mitis/specs/2026-08-28-part5-mock-test-design.md);
  plan: [`mitis/plans/2026-08-28-part5-mock-test.md`](mitis/plans/2026-08-28-part5-mock-test.md).
  No migration, no new column, no new table, no new dependency, and no new colour token.
  - **A mock test is one deck, and the whole deck** — every non-archived card exactly once.
    There is no length picker: `target_count` is computed by the server as the pool size at
    creation, and a client-supplied one is still rejected rather than ignored. A revision
    deck is already the set of things you decided are worth knowing, so sampling a strict
    subset of it tests a random half of your own syllabus.
  - **The stable serve needed no new storage.** The next card is the first card, in a fixed
    per-session order, with no `reviews` row for this session. Both halves come from data
    already stored, so a reload re-serves the same card and "session state lives only in
    `reviews`" gets *stronger* rather than weaker — it goes from "every input is derived
    from it" to "the output is identical".
  - **`backend/src/mock.rs` ranks by hash, deliberately not Fisher–Yates.** A shuffle is a
    function of *the list*, so archiving one card mid-test would reshuffle every remaining
    card and a reload would then serve a different question — the exact defect the stable
    serve exists to prevent. Rank-by-hash is a function of each card, so an archive
    shortens the run without reordering it. `mix64` is written locally rather than taken
    from `rand` because `StdRng`'s output is not a stability guarantee across versions.
    The sort key is the tuple `(hash, card_id)`, so the order is total even on a hash
    collision and determinism does not rest on sort stability.
  - **Choice order is seeded in mock mode too**, from `(session.id, card_id)`, not
    randomised per serve. A re-randomised order would be a second reload tell, and
    remembering the order client-side is the forbidden client queue.
  - **A mock flashcard is typed and auto-graded** against `answer_md`, with no reveal step
    and no self-grade buttons — revealing the answer *is* feedback. Practice flashcards are
    completely unchanged, which is not politeness about compatibility: Part 7 (SM-2) is the
    first consumer of `reviews.self_grade` and needs the four levels. **A mock flashcard
    review therefore has `self_grade IS NULL` and carries its verdict in `correct`, so Part
    7 must map those through `correct`, not through the grade table.**
  - **Spelling tolerance, flashcards only:** `tolerance(n) = min(n / 8, 2)` over the
    *normalised expected* answer, applying only while both sides are ≤ 120 characters.
    The divisor is 8 rather than 6 because the errors are asymmetric — a false reject is
    one click from fixed, a false accept is permanent — and divisor 6 concretely graded
    `ridge` correct against `bridge`. `grade_short_answer` is untouched, because it is
    shared with practice and nobody asked for practice grading to change.
  - **`GET /api/sessions/:id/results`** serves the post-run record: every question in
    answer order, not only the missed ones. It is gated on `ended_at IS NOT NULL` — 409
    while live — and on **state, not mode**, so practice sessions get a per-question record
    too and Part 6 starts with one for free. `/finish` is byte-for-byte unchanged.
  - **Three answer leaks were closed, not one.** `/answer` returns a *separate struct* in
    mock mode carrying only `mode` and two progress counts, so it is structurally incapable
    of holding a verdict rather than conditionally nulling one; `/next` omits
    `correct_count`, which is a running score; and `/reveal` 409s for a mock session,
    because it is a naked answer oracle.
  - **The sharpest finding in the design: the override is an oracle too.**
    `POST /api/reviews/:id/override` returns `expected` and distinguishes an
    already-correct review with its own 409. Extended to flashcards and left ungated, it
    becomes a per-card answer-and-correctness oracle usable *during* a live mock run —
    review ids are sequential integers, so omitting `review_id` from the mock answer
    response is not a control. It now refuses while the review's session is a mock with
    `ended_at IS NULL`, and **the check order is part of the fix**: the mock-active gate
    runs before both the kind check and the already-correct check, so a live mock gets one
    identical refusal and neither of the other checks leaks through its own message.
  - **Mock got its own page** (`/mock/:id`, `MockSessionPage.tsx`) rather than a mode branch
    in `SessionPage.tsx`. Not for line count: `SessionPage` holds five pieces of state a
    mock run must never enter — the verdict, the revealed answer, the two override flags and
    the streak — and every one is read by the render tree. A mode branch would have to prove
    five negatives on every render in a runner with **no test coverage at all**. A separate
    file proves them by absence.
  - **A separate route is not a mode guarantee.** Session ids are sequential and the URL is
    hand-editable, so the authority is `mode` on the serve payload: each runner redirects on
    its first serve if the mode is not its own. This is the **only** change Part 5 makes to
    `SessionPage.tsx`, and it gives Part 7's `sm2` its slot.
  - **The clock ticks in the timer leaf**, not the page — lifting it up would re-render the
    prompt's `react-markdown` + KaTeX once per second, worst on exactly the 100+ card COS781
    deck already flagged as an unverified responsiveness worry. Its baseline is the server's
    `started_at` (the third reason that field joined the serve payload), so a reload
    continues the clock; each tick recomputes from the current time rather than
    incrementing, so a throttled background tab cannot drift; and it is `aria-hidden`, since
    a per-second announcement is a screen-reader firehose — "Question 7 of 32" is the
    accessible progress information. `usePrefersReducedMotion` is deliberately **not** used
    here: a count-up clock is content, not decoration.
  - **Enter does two jobs in mock mode**, which is a double-submit hazard practice does not
    have — in practice two keydowns in one tick degrade harmlessly to "advance", but in mock
    one Enter submits *and* advances. The guard is a `useRef`
    (`submitting` in `MockSessionPage.tsx`), not React state, because two keydowns in the
    same tick would both read state as `false` and both post.
  - **No new colour tokens.** The results screen reuses `--success` and `--destructive`,
    already enforced in both palettes, so `check-contrast.py` still reports **16 ENFORCED
    rows with an empty RECORDED tier** — an unchanged count is the evidence that no pair
    crept in. Two things are forbidden there as a result: alpha tints (`bg-success/10` and
    friends), whose contrast depends on the surface beneath them, which is the class of bug
    Part 4 fixed and which would be invisible anyway against light mode's ~1.2:1 surface
    steps; and colour as the only signal, so every correctness marker pairs an opaque chip
    with an icon and a text label, with a left border stripe carrying the scannability a
    tint was reaching for.
  - **No Zustand — this reverses a prediction, and the reversal is the point.** Part 4's
    design doc deferred Zustand and named Part 5 as "the intended home", expecting mock mode
    to have "a stable serve order, a `target_count`, no per-question feedback, and a resume
    story that practice mode explicitly does not have". **All four turned out to be server
    properties**, and the fourth is self-cancelling: the resume story predicted to need a
    store is precisely what the stable serve order removed the need for. A store would be a
    second source of truth for the current serve — the client-side queue the "session state
    lives only in `reviews`" invariant forbids by name. Recorded here so Part 6 does not
    re-litigate it. Revisit only when two sibling subtrees genuinely need the same mutable
    value.
  - **`target_count` is a record, not an authority.** It is frozen at creation while the
    live pool is not, so a test can legitimately end at 31 answered of 32 after an archive.
    `/next` serves from the live pool and 409s when the live unanswered set is empty. It is
    stored rather than recomputed on read because it is the denominator the student was
    promised at the start; recomputing would silently rewrite history to make every
    abandoned-by-archiving test look complete.

`/stats` is still a placeholder page.

## Next up

**Merge `feat/part8-embed-lan`, and check the phone.** Step 8's code is complete and the
automated gate is green, but four of its changes are layout claims that no one has looked
at — see "Part 8 — verification status" under Outstanding. The check is short and needs a
phone rather than a walkthrough script:

    SQLX_OFFLINE=true cargo build --release
    QUIZAPP_BIND=0.0.0.0:3000 ./target/release/quizapp

Open the LAN URL it logs, on a phone, on the same wifi, and look at four things: the deck
page's left margin, the stats strip's five parts wrapping, the four self-grade buttons
under a revealed flashcard, and the header's theme toggle. Each is named with its file and
the reasoning behind it under Outstanding.

**Still outstanding, and now Hayley's:** the Part 5 (21 points), Part 6 (8 points) and
Part 7 (9 points) walkthroughs, plus Part 2c's nine. The gate cannot see a leak and it
cannot see a layout, only a type error. Part 5's answer-leak checks (points 3, 7, 9, 21)
remain the sharpest of these — only the Network tab proves the client never *asks* for
`/results` mid-run.

**After that, the build sequencing is complete.** There is no step 9. What is left is
whatever COS781 revision actually turns up: the deferred minors listed under
Known-and-accepted minors, a module-wide session picker if one deck per lecture proves
annoying (the API already supports it; no button starts one), and real cards.

**Part 6's one open question was answered by Part 7**, in its design spec §10 in place: the
strip grows a third figure for SM-2, on the same sampling-differs argument that split mock
from practice.

### The three things Part 5 had to resolve — and how it did

All three are closed. Recorded here rather than deleted, because each answer is a constraint
on later parts.

- **`/next` re-rolling on reload.** Resolved by *dissolving* it rather than fixing it: serving
  every card exactly once in a per-session deterministic order means the next card is the
  first unanswered card in that order, so a reload lands on the same card because nothing
  about the decision changed. No "served" table, no client queue, no new column. Practice is
  unchanged and its re-roll is still correct there.
- **What a flashcard means in a mock test.** Resolved by making it typed and auto-graded
  against `answer_md`, with no reveal and no self-grade. Practice flashcards keep their four
  self-grades because Part 7 needs them. See the prose caveat under Known-and-accepted minors
  — this answer has a real cost.
- **What a mock test is scoped to.** Resolved as **one deck**, matching the button that starts
  it. This is a decision about which buttons exist, not a narrowing of the API: the backend
  still accepts a deck list and a `module_id` for a mock session, and that path is still
  tested. A module-wide mock is a picker, not an API change, and is the thing to build if
  COS781 revision wants one deck per lecture and a mock over all of them.

## Running it

```bash
cargo run                    # API on http://127.0.0.1:3000
cd frontend && pnpm dev      # UI  on http://localhost:5273
```

Full setup, env vars, the sqlx workflow and DBeaver access are in [`../README.md`](../README.md).

## Environment quirks that will otherwise cost you an hour each

- **Port 5273, not 5173.** 5173 is permanently occupied by an unrelated project on this
  machine.
- **The browser tooling reaches the dev server on the LAN IP, not on `localhost`.** This is
  what blocked the walkthroughs for Parts 1, 2a, 2b and 2c, each recorded as "the dev-tools
  browser could not reach the dev server". Two separate problems, both fixable:
  1. `pnpm dev` binds to `localhost` only, which resolves to IPv6 `::1`. Chrome asks for
     IPv4 `127.0.0.1` and gets nothing. Start it as `pnpm dev --host 0.0.0.0`.
  2. Even then the Chrome instance is not on this machine's loopback. Navigate to the
     **Network** address vite prints (e.g. `http://192.168.2.161:5273`), not `localhost`.

  With both, Part 3's walkthrough drove cleanly. Do this before concluding the browser
  cannot reach the app.
- **pnpm, not npm.** `packageManager` is pinned in `frontend/package.json`. If a
  `package-lock.json` appears, something went wrong — delete it.
- **All cargo commands run from the repo root**, never from `backend/`. The cwd-relative
  `DATABASE_URL` default resolves to `data/` at the root; running from `backend/` silently
  creates `backend/data/`.
- **sqlx-cli is installed but not on PATH.** `export PATH="$HOME/.cargo/bin:$PATH"` first.
  The commands are `cargo sqlx migrate run --source backend/migrations` and
  `cargo sqlx prepare --workspace`, both from the root, cache at the root `.sqlx/`.
- **No `rust-toolchain.toml`, deliberately.** There is no rustup on this machine, so a pin
  would be inert; builds run on a standalone nightly. That nightly occasionally throws a
  self-recovering incremental-compilation ICE (`unstable fingerprints for
  evaluate_obligation`) and drops `rustc-ice-*.txt` files. They are gitignored, harmless,
  and clear with `cargo clean`. Every build still completes.
- **`cargo run` against a scratch database needs `SQLX_OFFLINE=true` too.** Pointing
  `DATABASE_URL` at a throwaway file for a manual walkthrough sends the sqlx macros ONLINE
  (they check queries against whatever database `DATABASE_URL` names), and it fails with
  roughly twenty errors. This looks like a compile catastrophe — it resembles the nightly
  ICE above — and is not one; it is only the query macros wanting the offline cache.
  `export SQLX_OFFLINE=true` before `cargo run` fixes it.

## The verification gate

```bash
cargo test
cargo clippy --all-targets -- -D warnings        # --all-targets matters, see below
SQLX_OFFLINE=true cargo build
python3 frontend/scripts/check-contrast.py
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
```

**Since Part 8, `cargo test` and `cargo build` run `pnpm build` first**, via
`backend/build.rs`. Two consequences worth knowing before they surprise you: a TypeScript
error now fails the *Rust* build, and a cold `cargo test` needs node and pnpm on `PATH`. The
build script only reruns when a `vite build` input under `frontend/` changed, so backend-only
work does not pay for it. `QUIZAPP_SKIP_FRONTEND_BUILD=1` opts out when you know `dist` is
current; `QUIZAPP_PNPM` points at pnpm if it is not on `PATH`.

**`pnpm exec oxlint` and the contrast script joined the gate in Part 4.** The lint run is
what makes CLAUDE.md rule 3 (never use `any`) mechanically enforced rather than prose —
adding the rule without running it in the gate would have changed nothing. The contrast
script proves the `brand` button clears WCAG AA in both themes; it caught a 2.14:1
white-on-orchid failure in light mode that had survived three parts unnoticed, because
until Part 4 there was no way to switch themes without visiting System Settings.

**`frontend/scripts/check-contrast.py` mirrors the token values in
`frontend/src/styles/globals.css`.** When a token changes there, change it here too — this
file is the only place the ratios are actually proven, and a screenshot cannot tell 4.4
from 4.6. It now covers every fixed foreground/background pair in the app, in both themes,
split into two tiers: ENFORCED rows fail the gate below 4.5:1, and a RECORDED/KNOWN tier for
a deliberately deferred failure, so it stays visible instead of silently passing. As of the
Makka Pakka repalette (`df3b138`) there are 16 ENFORCED pairs and the RECORDED tier is
**empty** — the one entry it ever held, light `--primary` + `--primary-foreground` at
3.24:1, was resolved by the repalette (light `--primary` is now 5.90:1) and moved into
ENFORCED. See the Makka Pakka repalette bullet under Part 4 in Where things stand.

**`tsc --noEmit` alone checks nothing — use `tsc -b --noEmit`.** `frontend/tsconfig.json` is a
solution file with `"files": []` and two project references, so a bare `tsc --noEmit` reads it,
finds zero files, and exits 0 whatever the code says. Verified: a deliberate
`const x: number = 'string'` passes `tsc --noEmit` and fails `tsc -b`. This was in the gate
from Part 1 to Part 3 and never caught anything. Nothing was actually unprotected, because
`pnpm build` runs `tsc -b`, but the first half of the gate was theatre. Fixed 2026-08-28.

**Regenerate the sqlx cache against a scratch database, not `data/quizapp.db`.**
`cargo sqlx prepare --workspace` needs `DATABASE_URL` pointing at a migrated database. Build
one in a temp directory by running the migrations in order with `sqlite3`, and point
`DATABASE_URL` at that. It keeps the dev database out of the loop entirely.

**Use `--all-targets`.** Plain `cargo clippy -- -D warnings` does not build test targets. It
was the gate for all of Part 1, which meant roughly 370 lines of test code had zero lint
coverage until the final review noticed. Do not quietly drop the flag.

## Conventions and traps, learned the hard way

These each cost a fix round. They are cheap to honour and expensive to rediscover.

**Ordering tests must be able to detect the collation.** A `COLLATE NOCASE` ordering test
needs inputs where BINARY and NOCASE genuinely disagree. BINARY compares bytes, so every
uppercase letter sorts before every lowercase one (`'Z'` = 0x5A < `'z'` = 0x7A).
`"apple"`/`"Banana"` and `"zebra"`/`"Zulu"` discriminate; `"Alpha"`/`"beta"` and
`"Deck A"`/`"Deck B"` do **not** — both collations order those identically, so the test
passes even with the collation deleted. Where an `ORDER BY` has two collated keys, prove
each independently by removing only that key's collation and watching the test go red.
Removing both at once proves nothing about either.

**Mutation evidence only proves what you mutate in isolation.** Related to the above and
the more general lesson: "I removed X and the test went red" is only evidence for X if X was
the sole change.

**Timestamps are ISO-8601 with `Z`** (`strftime('%Y-%m-%dT%H:%M:%SZ','now')`), stored as
`TEXT`, mapped to `String` in Rust. No `chrono`. The `Z` matters: JavaScript parses a
space-separated SQLite datetime as *local* time, which would silently shift every timestamp
on the future stats screen. ISO strings still sort lexicographically as chronological.

**Timestamps have one-second resolution, so ties are normal.** Any date ordering needs an
`id` tiebreak, and that tiebreak must **mirror the sort direction** — an unconditional
`d.id DESC` breaks descending/ascending symmetry on ties. See the comment above the list
query in `backend/src/routes/decks.rs`, which also documents which half of that tiebreak is
provable and why the other half is kept anyway.

**One parameterized sqlx query beats N literal ones.** `query_as!` needs a literal string,
which tempts a query per filter combination. Let SQL branch on bound parameters instead
(`? = 'all' OR (? = 'none' AND …)`, `CASE WHEN ? = 'oldest' THEN …`). Use plain repeated
`?` placeholders, never `?1`/`?2` — the macro counts by occurrence and numbered
placeholders break the binding.

The same counting rule bites when you *mutate* such a query to prove a clause is
load-bearing: preserve the placeholder count, or you get a confusing compile error that on
this machine's nightly arrives dressed up as the self-recovering ICE.

**Foreign keys are per-connection.** Enforcement comes from `.foreign_keys(true)` in
`backend/src/database.rs`, not from anything in the schema. Any other client — DBeaver, the
`sqlite3` CLI — has them OFF unless it asks.

**PATCH distinguishes absent from null.** A key missing from the body means "leave
unchanged"; an explicit `null` means "unparent". This is a hand-rolled `Option<Option<T>>`
deserializer in `backend/src/routes/decks.rs`. The client must therefore send an explicit
`null`, not omit the key — `JSON.stringify` drops `undefined` but keeps `null`. This is a
decks convention, not a codebase-wide one: `PATCH /api/cards/:id` is a deliberate full
replace where an absent optional MEANS null — see the doc comment on `cards::patch`.

**Every failure returns the envelope** `{"error", "message", "fields"}` as
`application/json`, `fields` being `[]` for non-validation errors. Malformed bodies go
through the `AppJson` extractor in `backend/src/extract.rs` so they get the envelope too
rather than axum's raw `text/plain`. The frontend renders `fields` inline beside the
offending input; **a rejected save must never clear typed content.**

**Editing an applied migration changes its checksum**, after which sqlx refuses to run
against an existing database — a comment-only edit is enough to trigger it. Delete
`data/quizapp.db` and let it regenerate. That is fine while the data is throwaway; it stops
being fine once real cards exist.

**Deletion is real, and cascades.** Deleting a card removes its `choices`, `accepted`,
`schedule` and `reviews` rows; deleting a deck removes its cards and everything under
them. `reviews.card_id` gained `ON DELETE CASCADE` in
`0004_delete_cascades.sql` — before that it deliberately had none, so that a stray delete
would fail loudly. Accuracy figures therefore shift retroactively when a card goes.
Deleting a module removes only the module: `decks.module_id` is `ON DELETE SET NULL`, so
its decks survive as unparented.

**`sessions` rows outlive their decks.** `sessions.deck_ids` is JSON text with no foreign
key, so a deleted deck leaves sessions pointing at a gone id. Their reviews cascade away,
so such a session reads as zero-answered. Left alone deliberately: nothing in the
interface lists sessions, and a session with no reviews is already a legitimate state.

**`0001_init.sql`'s header comment about `PRAGMA foreign_keys` is wrong, and is left
wrong on purpose.** It claims the migration runner's connection does not set the pragma,
and points at `backend/src/db.rs`. The file is `backend/src/database.rs`, and its
`connect()` sets `.foreign_keys(true)` on the pool options and then runs
`sqlx::migrate!` on that same pool — so foreign keys are enforced while migrations run.
Correcting the comment would change an applied migration's checksum, after which sqlx
refuses to run against an existing database, so it stays as it is. Do not build a
migration's safety argument on it: `0004_delete_cascades.sql` states the two reasons its
table rebuild is safe *under* enforcement.

**Session state lives only in `reviews`.** The weights, the staleness, the no-repeat window
and the progress counts are all derived from it, which is why a mid-session reload resumes
correctly with no client state. Adding a second store — a "served card" table, a client-side
queue — breaks that property. `/next` re-rolling on reload is a consequence, not a defect:
an unanswered serve wrote no row, so there is nothing to resume to.

**A test that passes for the wrong reason is worse than no test.** Part 3's mutation pass
found six that could not fail: a dominance test that only built one side of its comparison,
a `roll.clamp` that no input could reach, an error assertion that checked only the field name
while two different messages used that field, a no-repeat test that the weighting alone
satisfied, a `can_override` test missing the case that mattered, and an accuracy guard whose
removal produced `Some(NaN)` — which serde serialises as `null`, making it byte-identical
over HTTP. Run the mutation, one change at a time; roughly one test per task was hollow.

**When a hollow test is found, re-read the acceptance criterion it was meant to satisfy — not
just the assertion that was hollow.** Part 7 shipped five hollow tests, and the fifth was
caught only by the final whole-branch review. Its Task 4 criterion read "`due_at` one day on";
the test asserted `due_at.ends_with("T00:00:00Z")`, which cannot fail because the SQL appends
that literal unconditionally. That was spotted mid-plan and fixed with a `len() == 20`
assertion — which does kill a `date()`-to-`datetime()` mutation, so it looked like a fix. But
it addressed the *symptom*, and the criterion stayed untested: `date()` returns ten characters
for any offset, and `interval_days` is written from a separate binding, so forcing the offset
to `+0 days` — a scheduler that never delays anything, SM-2 silently degraded to practice with
extra bookkeeping — passed all 364 tests and looked correct on every screen. The test now
reads the review's own `answered_at` back and compares against a computed
`date(answered_at, '+1 days')`. Fixing the assertion is not the same as testing the criterion.

**shadcn's `destructive` badge variant fails AA on `bg-card`.** It is a 10%-alpha tint of
`--destructive` under `--destructive` text, which measures **3.64:1 in light and 3.60:1 in
dark** against the card surface. Part 6's weakness badge therefore uses the solid pair,
`bg-destructive` with `--destructive-foreground`, at 5.04:1 and 5.52:1 — already an ENFORCED
row in `check-contrast.py` as "verdict destructive", so the badge added no new pairs at all.
The tinted variant is vendored shadcn and stays as it is; do not "tidy" the badge back onto
`variant="destructive"`.

**Reverting a mutation with `mv` can leave cargo running the mutated binary.** Restoring a
backup file preserves its *old* mtime, so cargo sees nothing newer than its artifacts and
skips the rebuild — the next test run then reports the mutation's result against source that
no longer contains it. This cost a confused minute in Part 6, where a reverted `accuracy_of`
still returned `Some(NaN)`. `touch` the file after any restore, or copy rather than move.

**One rendering path.** `<Markdown>` in `frontend/src/components/Markdown.tsx` is the only
markdown renderer in the app, and it is why Part 2a shipped raw text everywhere. The card
list, the editor preview and Part 3's session runner all go through it. If you need different
behaviour, add a prop — a second renderer is the exact outcome the 2a/2b split existed to
prevent.

**KaTeX's fonts come from the npm package**, imported as `katex/dist/katex.min.css`. Do not
switch to a CDN. Part 8 vendored Quicksand and Inter the same way, so the app now has **no
network font dependency at all** — that is a property to preserve, not a coincidence. A phone
studying on a LAN with no internet is the case it exists for.

**Tailwind's preflight strips list markers and heading sizes.** The `.markdown` block at the
end of `globals.css` restores them. Delete it and every bullet in every card silently becomes
an unindented line.

**Uploaded filenames are content-addressed** — the first 8 bytes of the SHA-256 as hex, plus
an extension from the *sniffed* type, never from the uploaded filename. Re-uploading the same
image therefore reuses one file. The extension list in `images::ImageType::extension` and the
one in `cards::is_uploaded_image_path` must stay in step; the second rejects any path the
first could not have produced.

**Upload failures use the same envelope as everything else**, with `fields[0].field == "file"`,
which is why the upload route raises axum's `DefaultBodyLimit` above the 5 MiB check it does
itself — axum's own 413 is raw `text/plain` and would be the one failure in the app the
frontend cannot parse.

## Outstanding

### Deletion — verification status

Verified by running the full gate from the repo root on the `deletion` branch on
2026-08-31.

- `cargo test` — **391 passed, 0 failed** (381 recorded after Part 8; +10 new, across the
  `delete_card` / `delete_deck` / `delete_module` / `deletion-impact` handlers and their
  cascade tests)
- `cargo clippy --all-targets -- -D warnings` — clean, including the three new delete
  handlers
- `SQLX_OFFLINE=true cargo build` — clean
- `python3 frontend/scripts/check-contrast.py` — **16 rows, all `ok`, RECORDED tier
  empty**, the same 16 as before Part 8 and before this plan; deletion introduced no new
  colour pair, including the confirmation dialogs
- `pnpm exec tsc -b --noEmit` — clean
- `pnpm build` — succeeded, **no chunk-size warning this run**, largest chunk `markdown`
  at 389.69 kB
- `pnpm exec oxlint` — exit 0, **12 warnings, the same 12 pre-existing ones** (two
  `only-export-components` in the shadcn-vendored `ui/badge.tsx` and `ui/button.tsx`,
  eight `set-state-in-effect`, one `purity`), none `no-explicit-any`, none new

Every command passed. Nothing in this run needed a fix.

### Part 8 — verification status

**The automated gate is green, the running binary was exercised end to end, and the four
phone-layout changes have not been looked at by anybody.** Those are three different levels
of evidence and they are kept separate below on purpose.

Verified by running it on `feat/part8-embed-lan` on 2026-08-30:

- `cargo test` — **381 passed, 0 failed** (364 before Part 8; +17: 8 in the new
  `backend/tests/frontend.rs`, 2 in `tests/health.rs`, 4 unit tests in `assets.rs`, 3 in
  `configuration.rs`)
- `cargo clippy --all-targets -- -D warnings` — clean
- `SQLX_OFFLINE=true cargo build` — clean
- `python3 frontend/scripts/check-contrast.py` — **16 rows, all `ok`, RECORDED tier empty**,
  the same 16 as before; Part 8 introduced no colour pair
- `pnpm exec tsc -b --noEmit` — clean
- `pnpm build` — **no chunk-size warning**, largest chunk `markdown` at 389.69 kB
- `pnpm exec oxlint` — exit 0, **12 warnings, the same 12 pre-existing ones**, none
  `no-explicit-any`, none new

Verified against the actual release binary, with no Vite running:

- `/`, `/decks`, `/decks/3`, `/session/1` all return **200 `text/html`** — client-side routes
  survive a hard refresh
- `/api/nope` returns **404 `application/json`** with the envelope; `/api/decks` returns 200
  JSON. Creating a deck and reading `/api/decks/1/stats` both worked through the binary
- a hashed JS asset: `text/javascript; charset=utf-8`, `immutable`, **gzip 111 KB vs 281 KB
  raw**; a woff2: `font/woff2`, `immutable`, **no `content-encoding`**
- bound to `0.0.0.0`, the startup log printed `http://192.168.101.116:3112`, which matched
  `ipconfig getifaddr en0` and served the app on that address
- **the clean-state build ordering**, which is the premise the whole build.rs approach rests
  on: `rm -rf frontend/dist && cargo clean -p quizapp && cargo build` succeeds, because cargo
  runs a crate's build script before compiling that crate
- **build-script rerun granularity**, measured by `dist/index.html` mtime rather than assumed:
  a backend-only edit does not rebuild the frontend; an edit to `frontend/src/main.tsx` does
- **both escape-hatch paths**: `QUIZAPP_SKIP_FRONTEND_BUILD=1` with a built `dist` reuses it;
  with `dist` missing it fails with the readable message naming the way out

**Not verified, because it needs a phone and a pair of eyes.** Four layout changes were made
by reading class names against a 375px viewport. **Every one is reasoning, not observation** —
this is the same class of claim the earlier parts recorded honestly, and it is recorded the
same way rather than folded into "Part 8 complete":

1. **`DeckPage.tsx`: `pl-11` → `sm:pl-11`, and the card list's `-ml-7` → `sm:-ml-7`.** These
   align the drag grips outside the content column on desktop; at 375px they were spending
   44 px of a 343 px column on nothing. The claim is that phone width now uses the full
   column *and* desktop alignment is unchanged. Both halves need looking at.
2. **`DeckStatsStrip.tsx`: the `·` separator moved from before its part to after the
   preceding one.** With five parts it will wrap at 375px, and it previously rendered the dot
   *inside* the following span, so a wrapped line could begin with a leading `·`. The claim is
   that lines now start with real text and the desktop rendering is unchanged.
3. **`SessionPage.tsx`: the four self-grade buttons became `grid-cols-2` at phone width**,
   `sm:flex sm:flex-wrap` above it. Four `min-w-24` buttons want ~408 px in a ~327 px column,
   so they were wrapping 3+1 and leaving an orphan. The claim is a clean 2×2.
4. **`AppShell.tsx`: the theme toggle got `ml-auto`.** It sat immediately after the nav links
   rather than at the trailing edge. Cosmetic, and the least risky of the four.

Everything else on the responsive side was **checked and deliberately not changed**:
`sm:grid-cols-2` on the choice lists and results rows, `SummaryTiles` at
`grid-cols-2 sm:grid-cols-4`, `DecksPage`'s `flex-col sm:flex-row` toolbar, and the
`max-w-2xl` runner shells all read as correct at 375px. Do not churn them.

**Also unverified:** how the app behaves on a phone browser at all — touch targets, the drag
grips under touch rather than mouse, and whether the KaTeX in a prompt is legible at that
width. None of that has ever been observed in this project.

**Facts recorded here so they are not later "fixed":**

- **The embed reads `$OUT_DIR/frontend`, not `../frontend/dist`.** Pointing it at `dist`
  directly reintroduces a confusing compile failure whenever `dist` is deleted without a
  watched input changing. See the Part 8 bullet under Where things stand.
- **`.fallback()` sits above `.layer()` in `lib.rs`** and must stay there.
- **`api_router()` has its own fallback**, and removing it makes `/api/*` typos return HTML
  with a 200 rather than the envelope.
- **`manualChunks` will not work here.** Vite 8 on Rolldown; use
  `build.rolldownOptions.output.codeSplitting`, and keep `react` at a higher priority than
  `markdown`.
- **The default bind is loopback on purpose**, since the app has no authentication.
- **The `rust-objcopy` / `libLLVM.dylib` warning on `cargo build --release` is a
  pre-existing toolchain quirk of this machine's nix nightly**, not a Part 8 regression. The
  build completes; debug-info stripping is what fails.

### Part 7 — verification status

**The automated gate is green and the browser walkthrough was never performed.** Same shape
as Part 5's entry below, and the same caveat applies: a green gate is not the walkthrough.

Verified, by running it on `feat/part7-sm2` at `85368ab` on 2026-08-28:

- `cargo test` — **364 passed, 0 failed**, across twelve test binaries:

  | Binary | Passed |
  | --- | --- |
  | `unittests src/lib.rs` | 106 |
  | `unittests src/main.rs` | 0 |
  | `tests/cards.rs` | 49 |
  | `tests/decks.rs` | 23 |
  | `tests/health.rs` | 2 |
  | `tests/images.rs` | 11 |
  | `tests/mock.rs` | 57 |
  | `tests/modules.rs` | 11 |
  | `tests/sessions.rs` | 70 |
  | `tests/sm2.rs` (new in Part 7) | 20 |
  | `tests/stats.rs` | 15 |
  | Doc-tests | 0 |

  This is the count **actually observed for this update**, not carried forward from
  `docs/PART-7-HANDOVER.md`'s mid-execution figure of 355 (recorded after Task 4 of 8, before
  Tasks 5-8 added more), and not carried forward from this document's own pre-Part-7 figure of
  317. This document's test count has been stale twice before; this one is the number this
  session watched `cargo test` print. The final review wave that closed Part 7 removed one
  redundant unit test in `scheduler.rs` (Finding 3, a strict subset of
  `quality_follows_the_specified_table`) and added one integration test in `sm2.rs` (Finding 1,
  a multiple-choice case for the `due_at` offset assertion), leaving the total at 364 by
  coincidence rather than by an unchanged suite.
- `cargo clippy --all-targets -- -D warnings` — clean
- `SQLX_OFFLINE=true cargo build` — clean
- `python3 frontend/scripts/check-contrast.py` — **16 rows, all `ok`, no RECORDED section**,
  the same 16 as before Part 7 — the evidence that no new colour pair crept in
- `pnpm exec tsc -b --noEmit` — clean, no output
- `pnpm build` — builds; the JS bundle is 906.95 kB (276.80 kB gzipped), still tripping Vite's
  500 kB chunk warning, unchanged in shape from Part 5's note about it
- `pnpm exec oxlint` — exit 0, 12 warnings, the same 12 pre-existing warnings as recorded under
  Part 5's minor findings, none of them `no-explicit-any` and none new

**Not verified, because no browser was available.** The Chrome extension is not connected on
this machine — checked directly for this update, not assumed. The task-8 brief's Step 5 lists
nine points; **none were driven.** This is a complete gap, itemised rather than folded into
"Part 7 complete":

1. A fresh deck: the tile is enabled and reads the due count.
2. Starting an SM-2 session: the header reads `0 of N due`.
3. A correct multiple-choice answer: `schedule` shows `repetitions 1`, `interval_days 1`,
   `due_at` tomorrow at midnight — checked in DBeaver, where foreign keys are per-connection
   and off.
4. Revealing a flashcard and grading `again`: `repetitions 0`, `lapses 1`, and **`ease`
   unchanged** — the one point that most needs a human eye, since the ease-unchanged behaviour
   is exactly the thing a future reader is likeliest to "fix" on sight.
5. A mid-session reload: the same card, counts intact.
6. Emptying the due pool: the summary screen; starting another SM-2 session is refused, naming
   the next due date; the tile is disabled with the same date.
7. A short-answer miss, then an override: the schedule is recomputed, not left at the lapse.
8. The strip shows three figures; a mode with no reviews reads `—`, not `0%`.
9. Both palettes; hand-editing `/mock/:id` to an sm2 session id and confirming the redirect.

None of the unit and integration tests substitute for this list — they prove the numbers the
server computes, not that a human looking at the screen sees the right thing, in the right
place, in both palettes, at whatever width the browser happens to be. The gate cannot see a
layout and it cannot see an information leak.

**375px phone width remains completely unrendered** — Part 8 made four layout changes
*for* it by inspection, but nobody has looked at the result; see "Part 8 — verification
status" above. It was unrendered across Parts 1, 2b, 2c, 3, 4, 5, 6 and 7. Part 7 is the sharpest instance of this risk so far, not just another entry in the
list: it adds a *third* figure to a stats strip that was already unverified at phone width
after Part 6, on a wrapping flex row that has never been seen collapse. `resize_window`
reports success in this environment but the viewport does not actually change. This belongs
to build step 8's phone layout pass.

**Facts recorded here so they are not later "fixed":**

- **`repetitions` in Rust and TypeScript, `reps` in SQL** — see the Part 7 bullet under Where
  things stand for the full reasoning. The split is deliberate; Part 7 ships no migration.
- **A lapse leaves the ease factor unchanged** — original SM-2, pinned by a unit test in
  `backend/src/scheduler.rs`, and a majority of implementations elsewhere do the opposite.
- **`schedule_for` was not replaced**; `schedule_state_for` was added alongside it, and Part
  5's mock canary now proves through it that mock mode leaves `interval_days`, `ease`,
  `repetitions` and `lapses` untouched, not only that a schedule row exists.
- **The `answer` handler's inline `can_override` predicate was kept**, not swapped for
  `can_override_result`, which would have broken the pinned flashcard-regrade test at
  `backend/tests/sessions.rs:1023`.
- **All modes now insert their `reviews` row inside a transaction**, not only sm2 — an
  incidental widening from Task 4, accepted at review as the minimal way to make the sm2
  answer write atomic.
- **`NextResponse` is `#[serde(untagged)]` with no `Deserialize`, so there is no ambiguity
  today** — but if `Deserialize` is ever added, the three variants still discriminate only by
  required-field presence, which becomes a real hazard at that point.
- **The Spaced-repetition tile is disabled while `due_count` is still loading**, not only when
  it is `0` — a ruling made during Task 7 execution against the plan's own code, because the
  task's acceptance criterion said so and `deck`/`deckStats` load in parallel.

### Part 5 — verification status

**The automated gate is green and the browser walkthrough was never performed.** Both halves
of that sentence matter, and the second is not a formality.

Verified, by running it on 2026-08-28:

- `cargo test` — **317 passed, 0 failed**, including a new `backend/tests/mock.rs` of roughly
  1,460 lines
- `cargo clippy --all-targets -- -D warnings` — clean
- `SQLX_OFFLINE=true cargo build` — clean
- `python3 frontend/scripts/check-contrast.py` — **16 ENFORCED rows, RECORDED tier empty**,
  the same count as before Part 5, which is the plan's own stated evidence that no new colour
  pair crept in
- `pnpm exec tsc -b --noEmit` — clean
- `pnpm build` — builds; the JS bundle is now 904 kB (276 kB gzipped), still tripping Vite's
  500 kB chunk warning
- `pnpm exec oxlint` — exit 0, 12 warnings, none of them `no-explicit-any`

**Not verified, because no browser was available.** The Chrome extension is not connected on
this machine, and the plan's Task 17 walkthrough has twenty-one points, none of which were
driven. **Part 5 was merged anyway, as a deliberate decision**, so this list is not a
pre-merge blocker but a standing list of what remains unobserved in shipped code. What that
costs is specific rather than general:

- **Four points (3, 7, 9 and 21) are the answer-leak checks**, and they are the reason this
  gap is not merely tidiness. The tests assert the leak boundary at the Rust level, but the
  design closed leaks on four endpoints (`/answer`, `/next`, `/reveal`, `/override`) plus
  `/results`, and **point 9 is the only check that the running client never *asks*** — that
  `GET /results` does not appear mid-run at all, not as a 200 and not as a 409. A test can
  prove the endpoint refuses; only the Network tab proves the client never asks. Point 21
  is its manual counterpart: probing `/override` and `/results` by hand mid-run.
- **The double-submit guard is unexercised.** Point 5 holds Enter down with and without a
  selection and counts the `POST /answer` requests. The guard is a `useRef`, which is the
  correct shape, but "correct shape" and "one request" are different claims.
- **Reload-stability is unobserved end to end.** The backend tests cover the same card being
  served; point 8 also wants the same *choice order* and a clock that continues rather than
  restarting.
- **The mode-mismatch redirects** (point 16, hand-editing `/session/:id` to `/mock/:id` and
  back) exist only as code.
- **Both palettes on the results screen** (point 17) — the chips, the border stripes, the
  progress bar against its track, KaTeX inside a results row. The contrast script proves the
  ratios; it says nothing about whether the stripes read at a glance, which is point 12's
  squint test.
- **The override on the results screen** (points 13 and 14), including whether the count and
  accuracy update in place without a re-order.
- **The one-card and zero-card decks** (point 20), **End test early** (point 19), and
  **reduced motion** (point 18).

**375px phone width** is still unrendered — Part 8 addressed it in code, unobserved. Part
6's stats strip is the newest thing never seen at that width; it is a wrapping flex row and
is *expected* to wrap rather than overflow, but that is a prediction, not an observation.
`resize_window` reports success in this environment but the viewport does not change.
Part 8's phone pass covered the strip's wrapping specifically, still without observation.

**Part 5 minor findings**, from reading the committed code rather than from a review round:

- `MockSessionPage.tsx:40` initialises a `useRef` from `Date.now()` during render, which
  oxlint flags as `react(purity)`. Harmless in practice — a ref initialiser's value is
  discarded on re-render — but it is a warning that will be re-found by every future lint
  reader.
- `MockSessionPage.tsx:90` and five pre-existing sites carry `react(set-state-in-effect)`
  warnings. Twelve in total across the frontend; the count did not meaningfully change with
  Part 5, and none is `no-explicit-any`.
- The plan's own Task 17 checklist includes updating this document and the master spec. Those
  were done (this section, the Part 5 section under Where things stand, and the master spec's
  API list and Study engine paragraph). The walkthrough was not, and is recorded as not done
  rather than quietly folded into "Part 5 complete".

### Part 4 — verification status

**Both palettes have been driven, and light mode was adjusted as a result.** Hayley checked
Part 4 in a browser on 2026-08-28, then checked it again after the Makka Pakka repalette
(`df3b138`) and made `e09e76d` in response. Makka Pakka light and Bibble dark have both
been seen in the running app.

What that does *not* give you is an itemised record. Both passes were human, not agent —
no Chrome browser was connected to the sessions that ran Part 4 — and the confirmations
were general rather than point-by-point. So the individual items (the theme toggle's three
states and its no-flash-on-load behaviour, the sparkle burst, the streak badge and its
flutter, reduced motion suppressing both, the deck-card flip under reduced motion, KaTeX in
both palettes, and the markdown-link-does-not-flip fix) are **attested rather than
itemised** — nobody wrote down a per-item observation, and this document does not invent
one after the fact.

**Surface separation in light mode is the thing to look at first if you change the
palette.** It is a three-layer warm stack — page `#f5ebe0`, card `#e3d5ca`, recessed
`#d5bdaf` — and each step is only about a 1.2:1 luminance difference: card against page
**1.22:1**, an unselected choice against the card it sits on **1.25:1**, the deck-card
header band against the deck body **1.25:1**. Enough to perceive, but borders and shadows
carry a large share of the work of showing where one surface ends and the next begins. An
earlier revision was flatter still, with the card at **1.00:1** against the page —
identical in lightness, differing only in hue — which is what the current stack was chosen
to fix.

**A collision worth knowing about, because it will recur.** `--secondary` is the unselected
multiple-choice background and it previously shared `#e3d5ca` with what is now `--card`. When
the card moved to `#e3d5ca`, every choice button would have rendered at 1.00:1 against the
card it sits on — invisible. `--secondary` and `--muted` were moved down to `#d5bdaf` to
restore the step. Any future change to one layer of this stack has to be checked against the
layers either side of it; the contrast script covers text legibility, not surface separation.

**What remains genuinely unverified**, because no one has looked:

- **375px phone width**, now across Parts 1, 2b, 2c, 3 and 4. `resize_window` reports
  success in this environment but the viewport does not change. It belongs to build step
  8's phone layout pass and needs a human at a browser.
- **Part 2c's nine-point walkthrough**, still outstanding — it was not performed here
  either. See the list further down. Note that `e09e76d` reworked the deck page around the
  card list, so points 6–8 (drag reorder, keyboard reorder) are now unverified against
  changed layout as well as never having been driven.

**Part 4 deferred minor findings**, recorded during execution. One entry that used to be
here — `--primary` + `--primary-foreground` failing AA at 3.24:1 in light mode, deferred
because fixing it meant a palette decision — is **removed**: the Makka Pakka repalette
(`df3b138`) made that decision. Light `--primary` is now `oklch(0.47 0.075 68)` at 5.90:1,
the pair passes, and `check-contrast.py`'s RECORDED/KNOWN tier is empty. See the Makka
Pakka repalette bullet under Part 4 in Where things stand for the full change, and the
verification note above this one for what is unverified about the new palette itself.

Findings still open:

- `button.tsx`'s pre-existing `brand` comment still says the variant takes "the orchid band
  colour from the deck card", which is no longer true now that it uses `--brand`.
- `index.html`'s `catch (error)` binds an unused variable (outside the TS project, so no
  linter sees it).
- `SessionSummary.tsx`'s accuracy ternary was reflowed from three lines to one during
  extraction; rendered output identical.
- Mixed radius on the runner: newer surfaces are `rounded-xl` while `AnswerVerdict.tsx` and
  `ChoiceList.tsx` remain `rounded-lg`. Reviewed and judged defensible (smaller nested
  elements taking a smaller radius is conventional), deliberately deferred.
- `CardEditorPage.tsx`'s new surface `div` sits at the same indentation as the ternary
  containing it, with roughly 130 lines of children not re-indented. JSX unaffected; the
  file's indentation is now misleading.
- `promptLabel` in `CardRow.tsx` strips hyphens, so "k-means" becomes "k means". Judged
  non-blocking because screen readers do not vocalise a mid-word hyphen. Markdown link URLs
  can also leak into the label.

**Part 3's walkthrough was driven and passed.** Recorded here because it is the first part of
this project to get one. Verified in a browser on 2026-08-28, against the UI as it stood
then — `/study` has since been deleted, so the first item below no longer describes a
screen that exists, though the session it started is unchanged: the `/study` picker and its
live card count; the multiple-choice loop by keyboard (`2`, Enter); the flashcard loop (Space to
reveal via `/reveal`, `3` to grade); short-answer normalisation, where `  K-MEANS!  ` graded
correct against accepted `k-means`; the override, where a wrong answer flipped to "Counted as
correct" and the *same wording in different case and punctuation* then graded correct on the
next serve; a mid-session reload preserving the answered/correct counts; the finish summary
reporting "1 counted correct by override"; KaTeX and markdown rendering in prompts; and forty
consecutive serves across all three kinds carrying no `is_correct`, `answer_md`,
`explanation_md` or `accepted` in the card object, with both choice orderings observed.

**Still outstanding from Part 3:**

- **Phone width.** `resize_window` reports success but the viewport does not change in this
  environment, so 375px was never actually rendered. The runner uses the same `max-w-2xl`
  shell as every other screen and the multiple-choice grid is `sm:grid-cols-2`, so it should
  collapse to one column — but that is reasoning, not observation.
- **Both themes.** Only the dark palette was seen.
- Whether a 100+ card deck stays responsive in the runner, which is what COS781 will be.


**Needs a human at a browser** — no agent could verify these; the Chrome extension is not
connected on this machine, so no agent could drive a browser for either Part 1 or Part 2a:

- The card editor at 375px: the choices rows, the radio column, the action bar
- Both themes, and the kind badges legible in each
- Whether the keyboard loop genuinely *feels* like a loop — prompt, choices, save, next —
  without a pause to find where focus went
- Focus landing correctly after appending a row and after save-and-next
- The `Cmd/Ctrl+Enter` fix in practice: pressing it from the last choice row must save
  without appending a phantom row
- Both `DeckPage` (`if (!deck) return null`) and `CardEditorPage` (`if (!loaded) return
  null`) render a blank body during their initial load rather than a skeleton — whether the
  flash-of-nothing is noticeable on either. One browser pass covers both.
- Whether the deck detail page reads well at 100+ cards, which is what COS781 will actually
  be. If not, the kind filter and prompt search deferred out of Task 4 are the fix.
- Still outstanding from Part 1: the OS theme toggle actually swapping the Bibble light/dark
  palettes, and `/decks` at 375px

Added by Part 2b (the extension was still not connected, so none of these were driven):

- Whether 100+ unclamped rows, each rendering KaTeX, stay responsive and scannable — this is
  what COS781 will actually be. If not, the kind filter and prompt search deferred out of
  Part 2a Task 4 are the fix.
- The Edit/Preview toggle inside the keyboard loop, and where focus lands coming back
- That `⌘/Ctrl+P` toggles the preview and does not open the browser's print dialog
- The image thumbnail and its lightbox at 375px, and Escape closing the lightbox
- KaTeX legibility against both Bibble palettes
- That a rejected upload leaves every other typed field untouched, and that the same file can
  be picked again straight after a failure (the input-value reset)

Added by Part 2c (the browser tooling still could not reach the dev server, so **none of these
nine points were driven** — this is not a partial pass, it is a complete gap):

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
8. ~~Toggle "Show archived" on, drag a visible card past an archived one, reload — the
   archived card is still where it was.~~ **Struck**: `e09e76d` removed the toggle, so
   there is no longer a way to put an archived card on screen. Do not re-add this point
   unless the toggle comes back.
9. In macOS System Settings → Accessibility → Display, turn on "Reduce motion", then flip a
   card — the face swaps with no rotation.

**Known defects and follow-ups from Part 2c's review** — each was found by review, judged
non-blocking, and deliberately left. They are real; none is a mystery.

- **Clicking a markdown link inside a card navigates *and* flips the card.** The flip target is
  the card body, and its `onClick` has no target check, so a click on an `<a>` in a prompt or
  answer bubbles into `flip()`. The keyboard path does not have this problem — `onKeyDown` got a
  `e.target !== e.currentTarget` guard so Enter on a link or on the image thumbnail reaches its
  own default action. The mouse path is guarded only by the `stopPropagation` wrapper around the
  image thumbnail, which does not cover markdown. Fix is roughly
  `if ((e.target as HTMLElement).closest('a,button')) return` at the top of `onClick`. Introduced
  by Part 2c, since the row only became a flip target here.
- **`move_card` and `create` are read-then-write deferred transactions.** With no WAL and
  `max_connections(5)`, SQLite returns `SQLITE_BUSY` *immediately* on a lock upgrade —
  `busy_timeout` does not retry that case — so two overlapping requests can 500. Data is safe
  (the transaction rolls back) and the client toasts and refetches, so the worst symptom is a
  spurious "Could not reorder cards" during rapid drags. Hardening is `BEGIN IMMEDIATE` for those
  two transactions.
- **A list fetch issued *during* an in-flight `moveCard` can land with pre-move data and stick.**
  `droppedFetch` only covers fetches that predate the drag, so archiving a card inside the move's
  round trip can leave the old order on screen until the next navigation. Cheapest fix is to
  always refetch after a successful move, dropping the `droppedFetch` optimisation.
- **Two accessible-name problems on the card row.** The flip target's `aria-label` ("Show answer")
  becomes the element's whole accessible name, so a focused card announces nothing about which
  card it is; and the drag grip's label slices raw markdown, so a prompt starting with `$$…$$` or
  `# ` is read out as literal syntax. Both are fixed from the same place — `aria-labelledby`
  pointing at the prompt, plus a visually-hidden action label. Related: `role="button"` gives its
  children presentational semantics, which sits awkwardly with the nested image button.
- **`strict` is off for the frontend.** `frontend/tsconfig.app.json` sets neither `strict` nor
  `extends`, so `strictNullChecks` is not checking anything — including Part 2c's fairly heavy use
  of nullable state (`full`, `inFlight.current`, `pending.current`, `image_path`). Pre-existing
  config, but it is the highest-value frontend follow-up. (Separately, the gate's
  `tsc --noEmit` was checking nothing at all until Part 3 corrected it to `tsc -b --noEmit` —
  see The verification gate. `strict` being off is the remaining half of that problem.)

**Housekeeping**

- `data/quizapp.db` holds verification debris from Part 1, Part 2a **and Part 3's
  walkthrough** (modules like `REVIEW_MOD_1` and `COS781 walkthrough`; decks like
  `kinetics 100%`, `Clustering walkthrough`, `Override walkthrough`, `Leak check deck`; and
  the sessions and reviews they generated). Clear it before writing real cards — it
  regenerates on startup.
- ~~KaTeX and `react-markdown` roughly doubled the JS bundle … trips Vite's 500 kB chunk
  warning on every build.~~ **Closed by Part 8.** Split into `react`, `markdown` and `dnd`
  chunks via Rolldown's `codeSplitting`; the largest is now 389.69 kB and the warning is
  gone because the limit is met, not raised.
- ~~Google Fonts: `globals.css` imports Quicksand and Inter over the network, so typography
  silently falls back offline or on a LAN-only phone.~~ **Closed by Part 8.** Vendored via
  `@fontsource/quicksand` and `@fontsource/inter`; `dist` contains no Google reference.

**Known-and-accepted minors**

- **A flashcard whose answer is a sentence will auto-grade wrong in a mock test, nearly every
  time.** `answer_md` is markdown prose — card validation requires it non-blank and nothing
  more — and the whole reason to author a flashcard rather than a short-answer card is that the
  answer was not reducible to a key. No distance metric fixes this; it is what grading free
  text against prose costs, and no authoring restriction was added because the card model is
  not the problem. Two mitigations, neither a cleverer matcher: the override is the correction
  path (which is *why* it had to be extended to flashcards), and the results screen carries a
  one-line note whenever a run contained a flashcard — without it, 30% on a deck of prose
  flashcards reads as either a broken feature or a bad night's revision, and it is neither.
  **The practical advice: for cards you intend to sit a mock test on, keep flashcard answers
  short and keyword-ish, or author them as short-answer cards**, which have an `accepted` list
  built for exactly this.
- **Fuzzy matching can only ever mark a wrong answer right, and there is no reverse
  override.** `type i error` and `type ii error` are distance 1 within a tolerance of 1, so one
  grades as the other — a realistic case for a Data Mining test. No divisor fixes it: the terms
  differ by exactly one character, so any tolerance at all accepts them, and zero tolerance at
  that length would forgive nothing anywhere. This is pinned by a test rather than engineered
  around, so the choice stays visible. **A false accept is currently unfixable from the UI**,
  because "I was right" has no opposite. The follow-up, if it proves annoying in practice, is a
  "mark wrong" action — deliberately not built, because it is a second write path to a
  `reviews` row and Part 3 left the override as the only one.
- **Overriding a mock flashcard fixes the row but does not teach the card.** A short-answer
  override inserts an `accepted` row, so the same wording grades correct next time. Flashcard
  grading compares against `answer_md`, and card validation forbids `accepted` rows on a
  flashcard, so wiring flashcards into `accepted` would be a card-model change. Out of scope,
  and stated rather than left to be discovered.
- **Reloading the results screen fires a redundant `POST /finish`.** The runner's state machine
  has one terminator: a 409 from `/next` — which covers both "the pool is done" and "this
  session has ended" — POSTs `/finish` and then GETs `/results`. On a reload of a finished
  session that path runs again. `/finish` is idempotent, so the second call changes nothing;
  the alternative is a second code path to the results screen, which is worse than one
  redundant request.
- **The `id` half of `/results`' ordering tiebreak is not provable by test.** Ordering is by
  `answered_at` then `id`, both ascending. With several reviews sharing a one-second
  `answered_at`, SQLite returns them in rowid order whether or not the tiebreak is written, so
  removing it changes nothing observable. It is kept because the resulting order is only
  *incidentally* correct — nothing guarantees rowid order for an unqualified `ORDER BY`, and a
  future index or query-plan change could silently reorder a results screen. The sort
  *direction* is proven, by a deliberately constructed tie. This mirrors the decks list query,
  where the same split is already documented.
- **Multi-deck and module-wide mock tests are intact and unreachable**, exactly as for
  practice. The backend accepts both; no button starts one.
- **Archiving is now unreachable from the interface entirely.** `e09e76d` removed the
  "Show archived" switch, and the deletion work removed the per-card archive button that
  was the last control driving it. `archiveCard` and `unarchiveCard` are gone from
  `frontend/src/lib/api.ts`. What remains, deliberately: the `archived` column, both
  endpoints with their tests, `GET /api/cards`'s `archived=all` parameter, and every
  `archived = 0` filter across sessions, stats and the decks list. Two knock-on facts,
  both left as they are: `CardRow` still renders an `Archived` badge and an `opacity-60`
  class that no list can now trigger, and `GET /api/decks/:id/deletion-impact` exists
  precisely because archived cards are invisible yet still get deleted, so the deck
  confirmation must count them separately from `card_count`.
- `DecksPage`'s empty state keys on there being no groups, so the onboarding copy does not
  show when unparented decks exist but no modules do. Cosmetic.
- `AppError::tag_foreign_key_violation` takes `&str` while `AppError::validation` is generic
  over `Into<String>`.
  Both call sites pass literals; generalising it now would be churn.
- `patch_unknown_card_is_404`'s `count(cards) == 0` assertion cannot fail — each test gets a
  fresh empty database. Not a false claim, just an assertion carrying no weight; the 404
  assertion above it is the real test.
- `CardEditorPage` renders an inline error slot for `explanation_md`, which the validator
  never emits. Dead but harmless; Part 2b may give it a use.
- Duplicate accepted answers that normalise to the same key are still accepted on card
  *authoring* — `validate` does not dedupe and `idx_accepted_card_normalised` is a plain,
  non-unique index. Part 3 met this and found it harmless to grading, because the lookup is
  set membership rather than a fetch. The override endpoint carries a `WHERE NOT EXISTS`
  guard so it cannot add to the pile.

## Where the record lives

- **Spec** — [`mitis/specs/2026-08-26-quiz-study-app-design.md`](mitis/specs/2026-08-26-quiz-study-app-design.md). Kept current; amended when
  the implementation legitimately diverged (e.g. deck-name uniqueness per module).
- **Plans** — `mitis/plans/*.md` plus their `.tasks.json`. These carry the full per-task
  code, acceptance criteria and verification commands. All are marked complete.
- **Part 7's design decisions** — [`mitis/specs/2026-08-28-part7-sm2-design.md`](mitis/specs/2026-08-28-part7-sm2-design.md).
  Records why no migration is needed, the quality mapping's `overridden`-is-not-a-parameter
  reasoning, the pure `scheduler.rs` core and the ease-unchanged-on-lapse rule (§3a), the
  due-ordered serve and why it needs no hash trick, the refuse-when-nothing-due ruling, the
  day-granular `due_at`, the transactional answer write, the override replay, the answer to
  Part 6 §10, and why `SessionPage.tsx` is reused rather than forked for sm2 the way mock was.
  Plan: [`mitis/plans/2026-08-28-part7-sm2.md`](mitis/plans/2026-08-28-part7-sm2.md).
- **Part 5's design decisions** — [`mitis/specs/2026-08-28-part5-mock-test-design.md`](mitis/specs/2026-08-28-part5-mock-test-design.md).
  Records the one-deck/whole-deck ruling, why the stable serve needs no storage, rank-by-hash
  over Fisher–Yates, the typed-flashcard decision and its caveat, the tolerance rule and its
  accepted false-accept cost, the three answer leaks and the override oracle, the separate
  page, and the Zustand reversal. Read this one for *why*.
- **Part 3's design decisions** — [`mitis/specs/2026-08-27-part3-practice-mode-design.md`](mitis/specs/2026-08-27-part3-practice-mode-design.md).
  Records the weighted-sampling-over-`position` ruling, the two API amendments, why the
  flashcard reveal is its own endpoint, why never-seen dominance is derived rather than
  tuned, and the small-deck window rule.
- **Part 2b's three open design questions** were answered in the design session of
  2026-08-27 and are recorded in [`mitis/specs/2026-08-27-part2b-images-markdown-design.md`](mitis/specs/2026-08-27-part2b-images-markdown-design.md):
  standalone upload endpoint, whole-form Edit/Preview toggle, unclamped markdown rows with a
  thumbnail lightbox. `PART-2B-HANDOFF.md`, which posed them, has been deleted.
- **Execution ledgers** — `.mitis/sdd/<plan-name>/progress.md`. **These are untracked and
  local to the machine that ran them**, so they will not exist in a fresh clone. They hold
  the fix-round history and adjudications; everything durable from them has been distilled
  into this document. If you are running plans with the mitis skills, expect to create your
  own.

## If you are an agent picking this up

Work through `mitis:brainstorming` before designing build step 8, then `mitis:writing-plans`,
then `mitis:subagent-driven-development`. The six existing plans (Part 3 onward) are worth
reading as a format reference — particularly how each task carries complete code, an explicit
verify command, and a `json:metadata` fence. Part 5's is the most developed of them; Part 6's
and Part 7's each record a mutation table per task.

**Before touching build step 8, merge `feat/part7-sm2` and drive the three outstanding
walkthroughs (Parts 5, 6, 7) — see Next up.** The gate has been green through three merges in
a row with the walkthrough undriven each time; that pattern is itself worth noticing rather
than repeating a fourth time by default.

**Part 5's task ledger is not trustworthy.** `docs/mitis/plans/2026-08-28-part5-mock-test.md.tasks.json`
marks all seventeen tasks `pending` while the code for all of them is committed, and there is
no `.mitis/sdd/` progress ledger for Part 5 at all. So there is no per-task record of fix
rounds or adjudications for this part — only the code, the design doc, and this document.
Treat the design doc as the authority on intent and the tests as the authority on behaviour.

Two habits that earned their cost here: give reviewers the *both sides* of any seam they are
judging (a per-task review structurally cannot see an API-to-client mismatch), and demand
mutation evidence — a test that cannot fail is not a test.

**Run one implementer at a time.** Part 2a dispatched two concurrently because their file
lists were disjoint. Git's index is not per-file: one agent's `git add`/`commit` swept the
other's staged work into a commit labelled as something else, recoverable only because the
branch was local and unpushed. Read-only reviewers can run alongside an implementer; two
writers cannot.
