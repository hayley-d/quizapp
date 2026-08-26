# quizapp

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

## Environment

| Var                 | Default                              |
| ------------------- | ------------------------------------ |
| `QUIZAPP_BIND`      | `127.0.0.1:3000`                     |
| `DATABASE_URL`      | `sqlite://data/quizapp.db?mode=rwc`  |
| `QUIZAPP_DATA_DIR`  | `data`                               |
| `RUST_LOG`          | `info,sqlx=warn`                     |

## After ANY change to SQL or migrations

sqlx checks queries at compile time against a committed offline cache:

    DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx migrate run
    DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
    git add .sqlx backend/migrations

A build failing with "set DATABASE_URL to use query macros online" means the
cache is stale. Re-run `cargo sqlx prepare --workspace`.

## Tests

    cargo test                        # unit + API integration (temp SQLite per test)
    cargo clippy -- -D warnings
    cd frontend && pnpm exec tsc --noEmit && pnpm build

Frontend has no test framework by design (see the spec's non-goals).

## Database access (DBeaver, sqlite3)

SQLite is a single file, so there is no host, port, username or password.

**DBeaver:** New Connection → **SQLite** → Path:

    /Users/hayley/Documents/side_projects/quizapp/data/quizapp.db

Leave user/password empty; let DBeaver download the SQLite JDBC driver when it offers.

Two things that will bite you if you don't know them:

- **Foreign keys are OFF in any client that doesn't ask for them.** `PRAGMA foreign_keys`
  is per-connection, and the app turns it on for its own pool (`backend/src/db.rs`).
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

## Backups

The whole database is `data/quizapp.db`; images will live in `data/images/`.
Copy `data/` to back up.
