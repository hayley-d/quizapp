# Handover

Read this first if you are picking up this project without the conversation that built it.

**Last updated:** 2026-08-28, on `main`, at commit `e09e76d` (the styling pass that
followed the Part 4 merge). **Part 4 (the Bibble theme pass) is done and merged** —
`feat/part4-bibble-theme` landed as `60575c8`, and there is no feature branch outstanding.
Both palettes have now been driven in a browser: Hayley looked at Makka Pakka light and
Bibble dark and made `e09e76d` in response, which is a structural change as much as a
styling one — it deleted the `/study` screen and moved session starting onto the deck
page. Read "The `e09e76d` styling pass" under Where things stand before touching the
frontend; it is the most recent thing that happened and the least like what its commit
message says.

Part 2c's nine-point walkthrough is still outstanding, and 375px phone width has never
been rendered in any part (see Outstanding).

## What this is

A self-hosted quiz app for exam revision, replacing Quizlet. The driving deadline is the
COS781 (Data Mining) test on **11 September 2026**. Full design: [`mitis/specs/2026-08-26-quiz-study-app-design.md`](mitis/specs/2026-08-26-quiz-study-app-design.md) — that
document is the record of what the app is meant to be, and it is kept current.

## Where things stand

Parts 1, 2a and 2b of the spec's build sequencing are **done**, and so is the deck card list
redesign that followed them ("Part 2c" below). All of it is merged to `main`; there is no
feature branch outstanding. Concretely, working today:

**Light and dark are now two separate visual identities, not one palette rendered twice.**
Dark mode is Bibble, unchanged. Light mode was repaletted from a pale-aqua rendering of the
same tokens into "Makka Pakka" — warm stone and sand neutrals with ochre-brown and clay
accents. Design record:
[`.mitis/sdd/2026-08-28-part4-bibble-theme/makka-pakka-palette.md`](../.mitis/sdd/2026-08-28-part4-bibble-theme/makka-pakka-palette.md).
The practical consequence: a new colour now needs a value in *both* identities, not one
shared value. A token that reads correctly in one theme may be meaningless or wrong in the
other — do not assume a value carries over.

- Cargo workspace: root manifest, Rust package in `backend/`, React app in `frontend/`
- All eight tables from the data model, in one migration (`backend/migrations/0001_init.sql`)
- `AppError` envelope on every failure, including malformed request bodies
- `GET|POST /api/modules`; `GET|POST /api/decks`, `GET /api/decks/:id`, `PATCH /api/decks/:id`
- `GET /api/decks` supports server-side name search (`q`), module filter (`module_id`) and
  date sort (`sort`)
- `GET|POST /api/cards`, `GET|PATCH /api/cards/:id`, `POST /api/cards/:id/archive`,
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
- 229 backend tests (the count of all lib and integration test binaries combined; the
  earlier figure of 119 recorded here was stale). No frontend test framework — that is a
  deliberate spec decision, not an
  omission.

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
    [`.mitis/sdd/2026-08-28-part4-bibble-theme/makka-pakka-palette.md`](../.mitis/sdd/2026-08-28-part4-bibble-theme/makka-pakka-palette.md).
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
    Practice (live), Mock test (disabled, "Arrives in part 5.") and Spaced repetition
    (disabled, "Arrives in part 7."). `startSession` calls
    `api.createSession({ mode, deck_ids: [deckId] })` and navigates to `/session/:id`.
    Buttons disable when `deck.card_count === 0`, which is the right guard —
    `card_count` already excludes archived cards (`backend/src/routes/decks.rs`).
  - **This means Part 5's entry point already exists.** The Mock test button is built,
    positioned and labelled; Part 5 enables it rather than inventing a screen. Same for
    SM-2 in build step 7.
  - **Multi-deck and module-wide sessions are no longer reachable from the UI.**
    `startSession` always sends exactly one deck. The backend and the client types still
    support both a list of decks and a whole module (`frontend/src/lib/api.ts`,
    `backend/src/routes/sessions.rs`), and that path is still tested — it is intact and
    unused, not removed. Whether Part 5 restores it is an open question, recorded under
    Next up.
  - **The "Show archived" switch was removed** and the list fetch is hardcoded to
    `archived: 'false'`. Deliberate — see the entry under Known-and-accepted minors for
    what it costs.
  - **The theme toggle names the palettes.** Its options now read "Makka Pakka (light)"
    and "Bibble (dark)", drawn with two hand-made SVGs in
    `frontend/src/components/icons/` rather than lucide's `Sun` and `Moon`. `Monitor`
    still marks System.
  - Two manual layout offsets on the deck page, `pl-11` on the container and `-ml-7` on
    the card list, aligning the card rows' drag grips outside the content column.

