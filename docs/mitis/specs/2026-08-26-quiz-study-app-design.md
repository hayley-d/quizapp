# Quiz Study App — Design

**Date:** 2026-08-26
**Author:** Hayley Dodkins (design session with Claude)
**Status:** Approved for planning

## Purpose

A self-hosted quiz application for exam revision, replacing Quizlet. It must support
multiple-choice questions, short typed answers, and diagram questions where an image has a
label blanked out and the student types the missing label. First use case is the COS781
(Data Mining) test on **11 September 2026**.

Quizlet's limitations that motivate this: paywalled features, and no support for
image-based questions where the student interprets or completes a diagram.

## Non-goals

These are deliberately excluded from v1. They are recorded so the exclusion is a decision,
not an oversight.

- **Internet deployment, accounts, authentication.** The app runs on the local machine.
- **Tags.** Decks are the only organising mechanism.
- **Cards in multiple decks.** Each card belongs to exactly one deck.
- **Multi-select multiple choice** ("select all that apply").
- **Image hotspots / click-a-region answering.** Blanked-out diagrams are prepared by hand
  in an external image editor before upload.
- **Automatic card generation from lecture notes.** Cards are written by hand in the app.
- **Frontend test framework.**

## Stack

| Layer     | Choice                                  |
| --------- | --------------------------------------- |
| Backend   | Rust, axum, sqlx                        |
| Database  | SQLite (single file, in `data/`)        |
| Frontend  | React, Vite, TypeScript, shadcn/ui      |
| Styling   | Tailwind                                |
| Math      | KaTeX                                   |

Source notes are written in Obsidian markdown and use inline LaTeX (`$X, Y, Z$`,
`$10\ 000$`), so prompts, choices and answers all render markdown with KaTeX math.

### Repository location

The project lives at `/Users/hayley/Documents/side_projects/quizapp` as its own git
repository. It must **not** live inside `/Users/hayley/Documents/university/COS781`, which
is an Obsidian vault — Obsidian would attempt to index `node_modules`.

### Deployment model

Local, bound to the LAN address so the app is reachable from a phone on the same wifi while
the laptop is running. In development, the Vite dev server proxies `/api` to axum. For
actual use, the built React bundle is embedded into the Rust binary (`rust-embed`), so
studying is one command and one process with no separate build step.

Uploaded images are written to `data/images/` as files, with only the path stored in
SQLite. Blobs in the database would bloat it and make backup-by-copy awkward.

## Data model

```
modules        id, name
decks          id, module_id (nullable), name, description
cards          id, deck_id, kind, prompt_md, image_path (nullable),
               answer_md (nullable), explanation_md (nullable), archived
choices        id, card_id, text_md, is_correct, position        -- mc_single only
accepted       id, card_id, text, normalised, is_primary         -- short_answer only
sessions       id, mode, deck_ids (json), target_count, started_at, ended_at
reviews        id, card_id, session_id, answered_at, given, correct, overridden, ms
schedule       card_id (pk), due_at, interval_days, ease, reps, lapses
```

### Organisation: modules and decks

A **deck** is the primary unit and is scoped to a test — how revision is actually grouped
in practice (e.g. "COS781 Test 1"). Decks optionally belong to a **module** (e.g. COS781),
so decks for one subject can be studied together. `decks.module_id` is nullable: a deck
need not belong to a module.

Deck names are unique within a module, and unique among the decks that belong to no
module. Creating a duplicate is rejected with a conflict rather than silently allowed —
two decks called "Test 1" in the same module would be indistinguishable while revising.

Study sessions select one or more decks, or a whole module (which expands to its decks).

### Card kinds

`cards.kind` is a discriminator with three values, each using a different side table:

| kind           | Answer stored in | Graded by                              |
| -------------- | ---------------- | -------------------------------------- |
| `mc_single`    | `choices`        | Auto: selected choice `is_correct`      |
| `short_answer` | `accepted`       | Auto: normalised match against accepted |
| `flashcard`    | `answer_md`      | Self-graded: again / hard / good / easy |

