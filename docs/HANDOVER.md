# Handover

Read this first if you are picking up this project without the conversation that built it.

**Last updated:** 2026-08-27, at `part2b-images-markdown` (Part 2b complete, not yet merged
to `main`).

## What this is

A self-hosted quiz app for exam revision, replacing Quizlet. The driving deadline is the
COS781 (Data Mining) test on **11 September 2026**. Full design: [`mitis/specs/2026-08-26-quiz-study-app-design.md`](mitis/specs/2026-08-26-quiz-study-app-design.md) — that
document is the record of what the app is meant to be, and it is kept current.

## Where things stand

Parts 1, 2a and 2b of the spec's build sequencing are **done**. Parts 1 and 2a are merged to
`main`; Part 2b lives on `part2b-images-markdown`, gate-clean, awaiting merge. Concretely,
working today:

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
- `/decks/:id` screen: a deck's card list with kind badges, archive/unarchive, and a
  show-archived toggle
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
- 106 backend tests. No frontend test framework — that is a deliberate spec decision, not an
  omission.

`/study` and `/stats` are placeholder pages.

## Next up

**Part 3: practice mode.** The session runner, grading against `accepted.normalised`, the
"I was right" override, and `reviews` rows. This is the first feature that reads cards rather
than writing them, and the first consumer of `POST /api/sessions`.

Two things already in place that Part 3 must use rather than reinvent: the `<Markdown>`
component (the session runner is its third consumer — do not add a fourth rendering path),
and `normalise()`, which computes the same key grading will look up.

After that: Bibble theme pass → mock test → stats → SM-2 → embed the bundle and LAN binding.

## Running it

```bash
cargo run                    # API on http://127.0.0.1:3000
cd frontend && pnpm dev      # UI  on http://localhost:5273
```

Full setup, env vars, the sqlx workflow and DBeaver access are in [`../README.md`](../README.md).

## Environment quirks that will otherwise cost you an hour each

- **Port 5273, not 5173.** 5173 is permanently occupied by an unrelated project on this
  machine.
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
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

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
`backend/src/db.rs`, not from anything in the schema. Any other client — DBeaver, the
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

**Housekeeping**

- `data/quizapp.db` holds verification debris from Part 1 and Part 2a runs (modules like
  `REVIEW_MOD_1`, decks like `kinetics 100%`). Clear it before writing real cards — it
  regenerates on startup.
- KaTeX and `react-markdown` roughly doubled the JS bundle (437 kB → 833 kB, 254 kB gzipped),
  which now trips Vite's 500 kB chunk warning on every build. Harmless for a LAN-served app
  and deliberately not code-split yet, but build step 8 ("embed the bundle, LAN binding") is
  where it should be looked at.
- Google Fonts: `frontend/src/styles/globals.css` imports Quicksand and Inter over the
  network, so typography silently falls back offline or on a LAN-only phone. Deferred to
  build step 8, which is already the "embed the bundle, LAN binding" task, and noted there
  in the spec.

**Known-and-accepted minors**

- `AppError::Conflict` is constructed nowhere in production code — real 409s arrive via the
  sqlx unique-violation path.
- `DecksPage`'s empty state keys on there being no groups, so the onboarding copy does not
  show when unparented decks exist but no modules do. Cosmetic.
- `AppError::fk_as` takes `&str` while `AppError::validation` is generic over `Into<String>`.
  Both call sites pass literals; generalising it now would be churn.
- `patch_unknown_card_is_404`'s `count(cards) == 0` assertion cannot fail — each test gets a
  fresh empty database. Not a false claim, just an assertion carrying no weight; the 404
  assertion above it is the real test.
- `CardEditorPage` renders an inline error slot for `explanation_md`, which the validator
  never emits. Dead but harmless; Part 2b may give it a use.
- Duplicate accepted answers that normalise to the same key are accepted — `validate` does
  not dedupe and `idx_accepted_card_normalised` is a plain, non-unique index. Harmless in
  2a, but Part 3's grading lookup will meet it, so it belongs on the record now.

## Where the record lives

- **Spec** — [`mitis/specs/2026-08-26-quiz-study-app-design.md`](mitis/specs/2026-08-26-quiz-study-app-design.md). Kept current; amended when
  the implementation legitimately diverged (e.g. deck-name uniqueness per module).
- **Plans** — `mitis/plans/*.md` plus their `.tasks.json`. These carry the full per-task
  code, acceptance criteria and verification commands. All are marked complete.
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
