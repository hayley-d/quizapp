# quizapp

**Picking this up cold?** Start with [`docs/HANDOVER.md`](docs/HANDOVER.md) — current state,
what's next, environment quirks, and the conventions that cost a fix round to learn.

Self-hosted quiz app for exam revision. Design: `docs/mitis/specs/2026-08-26-quiz-study-app-design.md`.

## Prerequisites

- Rust toolchain (this machine runs a standalone nightly; there is no `rust-toolchain.toml` pin)
- Node 20+ and pnpm 11+ (`pnpm` is this project's package manager; `packageManager` in
  frontend/package.json pins it so corepack cannot silently fall back to npm)
- `cargo install sqlx-cli --no-default-features --features rustls,sqlite`

## First-time setup

    mkdir -p data
    export DATABASE_URL="sqlite://data/quizapp.db?mode=rwc"
    cargo sqlx migrate run --source backend/migrations
    cd frontend && pnpm install && cd ..

## Running (two terminals)

    cargo run                 # API on http://127.0.0.1:3000
    cd frontend && pnpm dev     # UI on http://localhost:5273 (proxies /api)

Migrations run automatically at startup, so `cargo run` on a fresh machine
creates and migrates `data/quizapp.db` by itself.

This is the *development* loop, with Vite serving the UI and hot-reloading it.
For actual studying, use the single binary below.

## Deploying: one binary, reachable from a phone

The React bundle is compiled into the Rust binary, so studying is one process
with no separate frontend server:

    SQLX_OFFLINE=true cargo build --release
    QUIZAPP_BIND=0.0.0.0:3000 ./target/release/quizapp

`cargo build` runs `pnpm build` for you via `backend/build.rs`, so the embedded
bundle can never be out of date with the source.

The startup line prints the address a phone should open — when bound to
`0.0.0.0` it resolves and logs the machine's actual LAN address rather than the
unopenable `http://0.0.0.0:3000`:

    INFO quizapp: listening on http://192.168.2.161:3000

Both the laptop and the phone have to be on the same wifi. The app has no
authentication, which is why `QUIZAPP_BIND` still defaults to `127.0.0.1:3000`
— binding every interface is a deliberate per-run choice, not the default.

Everything is served from that one origin: the UI at `/`, the API under `/api`,
and uploaded images under `/images`. Client-side routes such as `/decks/3`
survive a hard refresh. No internet is required — the fonts are bundled.

### Building without node

`backend/build.rs` shells out to `pnpm`. If pnpm is not on `PATH` it says so and
names the two ways round it: point `QUIZAPP_PNPM` at an absolute path, or set
`QUIZAPP_SKIP_FRONTEND_BUILD=1` to reuse an already-built `frontend/dist`.

The build script only reruns when something under `frontend/` that `vite build`
actually reads has changed, so backend-only edits do not pay for a Vite build.
One caveat inherited from how cargo watches directories: it compares the newest
mtime in the tree, so *deleting* a frontend file does not on its own trigger a
rebuild. `touch frontend/index.html` if you need to force one.

## Routes

Frontend (React Router, client-side):

    /decks                     deck list: search, module filter, sort
    /decks/:id                 deck detail: stats, card list, and where a session starts
    /cards/new?deck_id=        new card, all three kinds
    /cards/:id/edit            edit an existing card
    /session/:id               the practice and spaced-repetition runner
    /mock/:id                  the mock test runner

Cards API, added in Part 2a:

    GET   /api/decks/:id               one deck, with module_name and card_count
    GET   /api/cards                   list; filter by deck, kind, archived
    GET   /api/cards/:id               full card incl. choices/accepted (authoring view)
    POST  /api/cards                   create; choices/accepted nested, one transaction
    PATCH /api/cards/:id               update; nested children replaced in one transaction
    POST  /api/cards/:id/archive       archive
    POST  /api/cards/:id/unarchive     restore

Moving content between devices:

    GET   /api/decks/:id/export        one deck as a transfer file
    GET   /api/modules/:id/export      every deck in a module
    POST  /api/import                  create decks from a transfer file

Full API and the data model are in `docs/mitis/specs/2026-08-26-quiz-study-app-design.md`.

## Environment

| Var                 | Default                              |
| ------------------- | ------------------------------------ |
| `QUIZAPP_BIND`      | `127.0.0.1:3000`                     |
| `DATABASE_URL`      | `sqlite://data/quizapp.db?mode=rwc`  |
| `QUIZAPP_DATA_DIR`  | `data`                               |
| `RUST_LOG`          | `info,sqlx=warn`                     |