`/stats` is still a placeholder page.

## Next up

**Part 5: the mock test.** Part 4 is done and merged, and its entry point is already
built: the disabled "Mock test" button in the `/decks/:id` mode grid, labelled "Arrives in
part 5." Part 5 enables that button rather than adding a screen.

After Part 5: stats → SM-2 → embed the bundle and LAN binding.

**Three things Part 5 must resolve.** The first two are recorded in the Part 3 design doc;
the third was created by the `e09e76d` styling pass.

- `/next` re-rolls on reload. That is correct for practice, where an unanswered serve wrote
  no row and there is no ordered position to resume to. Under `target_count` each serve is
  consequential, so mock mode needs a stable serve.
- What a flashcard means in a mock test, where there is no feedback during the run but
  self-grading structurally needs the answer.
- **What a mock test is scoped to.** The deck-page entry point can only start a session for
  the one deck you are looking at, but a mock test standing in for the COS781 test is more
  plausibly the whole module. The backend already accepts both a deck list and a
  `module_id`, so this is a UI decision, not an API one: either mock tests are per-deck and
  match the button that starts them, or Part 5 brings back a way to pick a wider pool.

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

**Archive, never delete.** Cards are archived so their `reviews` rows keep meaning.
`reviews.card_id` deliberately has no `ON DELETE CASCADE` so a stray delete fails loudly.

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

**One rendering path.** `<Markdown>` in `frontend/src/components/Markdown.tsx` is the only
markdown renderer in the app, and it is why Part 2a shipped raw text everywhere. The card
list, the editor preview and Part 3's session runner all go through it. If you need different
behaviour, add a prop — a second renderer is the exact outcome the 2a/2b split existed to
prevent.

**KaTeX's fonts come from the npm package**, imported as `katex/dist/katex.min.css`. Do not
switch to a CDN. The Google Fonts `@import` in `globals.css` is already a known defect
deferred to build step 8; a second network dependency makes it worse.

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
- KaTeX and `react-markdown` roughly doubled the JS bundle (437 kB → 884 kB, 272 kB gzipped,
  the latter figure grown further by the deck card list redesign's three `@dnd-kit`
  packages), which now trips Vite's 500 kB chunk warning on every build. Harmless for a
  LAN-served app and deliberately not code-split yet, but build step 8 ("embed the bundle,
  LAN binding") is where it should be looked at.
- Google Fonts: `frontend/src/styles/globals.css` imports Quicksand and Inter over the
  network, so typography silently falls back offline or on a LAN-only phone. Deferred to
  build step 8, which is already the "embed the bundle, LAN binding" task, and noted there
  in the spec.

**Known-and-accepted minors**

- **Archived cards cannot be reached from the UI, deliberately.** `e09e76d` removed the
  "Show archived" switch and pinned the deck list to `archived: 'false'`, so archiving is
  one-way from the interface. Two knock-on facts, both left as they are: `unarchive(card)`
  in `DeckPage.tsx` is unreachable, because `card.archived` can never be true in a list
  fetched with that filter; and `POST /api/cards/:id/unarchive` now has no caller in the
  frontend. Neither was deleted — the endpoint is tested, `GET /api/cards` still takes
  `archived=all`, and "Archive, never delete" still holds at the data layer, so the
  capability is intact behind the API whenever a screen wants it back.
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

Work through `mitis:brainstorming` before designing Part 3, then `mitis:writing-plans`, then
`mitis:subagent-driven-development`. The three existing plans are worth reading as a format
reference — particularly how each task carries complete code, an explicit verify command,
and a `json:metadata` fence.

Two habits that earned their cost here: give reviewers the *both sides* of any seam they are
judging (a per-task review structurally cannot see an API-to-client mismatch), and demand
mutation evidence — a test that cannot fail is not a test.

**Run one implementer at a time.** Part 2a dispatched two concurrently because their file
lists were disjoint. Git's index is not per-file: one agent's `git add`/`commit` swept the
other's staged work into a commit labelled as something else, recoverable only because the
branch was local and unpushed. Read-only reviewers can run alongside an implementer; two
writers cannot.
