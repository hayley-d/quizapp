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

## Backups

The whole database is `data/quizapp.db`; images will live in `data/images/`.
Copy `data/` to back up.