A diagram question is not a fourth kind — it is a `short_answer` card with `image_path`
set. The image is prepared externally with the label erased.

One `cards` table with a discriminator, rather than three separate tables, because every
study mode, every statistic and the scheduler treat cards uniformly; three tables would
mean three-way unions throughout. The trade-off is that the schema cannot enforce
per-kind invariants, so these are validated in Rust on write:

- `mc_single`: at least 2 choices, exactly 1 with `is_correct = true`
- `short_answer`: at least 1 `accepted` row, exactly 1 with `is_primary = true`
- `flashcard`: `answer_md` is non-empty
- `choices` and `accepted` rows may only exist for the matching kind

### Accepted answers

`accepted` is a table rather than a delimited column because the "I was actually right"
override inserts a new accepted answer at study time.

`normalised` is the comparison key, computed on insert so matching is an indexed lookup
rather than a scan that re-normalises every row. Normalisation is:

1. Unicode NFKC
2. Lowercase
3. Replace punctuation with spaces
4. Collapse internal whitespace runs to a single space and trim

Punctuation becomes a space rather than being deleted so that "k-means" and "k means" produce the same key, which is the case this normalisation exists to handle.

`is_primary` marks the wording shown as "the answer" when the student is wrong.

### Reviews are append-only

`reviews` records every answer, forever. Rows are never updated or deleted, with one
exception: the override endpoint sets `correct = true, overridden = true` on the row it
targets, so an unfair miss does not permanently distort the statistics.

Practice-mode weighting, mock test results and progress-over-time statistics are all
queries over this table. A scheduling bug therefore cannot destroy history — in the worst
case `schedule` is recomputed from `reviews`.

### Schedule exists from day one

One `schedule` row is created per card at card creation, whether or not SM-2 has been
built. Practice and mock test ignore it entirely. This avoids a migration over cards that
have already been hand-written.

### Archiving, not deleting

Cards are archived, never hard-deleted. A hard delete would orphan the card's `reviews`
rows and silently rewrite history. Archived cards are excluded from sessions but retain
their past.

## API

The governing rule: **the correct answer is never sent to the client before the student
answers.** All grading happens server-side. This matters for the mock test in particular,
where the answer key must not be sitting in the browser.

```
GET  /api/modules                  list modules
POST /api/modules                  create
GET  /api/decks                    list decks (filter by module)
POST /api/decks                    create
PATCH /api/decks/:id               rename, re-parent, edit description

GET  /api/cards                    list (filter by deck, kind, archived)
GET  /api/cards/:id                full card incl. choices/accepted (authoring view)
POST /api/cards                    create; choices/accepted nested, one transaction
PATCH /api/cards/:id               update; nested children replaced in one transaction
POST /api/cards/:id/archive        archive
POST /api/cards/:id/image          multipart upload -> data/images/, returns path

POST /api/sessions                 {mode, deck_ids | module_id, target_count?}
GET  /api/sessions/:id/next        next card: prompt, image, shuffled choices, no key
POST /api/sessions/:id/answer      {given} -> {correct, expected, explanation}
POST /api/sessions/:id/finish      end session, return results summary
POST /api/reviews/:id/override     "I was right": insert accepted, flip review

GET  /api/stats?deck_ids=          accuracy overall, per deck, per card, over time
```

`GET /api/sessions/:id/next` shuffles `choices` per serve, so the student learns the answer
rather than its position.

`POST /api/sessions/:id/answer` writes the `reviews` row and, in SM-2 mode, updates
`schedule`. Its response carries the verdict, the primary expected answer, and the
explanation if one exists. In mock test mode the response withholds the verdict until
`/finish`.

## Study engine

Three modes draw from one card pool (non-archived cards in the selected decks).

**Practice** — weighted sampling, biased toward weakness:

- Never-seen cards receive the highest weight
- Then cards by recent miss rate (recent reviews weighted above old ones)
- Then staleness — time since last seen
- A card may not repeat within a rolling window of ~8 cards, so it does not feel like a loop
- The session has no end; the student stops when finished