Build-time only, read by `backend/build.rs`:

| Var                            | Effect                                        |
| ------------------------------ | --------------------------------------------- |
| `QUIZAPP_PNPM`                 | path to the pnpm binary (default: `pnpm`)     |
| `QUIZAPP_SKIP_FRONTEND_BUILD`  | reuse the existing `frontend/dist`            |

## After ANY change to SQL or migrations

sqlx checks queries at compile time against a committed offline cache:

    DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx migrate run
    DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
    git add .sqlx backend/migrations

A build failing with "set DATABASE_URL to use query macros online" means the
cache is stale. Re-run `cargo sqlx prepare --workspace`.

## Tests

    cargo test                        # unit + API integration (temp SQLite per test)
    cargo clippy --all-targets -- -D warnings
    SQLX_OFFLINE=true cargo build
    python3 frontend/scripts/check-contrast.py
    cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint

`--all-targets` matters: without it clippy does not build the test targets.
`tsc -b`, not bare `tsc`: `frontend/tsconfig.json` is a solution file with
`"files": []`, so `tsc --noEmit` reads it, finds nothing, and exits 0 whatever
the code says.

Frontend has no test framework by design (see the spec's non-goals).

## Database access (DBeaver, sqlite3)

SQLite is a single file, so there is no host, port, username or password.

**DBeaver:** New Connection → **SQLite** → Path:

    /Users/hayley/Documents/side_projects/quizapp/data/quizapp.db

Leave user/password empty; let DBeaver download the SQLite JDBC driver when it offers.

Two things that will bite you if you don't know them:

- **Foreign keys are OFF in any client that doesn't ask for them.** `PRAGMA foreign_keys`
  is per-connection, and the app turns it on for its own pool (`backend/src/database.rs`).
  DBeaver does not. So a delete you run in DBeaver can orphan rows that the app itself
  would have refused. Run `PRAGMA foreign_keys = ON;` in the DBeaver SQL editor first if
  you intend to delete anything.
- **Writers block each other.** The journal mode is the default (`delete`), not WAL, so
  DBeaver holding a write transaction while the app is running gives
  `database is locked` (the app waits 5s, then errors). Either stop `cargo run` before
  writing from DBeaver, or switch the file to WAL once — it persists, and lets DBeaver
  read while the app writes:

      sqlite3 data/quizapp.db "PRAGMA journal_mode = WAL;"

**sqlite3 CLI, from the repo root:**

    sqlite3 data/quizapp.db                       # interactive shell
    sqlite3 data/quizapp.db ".tables"             # list tables
    sqlite3 data/quizapp.db ".schema decks"       # one table's DDL
    sqlite3 data/quizapp.db "SELECT * FROM modules;"
    sqlite3 data/quizapp.db "SELECT version, description, success FROM _sqlx_migrations;"

Nine tables: the eight from the data model (`modules`, `decks`, `cards`, `choices`,
`accepted`, `sessions`, `reviews`, `schedule`) plus sqlx's own `_sqlx_migrations`.
The schema is `backend/migrations/0001_init.sql`; the data model is documented in
`docs/mitis/specs/2026-08-26-quiz-study-app-design.md`.

## Moving decks between devices

A deck authored on one machine reaches another as a single `.quizapp.json` file.
**Export** on a deck page writes that deck; the download button beside a module in
the Modules dialog writes every deck in it. **Import** on `/decks` reads one back.

The file carries content only — prompts, choices, accepted answers, explanations,
card order, the archived flag, and any uploaded images inlined as base64. It
deliberately does not carry review history, sessions or spaced-repetition
schedules, so an imported deck starts unstudied and each device keeps its own
progress.

Import never modifies or deletes anything. A module named in the file is reused if
one of that name exists and created if not; a deck whose name is already taken in
that module arrives as `… (2)`, and the toast says so. Re-importing the same file
therefore makes another copy rather than overwriting the first, and an unwanted
import is undone by deleting the deck it created.

Because image names are the SHA-256-derived names uploads already use, an image
that is already on the destination is not written twice.

The format is plain, pretty-printed JSON and stable at `format_version: 1`, so
writing a deck's worth of cards by hand in an editor and importing it works.
Only `format`, `format_version`, `decks`, and each deck's `name` are required.

## Backups

The whole database is `data/quizapp.db`; images will live in `data/images/`.
Copy `data/` to back up.