**Mock test** — `target_count` cards sampled uniformly at random. No feedback during the
run. `/finish` returns a score and every missed card with its expected answer and
explanation.

**SM-2** — standard SuperMemo-2 intervals and ease factors, over cards whose
`schedule.due_at` has passed. Quality mapping:

| Outcome                   | Quality |
| ------------------------- | ------- |
| Auto-graded correct       | 4       |
| Correct via override      | 4       |
| Auto-graded wrong         | 2       |
| Flashcard: again/hard/good/easy | 1 / 3 / 4 / 5 |

Note on SM-2 with 16 days until the test: spaced repetition is built for long-horizon
retention and its scheduling will barely get to prove itself before 11 September. It is
included because it makes the app worth keeping for the rest of the module, and it is
sequenced last so it never blocks studying.

### Testable core

Normalisation, answer grading, practice-mode weighting and SM-2 are pure functions in
their own Rust modules with no database access. They form the bulk of the test suite.

## Frontend

| Route              | Purpose                                     |
| ------------------ | ------------------------------------------- |
| `/study`           | Pick mode + decks, start a session          |
| `/session/:id`     | The study runner                            |
| `/decks`           | Deck and module management                  |
| `/decks/:id`       | Cards in a deck                             |
| `/cards/new`, `/cards/:id/edit` | Card editor                    |
| `/stats`           | Accuracy overall, per deck, weakest cards   |

**Keyboard-first, both sides.** The card editor is the app's most-used screen — cards are
written by hand — so authoring is type-prompt, tab through options, mark the correct one,
save-and-next, without reaching for the mouse. In a session, `1`–`4` selects a
multiple-choice option and `Enter` submits and advances, making a practice run pure
keyboard.

Responsive down to phone width, since sessions will run on a phone over wifi.

## Visual design: Bibble theme

Themed after Bibble from *Barbie: Fairytopia* — evoked through palette, shape and motion
rather than character art.

- **Dark base** (primary): deep twilight background; turquoise/cyan primary; magenta and
  lavender accents; gold for correct answers; iridescent gradients on highlights
- **Light base**: the pale aqua and lilac version of the same palette
- Rounded and softly glowing rather than sharp-edged
- A rounded display face (Quicksand) against a clean body sans
- A sparkle burst on a correct answer; a wing-flutter flourish on an answer streak

Both animations respect `prefers-reduced-motion`, and neither blocks advancing to the next
question — during a study run, responsiveness beats flourish.

## Error handling

- **Validation** — per-kind card invariants are checked in Rust on write and returned as
  structured field errors the editor renders inline. The editor must never lose typed
  content on a rejected save.
- **Uploads** — images are size- and type-checked; a rejected upload leaves the card intact.
- **Sessions** — a session with no eligible cards fails at creation with a clear message
  rather than producing an empty runner. Reloading mid-session resumes from `reviews`;
  session state is not held in browser memory.
- **Database** — writes touching multiple tables (card + children, answer + schedule) run in
  a single transaction.

## Testing

- **Unit** — normalisation, grading, practice weighting, SM-2 (pure functions)
- **Integration** — API endpoints against a temporary SQLite file, including the answer-key
  leakage rule: no session endpoint may return correctness data before an answer is submitted
- **Manual** — frontend, per the non-goals

## Build sequencing

Ordered so there is a working vertical slice early and the schema never needs migrating
over hand-written cards.

1. Schema and migrations (all tables, including `schedule`), decks and modules CRUD
2. Card editor and card CRUD — all three kinds, image upload, KaTeX rendering
3. Practice mode: session runner, grading, override, `reviews`
4. Bibble theme applied across the app
5. Mock test mode and results screen
6. Stats
7. SM-2 scheduling
8. Embed the bundle into the binary; LAN binding; phone layout pass — including
   vendoring the Quicksand and Inter woff2 files locally. Part 1 loads them from Google
   Fonts, which silently falls back to the system stack whenever the machine is offline or
   a phone on the LAN cannot reach Google. Embedding the bundle for phone use is exactly
   when that stops being acceptable.
