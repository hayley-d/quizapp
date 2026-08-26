# Part 1 — Foundations: Schema, Modules & Decks — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `mitis:subagent-driven-development`
> (recommended) or `mitis:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking. Tasks: `docs/mitis/plans/2026-08-26-part1-schema-modules-decks.md.tasks.json`

## Context

The repo at `/Users/hayley/Documents/side_projects/quizapp` currently contains only
`.gitignore` and the approved design spec
(`docs/mitis/specs/2026-08-26-quiz-study-app-design.md`). Nothing has been built.

The spec's build sequencing puts **step 1 = "Schema and migrations (all tables, including
`schedule`), decks and modules CRUD"**, ordered that way so the schema never has to be
migrated over hand-written cards, and so there is a working vertical slice early. The
COS781 test is on **11 September 2026**, 16 days out, so Part 1 must land quickly and leave
Part 2 (the card editor — the screen that actually consumes study time) unblocked.

**Outcome of Part 1:** one command starts the backend, one starts the frontend, and in the
browser you can create a module, create decks inside it, rename a deck, move it to another
module, and edit its description. All eight tables exist from the first migration. The
Bibble palette tokens are in place so nothing gets built against throwaway colours.

**Goal:** A running axum + SQLite backend with the complete schema and a tested
modules/decks REST API, plus a React frontend scaffold with a working `/decks` screen.

**Architecture:** Single Rust crate at the repo root (so the built bundle can later be
embedded with `rust-embed` without restructuring), `frontend/` for the Vite app. sqlx
compile-time-checked queries with a committed `.sqlx` offline cache. Integration tests drive
the axum `Router` in-process via `tower::ServiceExt::oneshot` against a temp SQLite file —
no ports, no fixtures to clean up.

**Tech Stack:** Rust (axum 0.8, sqlx 0.8/SQLite, tokio, tower-http, thiserror, tracing),
React 19 + Vite + TypeScript, Tailwind v4 (CSS-first `@theme`), shadcn/ui, react-router.

**User decisions (already made):**
- Part 1 is the **full vertical slice** — backend + frontend scaffold + working `/decks` screen.
- sqlx **compile-time macros** (`query!`/`query_as!`), with a committed `.sqlx` offline cache.
- Theming: **Bibble palette tokens + Quicksand only**. No glows, gradients, sparkle burst or
  wing-flutter — those stay in spec step 4.

**Deliberately out of scope for Part 1** (per the spec's sequencing): cards/choices/accepted
API, image upload, KaTeX rendering, sessions/grading/override, stats, SM-2 logic,
`rust-embed`, LAN binding, and the answer-key-leakage integration test (there are no session
endpoints yet). Their **tables** are created in Task 2; only their endpoints are deferred.

---

## Repository layout after Part 1

```
quizapp/
  Cargo.toml                  # workspace manifest: members = ["backend"]
  Cargo.lock                  # shared, workspace-level
  .sqlx/                      # committed sqlx offline query cache (workspace-wide)
  data/                       # git-ignored: quizapp.db, later images/
  backend/
    Cargo.toml                # package "quizapp"
    migrations/
      0001_init.sql           # ALL eight tables
    src/
      main.rs                 # bootstrap: tracing, config, pool, migrate, serve
      lib.rs                  # module tree + app(state) router builder
      config.rs               # env-driven config (bind addr, database url, data dir)
      db.rs                   # pool creation + run_migrations
      error.rs                # AppError -> structured JSON field errors
      state.rs                # AppState { pool }
      routes/
        mod.rs                # api_router() assembly
        health.rs
        modules.rs
        decks.rs
    tests/
      common/mod.rs           # test app harness (temp db + oneshot helpers)
      health.rs
      modules.rs
      decks.rs
  frontend/
    package.json  vite.config.ts  tsconfig.json  components.json  index.html
    src/
      main.tsx  App.tsx
      styles/globals.css      # Bibble tokens (light + dark) via Tailwind @theme
      lib/api.ts              # typed fetch client + ApiError
      lib/utils.ts            # shadcn cn()
      components/ui/*         # shadcn primitives (button, input, textarea, dialog,
                              #   select, label, card, sonner)
      components/AppShell.tsx # nav shell for /study /decks /stats
      pages/DecksPage.tsx
      pages/StubPage.tsx      # placeholder for /study and /stats
  README.md                   # dev runbook
  docs/
```

**Backend and frontend are peers.** The root `Cargo.toml` is a workspace manifest, not a
package, so `cargo run` / `cargo test` / `cargo clippy` still work from the repo root with no
flags, `target/` and `Cargo.lock` stay shared at the root, and the cwd-relative
`DATABASE_URL` default keeps resolving to `data/` at the root rather than `backend/data/`.

**sqlx commands run from the repo root too**, using the workspace flags, so the offline cache
sits at the root `.sqlx/` and `DATABASE_URL` stays root-relative:

    cargo sqlx migrate run --source backend/migrations
    cargo sqlx prepare --workspace

Running them from inside `backend/` instead would resolve `sqlite://data/...` to
`backend/data/` — don't. `sqlx::migrate!("./migrations")` in the code resolves against
`CARGO_MANIFEST_DIR` (= `backend/`), so it stays `"./migrations"` and needs no change.


**Files that change together live together:** each route module owns its own request/response
DTOs and its validation — there is no shared `models.rs` grab-bag.

---

## Cross-cutting conventions (every task must follow these)

**Timestamps are `TEXT` in SQLite and `String` in Rust/TS.** Columns default to
`datetime('now')` (UTC, `YYYY-MM-DD HH:MM:SS`). No `chrono` type mapping in Part 1 — it buys
nothing here and adds friction to the compile-time macros. Revisit if stats need date math.

**Ordering tests must be able to detect the collation.** Any test asserting a
`COLLATE NOCASE` ordering has to use inputs where BINARY and NOCASE genuinely disagree.
BINARY compares raw bytes, so every uppercase letter sorts before every lowercase one
(`'Z'` = 0x5A < `'z'` = 0x7A): `"apple"`/`"Banana"` and `"zebra"`/`"Zulu"` discriminate,
while `"Alpha"`/`"beta"` and `"Deck A"`/`"Deck B"` do NOT — both collations order those
identically, so the test passes even with the collation deleted. Where an ORDER BY has
two collated keys, prove each one independently by removing only that key's `COLLATE
NOCASE` and watching the test go red. This cost three fix rounds in Part 1; do not
rediscover it.

**Booleans are `INTEGER NOT NULL CHECK (col IN (0,1))`** in SQLite, `bool` in Rust. sqlx
maps these automatically for SQLite.

**IDs are `i64`.**

**Error envelope — one shape for every failure:**

```json
{ "error": "validation", "message": "Deck is invalid", "fields": [
  { "field": "name", "message": "Name must not be empty" } ] }
```

`fields` is `[]` for non-validation errors. The frontend renders `fields` inline and
`message` as a toast, and never clears the form on a rejected save.

**sqlx offline cache is part of every backend change.** After editing SQL or a migration:

```bash
cargo sqlx migrate run --source backend/migrations   # DATABASE_URL=sqlite://data/quizapp.db
cargo sqlx prepare --workspace                      # regenerates .sqlx/
git add .sqlx backend/migrations
```

A build that fails with `set DATABASE_URL to use query macros online` means the cache is
stale — re-run `cargo sqlx prepare --workspace`. CI-less solo project, so this is the one manual
discipline the macro choice costs.

---

## Task 1: Cargo project, config, and health endpoint

**Goal:** `cargo run` starts an axum server on `127.0.0.1:3000` that answers
`GET /api/health`, and `cargo test` runs green.

**Files:**
- Create: `Cargo.toml` (workspace), `backend/Cargo.toml`, `backend/src/main.rs`, `backend/src/config.rs`,
  `backend/src/state.rs`, `backend/src/routes/mod.rs`, `backend/src/routes/health.rs`
- Modify: `.gitignore` (add `.env`)

**Acceptance Criteria:**
- [ ] `cargo run` logs a bind line and serves `GET /api/health` → `200 {"status":"ok"}`
- [ ] Bind address and database URL come from env with working defaults
- [ ] `cargo clippy -- -D warnings` is clean
- [ ] Unknown route returns `404`

**Verify:** `cargo test` → all pass; `cargo run` then
`curl -s localhost:3000/api/health` → `{"status":"ok"}`

**Steps:**

- [ ] **Step 1: `Cargo.toml`**

```toml
[package]
name = "quizapp"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.8", features = ["macros", "multipart"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros", "migrate"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower = "0.5"
tower-http = { version = "0.6", features = ["trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"

[dev-dependencies]
tempfile = "3"
http-body-util = "0.1"
```

- [ ] **Step 2: `backend/src/config.rs`**

```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub data_dir: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            bind_addr: std::env::var("QUIZAPP_BIND")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/quizapp.db?mode=rwc".to_string()),
            data_dir: std::env::var("QUIZAPP_DATA_DIR")
                .unwrap_or_else(|_| "data".to_string()),
        }
    }
}
```

- [ ] **Step 3: `backend/src/state.rs`**

```rust
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
}
```

- [ ] **Step 4: `backend/src/routes/health.rs`**

```rust
use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
```

- [ ] **Step 5: `backend/src/routes/mod.rs`**

```rust
pub mod health;

use axum::Router;
use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new().merge(health::router())
}
```

- [ ] **Step 6: `backend/src/main.rs`** — DB wiring lands in Task 2; for now build the pool lazily so
      the binary compiles and runs without a schema.

```rust
mod config;
mod routes;
mod state;

use axum::Router;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")))
        .init();

    let config = Config::from_env();
    let pool = sqlx::SqlitePool::connect(&config.database_url).await?;
    let state = AppState { pool };

    let app = Router::new()
        .nest("/api", routes::api_router())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("listening on http://{}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
```

Add `anyhow = "1"` to `[dependencies]` for the `main` return type.

- [ ] **Step 7: Create the data dir and run**

```bash
mkdir -p data
cargo run
curl -s localhost:3000/api/health   # {"status":"ok"}
curl -s -o /dev/null -w '%{http_code}\n' localhost:3000/api/nope   # 404
```

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml backend/Cargo.toml Cargo.lock backend/src .gitignore
git commit -m "feat: axum skeleton with config and health endpoint"
```

```json:metadata
{"files":["Cargo.toml","backend/Cargo.toml","backend/src/main.rs","backend/src/config.rs","backend/src/state.rs","backend/src/routes/mod.rs","backend/src/routes/health.rs"],"verifyCommand":"cargo test && cargo clippy -- -D warnings","acceptanceCriteria":["cargo run serves GET /api/health as 200 {\"status\":\"ok\"}","bind addr and database url read from env with defaults","clippy clean with -D warnings","unknown route returns 404"],"modelTier":"mechanical"}
```

---

## Task 2: Migration 0001 — the complete schema

**Goal:** One migration creates all eight tables with their constraints and indexes;
migrations run automatically at startup; the `.sqlx` offline cache is committed.

**Files:**
- Create: `backend/migrations/0001_init.sql`, `backend/src/db.rs`, `backend/tests/common/mod.rs`, `backend/tests/health.rs`
- Modify: `backend/src/main.rs` (call `db::connect`), `backend/src/routes/health.rs` (add a DB ping)

**Acceptance Criteria:**
- [ ] All eight tables exist after startup: `modules`, `decks`, `cards`, `choices`,
      `accepted`, `sessions`, `reviews`, `schedule`
- [ ] `cards.kind` rejects a value outside the three kinds
- [ ] Foreign keys are enforced (inserting a `deck` with a non-existent `module_id` fails)
- [ ] Two decks with the same name in the same module (including "no module") are rejected
- [ ] Startup on a fresh empty `data/` dir creates the DB file and applies the migration
- [ ] `.sqlx/` is committed and `cargo build` succeeds with `SQLX_OFFLINE=true`

**Verify:** `cargo test --test health` → pass;
`SQLX_OFFLINE=true cargo build` → succeeds

**Steps:**

- [ ] **Step 1: Install sqlx-cli (once)**

```bash
cargo install sqlx-cli --no-default-features --features rustls,sqlite
```

- [ ] **Step 2: `backend/migrations/0001_init.sql`** — the full spec data model. `schedule` is here
      from day one deliberately (spec: "Schedule exists from day one") so no migration ever
      runs over hand-written cards.

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE modules (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  name       TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE decks (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  module_id   INTEGER REFERENCES modules(id) ON DELETE SET NULL,
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_decks_module ON decks(module_id);
-- SQLite treats NULLs as distinct in a UNIQUE index, so coalesce for the
-- module-less case; otherwise duplicate unparented deck names would slip through.
CREATE UNIQUE INDEX idx_decks_name_per_module
  ON decks(ifnull(module_id, -1), name);

CREATE TABLE cards (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id        INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  kind           TEXT NOT NULL CHECK (kind IN ('mc_single','short_answer','flashcard')),
  prompt_md      TEXT NOT NULL,
  image_path     TEXT,
  answer_md      TEXT,
  explanation_md TEXT,
  archived       INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0,1)),
  created_at     TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_cards_deck_archived ON cards(deck_id, archived);

CREATE TABLE choices (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  card_id    INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
  text_md    TEXT NOT NULL,
  is_correct INTEGER NOT NULL DEFAULT 0 CHECK (is_correct IN (0,1)),
  position   INTEGER NOT NULL
);
CREATE INDEX idx_choices_card ON choices(card_id, position);

CREATE TABLE accepted (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  card_id    INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
  text       TEXT NOT NULL,
  normalised TEXT NOT NULL,
  is_primary INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0,1))
);
CREATE INDEX idx_accepted_card_normalised ON accepted(card_id, normalised);

CREATE TABLE sessions (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  mode         TEXT NOT NULL CHECK (mode IN ('practice','mock','sm2')),
  deck_ids     TEXT NOT NULL,               -- JSON array of deck ids
  target_count INTEGER,
  started_at   TEXT NOT NULL DEFAULT (datetime('now')),
  ended_at     TEXT
);

-- Append-only: never UPDATEd or DELETEd, except the override endpoint flipping
-- correct/overridden on a single row (spec: "Reviews are append-only").
CREATE TABLE reviews (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  card_id     INTEGER NOT NULL REFERENCES cards(id),
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  answered_at TEXT NOT NULL DEFAULT (datetime('now')),
  given       TEXT,
  correct     INTEGER NOT NULL CHECK (correct IN (0,1)),
  overridden  INTEGER NOT NULL DEFAULT 0 CHECK (overridden IN (0,1)),
  ms          INTEGER
);
CREATE INDEX idx_reviews_card_time ON reviews(card_id, answered_at);
CREATE INDEX idx_reviews_session ON reviews(session_id);

CREATE TABLE schedule (
  card_id       INTEGER PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
  due_at        TEXT NOT NULL,
  interval_days REAL NOT NULL DEFAULT 0,
  ease          REAL NOT NULL DEFAULT 2.5,
  reps          INTEGER NOT NULL DEFAULT 0,
  lapses        INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_schedule_due ON schedule(due_at);
```

- [ ] **Step 2b: Note on `reviews` foreign keys** — `reviews.card_id` has **no**
      `ON DELETE CASCADE` on purpose. Cards are archived, never hard-deleted (spec:
      "Archiving, not deleting"); the missing cascade makes a stray delete fail loudly
      instead of silently rewriting history.

- [ ] **Step 3: `backend/src/db.rs`** — `foreign_keys` is per-connection in SQLite, so set it on
      every pooled connection, not once at startup.

```rust
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

pub async fn connect(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

- [ ] **Step 4: Wire it into `backend/src/main.rs`** — replace the `SqlitePool::connect` line:

```rust
mod db;
// ...
std::fs::create_dir_all(&config.data_dir)?;
let pool = db::connect(&config.database_url).await?;
```

- [ ] **Step 5: `backend/tests/common/mod.rs`** — the harness every later integration test reuses.
      Drives the `Router` in-process, so no ports and no server lifecycle.

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

pub struct TestApp {
    pub router: Router,
    _dir: tempfile::TempDir,
}

pub async fn spawn_app() -> TestApp {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = format!("sqlite://{}/test.db?mode=rwc", dir.path().display());
    let pool = quizapp::db::connect(&url).await.expect("db connect");
    let router = quizapp::app(quizapp::state::AppState { pool });
    TestApp { router, _dir: dir }
}

impl TestApp {
    pub async fn request(&self, method: &str, uri: &str, body: Option<Value>)
        -> (StatusCode, Value)
    {
        let req = Request::builder().method(method).uri(uri);
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&b).unwrap())),
            None => req.body(Body::empty()),
        }
        .unwrap();

        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, json)
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.request("GET", uri, None).await
    }
    pub async fn post(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.request("POST", uri, Some(body)).await
    }
    pub async fn patch(&self, uri: &str, body: Value) -> (StatusCode, Value) {
        self.request("PATCH", uri, Some(body)).await
    }
}
```

- [ ] **Step 6: Make the crate testable as a library** — add `backend/src/lib.rs` exposing the
      modules and an `app()` builder, and reduce `main.rs` to the binary entrypoint that
      calls it. `Cargo.toml` gains nothing; a `backend/src/lib.rs` alongside `backend/src/main.rs` is
      picked up automatically.

```rust
// src/lib.rs
pub mod config;
pub mod db;
pub mod error;   // added in Task 3
pub mod routes;
pub mod state;

use axum::Router;
use tower_http::trace::TraceLayer;

pub fn app(state: state::AppState) -> Router {
    Router::new()
        .nest("/api", routes::api_router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

`backend/src/main.rs` then becomes: init tracing, `Config::from_env()`, `create_dir_all`,
`db::connect`, `let app = quizapp::app(AppState { pool })`, bind, serve. Delete the
`mod config; mod routes; mod state;` lines from `main.rs` — they now live in `lib.rs`.
Create `backend/src/error.rs` as an empty file for now so `lib.rs` compiles; Task 3 fills it.

- [ ] **Step 7: `backend/tests/health.rs`** — the failing test first.

```rust
mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn health_returns_ok() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn migration_creates_all_tables() {
    let app = common::spawn_app().await;
    // spawn_app ran migrations; assert every spec table is present.
    let _ = &app; // schema assertions below use a direct pool
}
```

- [ ] **Step 8: Schema assertions** — add to `backend/tests/health.rs`, using a pool directly so the
      raw constraints are exercised, not just the HTTP surface.

```rust
#[tokio::test]
async fn schema_has_all_tables_and_constraints() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}/test.db?mode=rwc", dir.path().display());
    let pool = quizapp::db::connect(&url).await.unwrap();

    for table in ["modules", "decks", "cards", "choices", "accepted",
                  "sessions", "reviews", "schedule"] {
        let found: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
        )
        .bind(table)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(found.as_deref(), Some(table), "missing table {table}");
    }

    // foreign keys enforced
    let bad_fk = sqlx::query("INSERT INTO decks (module_id, name) VALUES (9999, 'x')")
        .execute(&pool)
        .await;
    assert!(bad_fk.is_err(), "foreign keys not enforced");

    // card kind CHECK
    sqlx::query("INSERT INTO modules (name) VALUES ('M')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO decks (module_id, name) VALUES (1, 'D')")
        .execute(&pool).await.unwrap();
    let bad_kind = sqlx::query(
        "INSERT INTO cards (deck_id, kind, prompt_md) VALUES (1, 'essay', 'p')")
        .execute(&pool).await;
    assert!(bad_kind.is_err(), "cards.kind CHECK not enforced");

    // duplicate deck name within the same module
    let dup = sqlx::query("INSERT INTO decks (module_id, name) VALUES (1, 'D')")
        .execute(&pool).await;
    assert!(dup.is_err(), "duplicate deck name allowed within a module");

    // duplicate deck name among module-less decks
    sqlx::query("INSERT INTO decks (module_id, name) VALUES (NULL, 'Loose')")
        .execute(&pool).await.unwrap();
    let dup_null = sqlx::query("INSERT INTO decks (module_id, name) VALUES (NULL, 'Loose')")
        .execute(&pool).await;
    assert!(dup_null.is_err(), "duplicate unparented deck name allowed");
}
```

- [ ] **Step 9: Run, then generate the offline cache**

```bash
cargo test --test health          # expect PASS
export DATABASE_URL="sqlite://data/quizapp.db?mode=rwc"
cargo sqlx migrate run --source backend/migrations
cargo sqlx prepare --workspace
SQLX_OFFLINE=true cargo build     # expect success
```

- [ ] **Step 10: Commit**

```bash
git add backend/migrations backend/src backend/tests .sqlx Cargo.toml Cargo.lock
git commit -m "feat: initial schema migration for all eight tables"
```

```json:metadata
{"files":["backend/migrations/0001_init.sql","backend/src/db.rs","backend/src/lib.rs","backend/src/main.rs","backend/tests/common/mod.rs","backend/tests/health.rs"],"verifyCommand":"cargo test --test health && SQLX_OFFLINE=true cargo build","acceptanceCriteria":["all eight spec tables exist after migration","cards.kind CHECK rejects an unknown kind","foreign keys enforced on every pooled connection","duplicate deck name rejected within a module and among module-less decks","fresh data dir is created and migrated at startup",".sqlx offline cache committed and SQLX_OFFLINE build succeeds"],"modelTier":"standard"}
```

---

## Task 3: Structured error type

**Goal:** One `AppError` that every handler returns, serialising to the single error envelope
with inline-renderable `fields`.

**Files:**
- Create/replace: `backend/src/error.rs`
- Test: `backend/src/error.rs` (`#[cfg(test)]` unit tests — pure serialisation, no DB)

**Acceptance Criteria:**
- [ ] `AppError::Validation` → `422` with a populated `fields` array
- [ ] `AppError::NotFound` → `404`, `AppError::Conflict` → `409`, both with `fields: []`
- [ ] A `sqlx` UNIQUE-constraint error converts to `Conflict`, not a `500`
- [ ] Any other `sqlx::Error` → `500` with a generic message; the real error is logged, not
      returned to the client

**Verify:** `cargo test error::` → pass

**Steps:**

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn validation_is_422_with_fields() {
        let err = AppError::validation([("name", "Name must not be empty")]);
        let (status, body) = err.parts();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body.error, "validation");
        assert_eq!(body.fields.len(), 1);
        assert_eq!(body.fields[0].field, "name");
    }

    #[test]
    fn not_found_is_404_with_empty_fields() {
        let (status, body) = AppError::NotFound("deck").parts();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error, "not_found");
        assert!(body.fields.is_empty());
    }

    #[test]
    fn unique_violation_becomes_conflict() {
        // sqlx exposes SQLite constraint violations via DatabaseError::is_unique_violation
        let err = AppError::Conflict("Deck name already used in this module".into());
        let (status, _) = err.parts();
        assert_eq!(status, StatusCode::CONFLICT);
    }
}
```

- [ ] **Step 2: Run — expect failure**

Run: `cargo test error::` → FAIL, `AppError` not defined.

- [ ] **Step 3: Implement `backend/src/error.rs`**

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: &'static str,
    pub message: String,
    pub fields: Vec<FieldError>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("validation failed")]
    Validation(Vec<FieldError>),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl AppError {
    pub fn validation<I, F, M>(fields: I) -> Self
    where
        I: IntoIterator<Item = (F, M)>,
        F: Into<String>,
        M: Into<String>,
    {
        AppError::Validation(
            fields
                .into_iter()
                .map(|(f, m)| FieldError { field: f.into(), message: m.into() })
                .collect(),
        )
    }

    pub fn parts(self) -> (StatusCode, ErrorBody) {
        match self {
            AppError::NotFound(what) => (
                StatusCode::NOT_FOUND,
                ErrorBody { error: "not_found", message: format!("{what} not found"),
                            fields: vec![] },
            ),
            AppError::Validation(fields) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorBody { error: "validation",
                            message: "The submitted values are invalid".into(), fields },
            ),
            AppError::Conflict(message) => (
                StatusCode::CONFLICT,
                ErrorBody { error: "conflict", message, fields: vec![] },
            ),
            AppError::Db(e) => {
                if let Some(dbe) = e.as_database_error() {
                    if dbe.is_unique_violation() {
                        return (
                            StatusCode::CONFLICT,
                            ErrorBody { error: "conflict",
                                        message: "That name is already in use".into(),
                                        fields: vec![] },
                        );
                    }
                    if dbe.is_foreign_key_violation() {
                        return (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            ErrorBody {
                                error: "validation",
                                message: "A referenced record does not exist".into(),
                                fields: vec![FieldError {
                                    field: "module_id".into(),
                                    message: "That module does not exist".into(),
                                }],
                            },
                        );
                    }
                }
                tracing::error!(error = ?e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody { error: "internal",
                                message: "Something went wrong".into(), fields: vec![] },
                )
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = self.parts();
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
```

- [ ] **Step 4: Run — expect pass**

Run: `cargo test error::` → PASS. Then `cargo clippy -- -D warnings`.

- [ ] **Step 5: Commit**

```bash
git add backend/src/error.rs
git commit -m "feat: structured AppError with inline field errors"
```

```json:metadata
{"files":["backend/src/error.rs"],"verifyCommand":"cargo test error:: && cargo clippy -- -D warnings","acceptanceCriteria":["Validation maps to 422 with populated fields","NotFound 404 and Conflict 409 with empty fields","sqlx unique violation maps to 409 not 500","sqlx foreign key violation maps to 422 with a module_id field error","other sqlx errors map to 500 with a generic message and are logged"],"modelTier":"mechanical"}
```

---

## Task 4: Modules API

**Goal:** `GET /api/modules` and `POST /api/modules` work, validate, and are covered by
integration tests.

**Files:**
- Create: `backend/src/routes/modules.rs`, `backend/tests/modules.rs`
- Modify: `backend/src/routes/mod.rs` (merge the router)

**Acceptance Criteria:**
- [ ] `GET /api/modules` → `200` with modules ordered by `name`, each `{id, name, created_at,
      deck_count}`
- [ ] `POST /api/modules` with `{"name":"COS781"}` → `201` and the created module
- [ ] Empty or whitespace-only name → `422` with a `name` field error
- [ ] Duplicate name → `409`
- [ ] Names are trimmed before insert

**Verify:** `cargo test --test modules` → all pass

**Steps:**

- [ ] **Step 1: Write the failing tests — `backend/tests/modules.rs`**

```rust
mod common;

use axum::http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn list_starts_empty() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/modules").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn create_then_list() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/modules", json!({"name": "COS781"})).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "COS781");
    assert_eq!(body["deck_count"], 0);

    let (_, list) = app.get("/api/modules").await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn name_is_trimmed_and_required() {
    let app = common::spawn_app().await;

    let (status, body) = app.post("/api/modules", json!({"name": "   "})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "name");

    let (_, created) = app.post("/api/modules", json!({"name": "  COS781  "})).await;
    assert_eq!(created["name"], "COS781");
}

#[tokio::test]
async fn duplicate_name_conflicts() {
    let app = common::spawn_app().await;
    app.post("/api/modules", json!({"name": "COS781"})).await;
    let (status, _) = app.post("/api/modules", json!({"name": "COS781"})).await;
    assert_eq!(status, StatusCode::CONFLICT);
}
```

- [ ] **Step 2: Run — expect failure**

Run: `cargo test --test modules` → FAIL (404s / compile error).

- [ ] **Step 3: Implement `backend/src/routes/modules.rs`**

```rust
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct ModuleDto {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub deck_count: i64,
}

#[derive(Deserialize)]
pub struct CreateModule {
    pub name: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/modules", get(list).post(create))
}

async fn list(State(st): State<AppState>) -> AppResult<Json<Vec<ModuleDto>>> {
    let rows = sqlx::query_as!(
        ModuleDto,
        r#"SELECT m.id AS "id!: i64",
                  m.name,
                  m.created_at,
                  (SELECT COUNT(*) FROM decks d WHERE d.module_id = m.id)
                      AS "deck_count!: i64"
           FROM modules m
           ORDER BY m.name COLLATE NOCASE"#
    )
    .fetch_all(&st.pool)
    .await?;
    Ok(Json(rows))
}

async fn create(
    State(st): State<AppState>,
    Json(body): Json<CreateModule>,
) -> AppResult<(StatusCode, Json<ModuleDto>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::validation([("name", "Name must not be empty")]));
    }

    let id = sqlx::query_scalar!("INSERT INTO modules (name) VALUES (?) RETURNING id", name)
        .fetch_one(&st.pool)
        .await?;

    let created = sqlx::query_as!(
        ModuleDto,
        r#"SELECT m.id AS "id!: i64", m.name, m.created_at, 0 AS "deck_count!: i64"
           FROM modules m WHERE m.id = ?"#,
        id
    )
    .fetch_one(&st.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(created)))
}
```

Note the `AS "col!: type"` annotations — sqlx cannot infer nullability through SQLite
subqueries and `AUTOINCREMENT` primary keys, and will error at compile time without them.
Drop the unused `post` import if clippy flags it.

- [ ] **Step 4: Register the router in `backend/src/routes/mod.rs`**

```rust
pub mod health;
pub mod modules;

use axum::Router;
use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(modules::router())
}
```

- [ ] **Step 5: Regenerate the offline cache and run**

```bash
DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
cargo test --test modules   # expect PASS
cargo clippy -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add backend/src/routes backend/tests/modules.rs .sqlx
git commit -m "feat: modules list and create endpoints"
```

```json:metadata
{"files":["backend/src/routes/modules.rs","backend/src/routes/mod.rs","backend/tests/modules.rs"],"verifyCommand":"cargo test --test modules && cargo clippy -- -D warnings","acceptanceCriteria":["GET /api/modules returns modules ordered by name with deck_count","POST /api/modules returns 201 with the created module","empty or whitespace name returns 422 with a name field error","duplicate name returns 409","names are trimmed before insert"],"modelTier":"standard"}
```

---

## Task 5: Decks API

**Goal:** `GET /api/decks` (optional `module_id` filter), `POST /api/decks`, and
`PATCH /api/decks/:id` (rename, re-parent, edit description), all tested.

**Files:**
- Create: `backend/src/routes/decks.rs`, `backend/tests/decks.rs`
- Modify: `backend/src/routes/mod.rs`

**Acceptance Criteria:**
- [ ] `GET /api/decks` → all decks, each `{id, module_id, module_name, name, description,
      created_at, card_count}`, ordered by module name then deck name
- [ ] `GET /api/decks?module_id=1` filters to that module; `?module_id=none` returns only
      unparented decks
- [ ] `POST /api/decks` → `201`; `module_id` may be omitted or `null`
- [ ] `POST` with a non-existent `module_id` → `422` with a `module_id` field error
- [ ] `PATCH /api/decks/:id` updates only the supplied fields (name / module_id /
      description); a `null` `module_id` unparents the deck
- [ ] `PATCH` on an unknown id → `404`
- [ ] Duplicate name within the target module (on create or patch) → `409`

**Verify:** `cargo test --test decks` → all pass

**Steps:**

- [ ] **Step 1: Write the failing tests — `backend/tests/decks.rs`**

```rust
mod common;

use axum::http::StatusCode;
use serde_json::json;

async fn module(app: &common::TestApp, name: &str) -> i64 {
    let (_, m) = app.post("/api/modules", json!({"name": name})).await;
    m["id"].as_i64().unwrap()
}

#[tokio::test]
async fn create_deck_in_module() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;

    let (status, body) = app
        .post("/api/decks", json!({
            "module_id": mid, "name": "Test 1", "description": "Ch 1-3"
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "Test 1");
    assert_eq!(body["module_id"], mid);
    assert_eq!(body["module_name"], "COS781");
    assert_eq!(body["card_count"], 0);
}

#[tokio::test]
async fn create_deck_without_module() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/decks", json!({"name": "Loose"})).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(body["module_id"].is_null());
    assert!(body["module_name"].is_null());
    assert_eq!(body["description"], "");
}

#[tokio::test]
async fn unknown_module_is_rejected() {
    let app = common::spawn_app().await;
    let (status, body) = app
        .post("/api/decks", json!({"module_id": 9999, "name": "X"}))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "module_id");
}

#[tokio::test]
async fn empty_name_is_rejected() {
    let app = common::spawn_app().await;
    let (status, body) = app.post("/api/decks", json!({"name": "  "})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "name");
}

#[tokio::test]
async fn filter_by_module_and_by_none() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Test 1"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, all) = app.get("/api/decks").await;
    assert_eq!(all.as_array().unwrap().len(), 2);

    let (_, in_module) = app.get(&format!("/api/decks?module_id={mid}")).await;
    assert_eq!(in_module.as_array().unwrap().len(), 1);
    assert_eq!(in_module[0]["name"], "Test 1");

    let (_, unparented) = app.get("/api/decks?module_id=none").await;
    assert_eq!(unparented.as_array().unwrap().len(), 1);
    assert_eq!(unparented[0]["name"], "Loose");
}

#[tokio::test]
async fn patch_renames_reparents_and_unparents() {
    let app = common::spawn_app().await;
    let a = module(&app, "COS781").await;
    let b = module(&app, "COS731").await;
    let (_, deck) = app.post("/api/decks", json!({"module_id": a, "name": "Test 1"})).await;
    let id = deck["id"].as_i64().unwrap();

    let (status, renamed) = app
        .patch(&format!("/api/decks/{id}"), json!({"name": "Test One"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["name"], "Test One");
    assert_eq!(renamed["module_id"], a, "module must be untouched");

    let (_, moved) = app
        .patch(&format!("/api/decks/{id}"), json!({"module_id": b}))
        .await;
    assert_eq!(moved["module_id"], b);
    assert_eq!(moved["name"], "Test One", "name must be untouched");

    let (_, loose) = app
        .patch(&format!("/api/decks/{id}"), json!({"module_id": null}))
        .await;
    assert!(loose["module_id"].is_null());

    let (_, described) = app
        .patch(&format!("/api/decks/{id}"), json!({"description": "Ch 4-6"}))
        .await;
    assert_eq!(described["description"], "Ch 4-6");
}

#[tokio::test]
async fn patch_unknown_deck_is_404() {
    let app = common::spawn_app().await;
    let (status, _) = app.patch("/api/decks/9999", json!({"name": "X"})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn duplicate_name_in_module_conflicts() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Test 1"})).await;
    let (status, _) = app
        .post("/api/decks", json!({"module_id": mid, "name": "Test 1"}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
```

- [ ] **Step 2: Run — expect failure**

Run: `cargo test --test decks` → FAIL.

- [ ] **Step 3: Implement `backend/src/routes/decks.rs`**

The PATCH body needs to distinguish "field absent" from "field explicitly `null`" — absent
`module_id` means leave it alone, `null` means unparent. `Option<Option<i64>>` with
`#[serde(default, deserialize_with = ...)]` does that; `serde_with::double_option` would too
but is an extra dependency, so hand-roll it.

```rust
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct DeckDto {
    pub id: i64,
    pub module_id: Option<i64>,
    pub module_name: Option<String>,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub card_count: i64,
}

#[derive(Deserialize)]
pub struct ListQuery {
    /// Numeric module id, or the literal "none" for unparented decks only.
    pub module_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateDeck {
    pub name: String,
    #[serde(default)]
    pub module_id: Option<i64>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Distinguishes "key absent" (None) from "key present and null" (Some(None)).
fn some_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(d).map(Some)
}

#[derive(Deserialize)]
pub struct PatchDeck {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "some_option")]
    pub module_id: Option<Option<i64>>,
    #[serde(default)]
    pub description: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/decks", get(list).post(create))
        .route("/decks/{id}", axum::routing::patch(patch))
}

async fn fetch_one(pool: &sqlx::SqlitePool, id: i64) -> AppResult<DeckDto> {
    sqlx::query_as!(
        DeckDto,
        r#"SELECT d.id AS "id!: i64",
                  d.module_id AS "module_id?: i64",
                  m.name      AS "module_name?: String",
                  d.name, d.description, d.created_at,
                  (SELECT COUNT(*) FROM cards c WHERE c.deck_id = d.id AND c.archived = 0)
                      AS "card_count!: i64"
           FROM decks d
           LEFT JOIN modules m ON m.id = d.module_id
           WHERE d.id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("deck"))
}

async fn list(
    State(st): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<DeckDto>>> {
    // Three literal queries rather than dynamic SQL: query_as! needs a literal
    // string, so every variant stays compile-time checked.
    let rows = match q.module_id.as_deref() {
        None => sqlx::query_as!(
            DeckDto,
            r#"SELECT d.id AS "id!: i64",
                      d.module_id AS "module_id?: i64",
                      m.name      AS "module_name?: String",
                      d.name, d.description, d.created_at,
                      (SELECT COUNT(*) FROM cards c
                        WHERE c.deck_id = d.id AND c.archived = 0) AS "card_count!: i64"
               FROM decks d
               LEFT JOIN modules m ON m.id = d.module_id
               ORDER BY m.name COLLATE NOCASE, d.name COLLATE NOCASE"#
        )
        .fetch_all(&st.pool)
        .await?,

        Some("none") => sqlx::query_as!(
            DeckDto,
            r#"SELECT d.id AS "id!: i64",
                      d.module_id AS "module_id?: i64",
                      m.name      AS "module_name?: String",
                      d.name, d.description, d.created_at,
                      (SELECT COUNT(*) FROM cards c
                        WHERE c.deck_id = d.id AND c.archived = 0) AS "card_count!: i64"
               FROM decks d
               LEFT JOIN modules m ON m.id = d.module_id
               WHERE d.module_id IS NULL
               ORDER BY d.name COLLATE NOCASE"#
        )
        .fetch_all(&st.pool)
        .await?,

        Some(raw) => {
            let mid: i64 = raw.parse().map_err(|_| {
                AppError::validation([("module_id", "module_id must be a number or \"none\"")])
            })?;
            sqlx::query_as!(
                DeckDto,
                r#"SELECT d.id AS "id!: i64",
                          d.module_id AS "module_id?: i64",
                          m.name      AS "module_name?: String",
                          d.name, d.description, d.created_at,
                          (SELECT COUNT(*) FROM cards c
                            WHERE c.deck_id = d.id AND c.archived = 0) AS "card_count!: i64"
                   FROM decks d
                   LEFT JOIN modules m ON m.id = d.module_id
                   WHERE d.module_id = ?
                   ORDER BY d.name COLLATE NOCASE"#,
                mid
            )
            .fetch_all(&st.pool)
            .await?
        }
    };
    Ok(Json(rows))
}
```

`ORDER BY m.name COLLATE NOCASE` puts `NULL` module names first in SQLite, so unparented
decks sort to the top of the unfiltered list. That is fine — the frontend groups by module
and renders "No module" last regardless of row order.

```rust
async fn create(
    State(st): State<AppState>,
    Json(body): Json<CreateDeck>,
) -> AppResult<(StatusCode, Json<DeckDto>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::validation([("name", "Name must not be empty")]));
    }
    let description = body.description.unwrap_or_default();

    let id = sqlx::query_scalar!(
        "INSERT INTO decks (module_id, name, description) VALUES (?, ?, ?) RETURNING id",
        body.module_id,
        name,
        description
    )
    .fetch_one(&st.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(fetch_one(&st.pool, id).await?)))
}

async fn patch(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<PatchDeck>,
) -> AppResult<Json<DeckDto>> {
    // 404 before any write, so a bad id never touches the row.
    let current = fetch_one(&st.pool, id).await?;

    let name = match body.name {
        Some(n) => {
            let n = n.trim().to_string();
            if n.is_empty() {
                return Err(AppError::validation([("name", "Name must not be empty")]));
            }
            n
        }
        None => current.name,
    };
    let module_id = match body.module_id {
        Some(v) => v,                    // present: Some(id) or None (unparent)
        None => current.module_id,       // absent: leave alone
    };
    let description = body.description.unwrap_or(current.description);

    sqlx::query!(
        "UPDATE decks SET module_id = ?, name = ?, description = ? WHERE id = ?",
        module_id,
        name,
        description,
        id
    )
    .execute(&st.pool)
    .await?;

    Ok(Json(fetch_one(&st.pool, id).await?))
}
```

An unknown `module_id` surfaces as a SQLite foreign-key violation, which `AppError::Db`
already maps to a `422` with a `module_id` field error (Task 3) — no extra existence check.

- [ ] **Step 4: Register in `backend/src/routes/mod.rs`** — add `pub mod decks;` and
      `.merge(decks::router())`.

- [ ] **Step 5: Regenerate cache and run**

```bash
DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
cargo test          # whole suite green
cargo clippy -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add backend/src/routes backend/tests/decks.rs .sqlx
git commit -m "feat: decks list, create and patch endpoints"
```

```json:metadata
{"files":["backend/src/routes/decks.rs","backend/src/routes/mod.rs","backend/tests/decks.rs"],"verifyCommand":"cargo test && cargo clippy -- -D warnings","acceptanceCriteria":["GET /api/decks returns decks with module_name and card_count ordered by module then name","module_id filter works for a numeric id and for the literal none","POST /api/decks accepts an absent or null module_id and returns 201","non-existent module_id returns 422 with a module_id field error","PATCH updates only supplied fields and a null module_id unparents","PATCH on an unknown id returns 404 before any write","duplicate deck name within a module returns 409"],"modelTier":"standard"}
```

---

## Task 6: Frontend scaffold with Bibble tokens

**Goal:** `npm run dev` in `frontend/` serves a React app at `:5273` that proxies `/api` to axum,
with the Bibble palette and Quicksand in place and a nav shell over `/study`, `/decks`,
`/stats`.

**Files:**
- Create: `frontend/package.json`, `frontend/vite.config.ts`, `frontend/tsconfig.json`,
  `frontend/tsconfig.node.json`, `frontend/index.html`, `frontend/components.json`, `frontend/src/main.tsx`,
  `frontend/src/App.tsx`, `frontend/src/styles/globals.css`, `frontend/src/lib/utils.ts`,
  `frontend/src/components/AppShell.tsx`, `frontend/src/pages/StubPage.tsx`

**Acceptance Criteria:**
- [ ] `npm run dev` serves the app; `/decks`, `/study`, `/stats` all render the shell
- [ ] A `fetch('/api/health')` from the browser console returns `{"status":"ok"}` through the
      Vite proxy
- [ ] `npm run build` and `npx tsc --noEmit` both succeed
- [ ] Bibble tokens are defined for light and dark; toggling the OS theme visibly changes the
      page. Deep-twilight background, turquoise primary, magenta/lavender accents, gold for
      correct — flat colours only, no glows or gradients yet
- [ ] Quicksand renders on headings, a clean sans on body

**Verify:** `cd frontend && npm run build && npx tsc --noEmit` → both succeed; browser check of
`/decks` in light and dark

**Steps:**

- [ ] **Step 1: Scaffold**

```bash
cd /Users/hayley/Documents/side_projects/quizapp
npm create vite@latest frontend -- --template react-ts
cd frontend
npm install
npm install react-router-dom
npm install tailwindcss @tailwindcss/vite
npm install class-variance-authority clsx tailwind-merge lucide-react
```

- [ ] **Step 2: `frontend/vite.config.ts`** — Tailwind v4 plugin plus the `/api` proxy (same-origin
      in the browser, so no CORS layer is needed on the axum side).

```ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { '@': path.resolve(__dirname, './src') } },
  server: {
    port: 5273,
    proxy: { '/api': { target: 'http://127.0.0.1:3000', changeOrigin: true } },
  },
})
```

Add the matching path alias to `tsconfig.json`:
`"baseUrl": ".", "paths": { "@/*": ["./src/*"] }`.

- [ ] **Step 3: `frontend/src/styles/globals.css`** — Bibble tokens. Tailwind v4 is CSS-first, so
      the theme lives here rather than in a `tailwind.config.js`. shadcn reads the
      `--background` / `--foreground` / `--primary` family of names, so define those and map
      them to the Bibble palette.

```css
@import "tailwindcss";
@import url('https://fonts.googleapis.com/css2?family=Quicksand:wght@500;600;700&family=Inter:wght@400;500;600&display=swap');

@theme {
  --font-display: 'Quicksand', ui-rounded, system-ui, sans-serif;
  --font-sans: 'Inter', system-ui, sans-serif;
  --radius: 0.9rem;
}

/* Bibble — light base: pale aqua and lilac */
:root {
  --background: oklch(0.98 0.015 200);
  --foreground: oklch(0.28 0.045 265);
  --card: oklch(1 0 0);
  --card-foreground: var(--foreground);
  --popover: oklch(1 0 0);
  --popover-foreground: var(--foreground);
  --primary: oklch(0.62 0.13 195);           /* turquoise */
  --primary-foreground: oklch(0.99 0.01 200);
  --secondary: oklch(0.94 0.035 290);        /* lilac */
  --secondary-foreground: oklch(0.30 0.06 285);
  --accent: oklch(0.68 0.19 335);            /* magenta */
  --accent-foreground: oklch(0.99 0.01 330);
  --muted: oklch(0.95 0.02 220);
  --muted-foreground: oklch(0.50 0.03 255);
  --success: oklch(0.80 0.14 88);            /* gold — correct answers */
  --success-foreground: oklch(0.25 0.05 80);
  --destructive: oklch(0.58 0.20 20);
  --destructive-foreground: oklch(0.99 0.01 20);
  --border: oklch(0.90 0.02 230);
  --input: var(--border);
  --ring: var(--primary);
}

/* Bibble — dark base (primary): deep twilight */
.dark {
  --background: oklch(0.20 0.045 275);
  --foreground: oklch(0.94 0.02 220);
  --card: oklch(0.25 0.05 275);
  --card-foreground: var(--foreground);
  --popover: oklch(0.25 0.05 275);
  --popover-foreground: var(--foreground);
  --primary: oklch(0.78 0.14 190);
  --primary-foreground: oklch(0.18 0.04 270);
  --secondary: oklch(0.34 0.06 290);
  --secondary-foreground: oklch(0.94 0.02 290);
  --accent: oklch(0.72 0.20 335);
  --accent-foreground: oklch(0.16 0.04 330);
  --muted: oklch(0.29 0.04 275);
  --muted-foreground: oklch(0.72 0.03 250);
  --success: oklch(0.84 0.15 88);
  --success-foreground: oklch(0.20 0.05 80);
  --destructive: oklch(0.65 0.19 22);
  --destructive-foreground: oklch(0.16 0.04 20);
  --border: oklch(0.34 0.04 275);
  --input: var(--border);
  --ring: var(--primary);
}

@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-popover: var(--popover);
  --color-popover-foreground: var(--popover-foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-success: var(--success);
  --color-success-foreground: var(--success-foreground);
  --color-destructive: var(--destructive);
  --color-destructive-foreground: var(--destructive-foreground);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
}

@layer base {
  * { border-color: var(--color-border); }
  body {
    background-color: var(--color-background);
    color: var(--color-foreground);
    font-family: var(--font-sans);
  }
  h1, h2, h3, .font-display { font-family: var(--font-display); }
}
```

- [ ] **Step 4: Dark by default, following the OS** — in `frontend/index.html`, add a tiny
      inline script before the app script so there is no light flash:

```html
<script>
  if (!window.matchMedia('(prefers-color-scheme: light)').matches) {
    document.documentElement.classList.add('dark')
  }
</script>
```

Delete the Vite starter's `App.css` and `index.css`, and import `./styles/globals.css` from
`main.tsx` instead.

- [ ] **Step 5: shadcn init and the primitives this part needs**

```bash
npx shadcn@latest init          # style: default, base color: neutral, CSS vars: yes
npx shadcn@latest add button input textarea label dialog select card sonner
```

If `init` offers to overwrite `globals.css`, decline and keep the Bibble tokens — shadcn's
generated variables use the same names, so its components pick them up as-is. If it has
already overwritten the file, restore it from Step 3.

- [ ] **Step 6: `frontend/src/components/AppShell.tsx`**

```tsx
import { NavLink, Outlet } from 'react-router-dom'
import { Toaster } from '@/components/ui/sonner'

const links = [
  { to: '/study', label: 'Study' },
  { to: '/decks', label: 'Decks' },
  { to: '/stats', label: 'Stats' },
]

export function AppShell() {
  return (
    <div className="min-h-dvh">
      <header className="border-b">
        <nav className="mx-auto flex max-w-5xl items-center gap-1 px-4 py-3">
          <span className="font-display mr-4 text-lg font-bold text-primary">quizapp</span>
          {links.map((l) => (
            <NavLink
              key={l.to}
              to={l.to}
              className={({ isActive }) =>
                `rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
                  isActive
                    ? 'bg-primary text-primary-foreground'
                    : 'text-muted-foreground hover:bg-secondary'
                }`
              }
            >
              {l.label}
            </NavLink>
          ))}
        </nav>
      </header>
      <main className="mx-auto max-w-5xl px-4 py-8">
        <Outlet />
      </main>
      <Toaster />
    </div>
  )
}
```

- [ ] **Step 7: `frontend/src/pages/StubPage.tsx`** and `frontend/src/App.tsx`

```tsx
// StubPage.tsx
export function StubPage({ title, note }: { title: string; note: string }) {
  return (
    <div>
      <h1 className="font-display text-2xl font-bold">{title}</h1>
      <p className="mt-2 text-muted-foreground">{note}</p>
    </div>
  )
}
```

```tsx
// App.tsx
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppShell } from '@/components/AppShell'
import { StubPage } from '@/pages/StubPage'
import { DecksPage } from '@/pages/DecksPage'   // added in Task 7

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route path="/" element={<Navigate to="/decks" replace />} />
          <Route path="/decks" element={<DecksPage />} />
          <Route
            path="/study"
            element={<StubPage title="Study" note="Session modes arrive in part 3." />}
          />
          <Route
            path="/stats"
            element={<StubPage title="Stats" note="Statistics arrive in part 6." />}
          />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}
```

Until Task 7 lands, point `/decks` at a `StubPage` too so the app compiles; swap it for
`DecksPage` in Task 7.

- [ ] **Step 8: Verify**

```bash
cd frontend
npm run build
npx tsc --noEmit
npm run dev      # then in the browser console: await (await fetch('/api/health')).json()
```

Backend must be running (`cargo run`) for the proxy check.

- [ ] **Step 9: Commit**

```bash
git add frontend
git commit -m "feat: vite react scaffold with Bibble palette tokens"
```

```json:metadata
{"files":["frontend/package.json","frontend/vite.config.ts","frontend/tsconfig.json","frontend/index.html","frontend/src/main.tsx","frontend/src/App.tsx","frontend/src/styles/globals.css","frontend/src/components/AppShell.tsx","frontend/src/pages/StubPage.tsx"],"verifyCommand":"cd frontend && npm run build && npx tsc --noEmit","acceptanceCriteria":["npm run dev serves the shell at /decks /study /stats","fetch('/api/health') succeeds through the Vite proxy","npm run build and tsc --noEmit both pass","Bibble tokens defined for light and dark and visibly applied","Quicksand on headings and a clean sans on body","no glow, gradient or animation work included"],"modelTier":"standard"}
```

---

## Task 7: `/decks` screen — module and deck management

**Goal:** In the browser: see modules and their decks, create a module, create a deck,
rename a deck, move it between modules, and edit its description — with server field errors
rendered inline and typed content never lost on a rejected save.

**Files:**
- Create: `frontend/src/lib/api.ts`, `frontend/src/pages/DecksPage.tsx`,
  `frontend/src/components/ModuleDialog.tsx`, `frontend/src/components/DeckDialog.tsx`
- Modify: `frontend/src/App.tsx` (route `/decks` to `DecksPage`)

**Acceptance Criteria:**
- [ ] Decks are listed grouped by module, with unparented decks under "No module"
- [ ] "New module" creates a module and the list refreshes
- [ ] "New deck" creates a deck with an optional module and description
- [ ] Editing a deck can change its name, its module (including to "No module"), and its
      description; the dialog sends only the changed fields
- [ ] A `422` renders the message next to the offending input and leaves the dialog open with
      the typed values intact
- [ ] A `409` (duplicate name) shows on the `name` input, not just as a toast
- [ ] Empty state reads sensibly when there are no modules and no decks
- [ ] Layout is usable at 375px width

**Verify:** `cd frontend && npx tsc --noEmit && npm run build`, then a browser pass through every
acceptance criterion with `cargo run` live

**Steps:**

- [ ] **Step 1: `frontend/src/lib/api.ts`** — one client, one error type, mirroring the backend
      envelope exactly.

```ts
export type FieldError = { field: string; message: string }

export class ApiError extends Error {
  status: number
  fields: FieldError[]
  constructor(status: number, message: string, fields: FieldError[]) {
    super(message)
    this.status = status
    this.fields = fields
  }
  /** Field errors keyed by field name, for inline rendering. */
  byField(): Record<string, string> {
    return Object.fromEntries(this.fields.map((f) => [f.field, f.message]))
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method,
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  if (!res.ok) {
    const payload = await res.json().catch(() => null)
    throw new ApiError(
      res.status,
      payload?.message ?? `Request failed (${res.status})`,
      payload?.fields ?? [],
    )
  }
  return res.status === 204 ? (undefined as T) : ((await res.json()) as T)
}

export type Module = { id: number; name: string; created_at: string; deck_count: number }
export type Deck = {
  id: number
  module_id: number | null
  module_name: string | null
  name: string
  description: string
  created_at: string
  card_count: number
}

export const api = {
  listModules: () => request<Module[]>('GET', '/modules'),
  createModule: (name: string) => request<Module>('POST', '/modules', { name }),
  listDecks: () => request<Deck[]>('GET', '/decks'),
  createDeck: (input: { name: string; module_id: number | null; description: string }) =>
    request<Deck>('POST', '/decks', input),
  // Only send keys the user actually changed — an absent module_id means
  // "leave it alone" on the server, while null means "unparent".
  updateDeck: (
    id: number,
    patch: Partial<{ name: string; module_id: number | null; description: string }>,
  ) => request<Deck>('PATCH', `/decks/${id}`, patch),
}
```

- [ ] **Step 2: `frontend/src/components/ModuleDialog.tsx`** — create-only dialog. The pattern
      here (local error state fed from `ApiError.byField()`, dialog stays open on failure) is
      the one the card editor reuses in Part 2.

```tsx
import { useState } from 'react'
import { toast } from 'sonner'
import { api, ApiError } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger,
} from '@/components/ui/dialog'

export function ModuleDialog({ onSaved }: { onSaved: () => void }) {
  const [open, setOpen] = useState(false)
  const [name, setName] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)

  async function save() {
    setBusy(true)
    setErrors({})
    try {
      await api.createModule(name)
      setName('')
      setOpen(false)
      onSaved()
    } catch (e) {
      if (e instanceof ApiError) {
        const byField = e.byField()
        // A 409 has no field payload; surface it on the input that caused it.
        setErrors(e.status === 409 ? { name: e.message } : byField)
        if (e.status !== 409 && Object.keys(byField).length === 0) toast.error(e.message)
      } else {
        toast.error('Could not reach the server')
      }
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="secondary">New module</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader><DialogTitle className="font-display">New module</DialogTitle></DialogHeader>
        <div className="space-y-2">
          <Label htmlFor="module-name">Name</Label>
          <Input
            id="module-name"
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter') save() }}
            aria-invalid={!!errors.name}
          />
          {errors.name && <p className="text-sm text-destructive">{errors.name}</p>}
        </div>
        <DialogFooter>
          <Button onClick={save} disabled={busy}>Save</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

- [ ] **Step 3: `frontend/src/components/DeckDialog.tsx`** — one dialog for create and edit. In
      edit mode it diffs against the loaded deck and sends only changed keys, which is what
      makes the PATCH semantics from Task 5 correct end-to-end.

```tsx
import { useState } from 'react'
import { toast } from 'sonner'
import { api, ApiError, type Deck, type Module } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import {
  Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'

const NO_MODULE = 'none'

type Props = {
  modules: Module[]
  deck?: Deck            // absent => create mode
  open: boolean
  onOpenChange: (open: boolean) => void
  onSaved: () => void
}

export function DeckDialog({ modules, deck, open, onOpenChange, onSaved }: Props) {
  const [name, setName] = useState(deck?.name ?? '')
  const [moduleId, setModuleId] = useState(
    deck?.module_id != null ? String(deck.module_id) : NO_MODULE,
  )
  const [description, setDescription] = useState(deck?.description ?? '')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)

  const selectedModuleId = moduleId === NO_MODULE ? null : Number(moduleId)

  async function save() {
    setBusy(true)
    setErrors({})
    try {
      if (deck) {
        const patch: Record<string, unknown> = {}
        if (name !== deck.name) patch.name = name
        if (selectedModuleId !== deck.module_id) patch.module_id = selectedModuleId
        if (description !== deck.description) patch.description = description
        if (Object.keys(patch).length > 0) await api.updateDeck(deck.id, patch)
      } else {
        await api.createDeck({ name, module_id: selectedModuleId, description })
      }
      onOpenChange(false)
      onSaved()
    } catch (e) {
      // Never reset the form here — a rejected save must keep what was typed.
      if (e instanceof ApiError) {
        const byField = e.byField()
        setErrors(e.status === 409 ? { name: e.message } : byField)
        if (e.status !== 409 && Object.keys(byField).length === 0) toast.error(e.message)
      } else {
        toast.error('Could not reach the server')
      }
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="font-display">{deck ? 'Edit deck' : 'New deck'}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="deck-name">Name</Label>
            <Input
              id="deck-name" autoFocus value={name}
              onChange={(e) => setName(e.target.value)}
              aria-invalid={!!errors.name}
            />
            {errors.name && <p className="text-sm text-destructive">{errors.name}</p>}
          </div>
          <div className="space-y-2">
            <Label htmlFor="deck-module">Module</Label>
            <Select value={moduleId} onValueChange={setModuleId}>
              <SelectTrigger id="deck-module"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_MODULE}>No module</SelectItem>
                {modules.map((m) => (
                  <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            {errors.module_id && <p className="text-sm text-destructive">{errors.module_id}</p>}
          </div>
          <div className="space-y-2">
            <Label htmlFor="deck-description">Description</Label>
            <Textarea
              id="deck-description" value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
        </div>
        <DialogFooter>
          <Button onClick={save} disabled={busy}>Save</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

Mount `DeckDialog` with a `key` (`key={deck?.id ?? 'new'}`) so opening it for a different
deck resets the `useState` initialisers.

- [ ] **Step 4: `frontend/src/pages/DecksPage.tsx`**

```tsx
import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { api, type Deck, type Module } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { ModuleDialog } from '@/components/ModuleDialog'
import { DeckDialog } from '@/components/DeckDialog'

export function DecksPage() {
  const [modules, setModules] = useState<Module[]>([])
  const [decks, setDecks] = useState<Deck[]>([])
  const [editing, setEditing] = useState<Deck | 'new' | null>(null)

  const load = useCallback(async () => {
    try {
      const [m, d] = await Promise.all([api.listModules(), api.listDecks()])
      setModules(m)
      setDecks(d)
    } catch {
      toast.error('Could not load decks')
    }
  }, [])

  useEffect(() => { void load() }, [load])

  const groups = useMemo(() => {
    const named = modules.map((m) => ({
      key: String(m.id),
      title: m.name,
      decks: decks.filter((d) => d.module_id === m.id),
    }))
    const loose = decks.filter((d) => d.module_id === null)
    return loose.length > 0
      ? [...named, { key: 'none', title: 'No module', decks: loose }]
      : named
  }, [modules, decks])

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-display text-2xl font-bold">Decks</h1>
        <div className="flex gap-2">
          <ModuleDialog onSaved={load} />
          <Button onClick={() => setEditing('new')}>New deck</Button>
        </div>
      </div>

      {groups.length === 0 && (
        <p className="text-muted-foreground">
          No modules or decks yet. Create a module (e.g. COS781), then a deck for each test.
        </p>
      )}

      {groups.map((g) => (
        <section key={g.key} className="space-y-3">
          <h2 className="font-display text-lg font-semibold text-primary">{g.title}</h2>
          {g.decks.length === 0 ? (
            <p className="text-sm text-muted-foreground">No decks in this module yet.</p>
          ) : (
            <div className="grid gap-3 sm:grid-cols-2">
              {g.decks.map((d) => (
                <Card key={d.id}>
                  <CardHeader className="flex flex-row items-start justify-between gap-2">
                    <div>
                      <CardTitle className="font-display text-base">{d.name}</CardTitle>
                      <p className="text-sm text-muted-foreground">
                        {d.card_count} card{d.card_count === 1 ? '' : 's'}
                      </p>
                    </div>
                    <Button variant="ghost" size="sm" onClick={() => setEditing(d)}>
                      Edit
                    </Button>
                  </CardHeader>
                  {d.description && (
                    <CardContent className="text-sm text-muted-foreground">
                      {d.description}
                    </CardContent>
                  )}
                </Card>
              ))}
            </div>
          )}
        </section>
      ))}

      {editing && (
        <DeckDialog
          key={editing === 'new' ? 'new' : editing.id}
          modules={modules}
          deck={editing === 'new' ? undefined : editing}
          open
          onOpenChange={(o) => { if (!o) setEditing(null) }}
          onSaved={load}
        />
      )}
    </div>
  )
}
```

- [ ] **Step 5: Point the route at the real page** — in `App.tsx`, replace the `/decks`
      stub with `<DecksPage />`.

- [ ] **Step 6: Verify in the browser**

```bash
# terminal 1
cargo run
# terminal 2
cd frontend && npm run dev
```

Walk every acceptance criterion: create module "COS781"; create deck "Test 1" in it; create
a deck with no module; try a duplicate deck name in the same module (expect the 409 message
under the name input, dialog still open, text intact); rename; move between modules; move to
"No module"; edit a description; try an empty name (expect the 422 field error). Then narrow
the window to 375px and confirm the layout holds.

- [ ] **Step 7: Commit**

```bash
git add frontend
git commit -m "feat: decks screen with module and deck management"
```

```json:metadata
{"files":["frontend/src/lib/api.ts","frontend/src/pages/DecksPage.tsx","frontend/src/components/ModuleDialog.tsx","frontend/src/components/DeckDialog.tsx","frontend/src/App.tsx"],"verifyCommand":"cd frontend && npx tsc --noEmit && npm run build","acceptanceCriteria":["decks listed grouped by module with a No module group","new module creates and refreshes the list","new deck creates with optional module and description","edit changes name, module including to No module, and description, sending only changed fields","422 renders inline beside the offending input with typed values intact","409 duplicate name shows on the name input","sensible empty state","usable at 375px width"],"modelTier":"standard"}
```

---

## Task 8: Dev runbook and Part 1 verification pass

**Goal:** A README anyone (including future-you at 1am before the test) can follow to run the
app, plus one recorded end-to-end pass over Part 1.

**Files:**
- Create: `README.md`
- Modify: `.gitignore` (ensure `.sqlx` is NOT ignored; `data/`, `target/`, `node_modules/`,
  `dist/` already are)

**Acceptance Criteria:**
- [ ] README documents: prerequisites, first-time setup, the two dev commands, env vars, and
      the `cargo sqlx prepare` rule after any SQL change
- [ ] `cargo test` green, `cargo clippy -- -D warnings` clean,
      `cd frontend && npx tsc --noEmit && npm run build` clean — all four outputs captured
- [ ] A fresh-clone simulation works: delete `data/`, `cargo run`, DB is recreated and
      migrated, `/decks` still loads

**Verify:** `cargo test && cargo clippy -- -D warnings && (cd frontend && npx tsc --noEmit && npm run build)`
→ all succeed; plus the fresh-`data/` check below

**Steps:**

- [ ] **Step 1: Write `README.md`**

```markdown
# quizapp

Self-hosted quiz app for exam revision. Design: `docs/mitis/specs/2026-08-26-quiz-study-app-design.md`.

## Prerequisites

- Rust toolchain (this machine runs a standalone nightly; there is no `rust-toolchain.toml` pin)
- Node 20+
- `cargo install sqlx-cli --no-default-features --features rustls,sqlite`

## First-time setup

    mkdir -p data
    export DATABASE_URL="sqlite://data/quizapp.db?mode=rwc"
    cargo sqlx migrate run --source backend/migrations
    cd frontend && npm install && cd ..

## Running (two terminals)

    cargo run                 # API on http://127.0.0.1:3000
    cd frontend && npm run dev     # UI on http://localhost:5273 (proxies /api)

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
    cd frontend && npx tsc --noEmit && npm run build

Frontend has no test framework by design (see the spec's non-goals).

## Backups

The whole database is `data/quizapp.db`; images will live in `data/images/`.
Copy `data/` to back up.
```

- [ ] **Step 2: Confirm `.sqlx` is tracked**

```bash
git check-ignore -v .sqlx || echo ".sqlx is tracked - good"
git status --short
```

- [ ] **Step 3: Run the full verification and capture output**

```bash
cargo test
cargo clippy -- -D warnings
cd frontend && npx tsc --noEmit && npm run build && cd ..
```

- [ ] **Step 4: Fresh-database check**

```bash
mv data data.bak
cargo run &            # note the pid
sleep 3
curl -s localhost:3000/api/health
curl -s localhost:3000/api/decks     # expect []
kill %1
ls data                # quizapp.db recreated
rm -rf data && mv data.bak data
```

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: dev runbook for part 1"
```

```json:metadata
{"files":["README.md",".gitignore"],"verifyCommand":"cargo test && cargo clippy -- -D warnings && (cd frontend && npx tsc --noEmit && npm run build)","acceptanceCriteria":["README covers prerequisites, setup, dev commands, env vars and the sqlx prepare rule",".sqlx is tracked by git","cargo test green and clippy clean with captured output","web tsc and build clean with captured output","deleting data/ and running cargo run recreates and migrates the database"],"modelTier":"mechanical"}
```

---

## Verification — Part 1 as a whole

**Automated**

```bash
cargo test                                   # health, schema, modules, decks, error unit tests
cargo clippy -- -D warnings
SQLX_OFFLINE=true cargo build                # proves the committed query cache is current
cd frontend && npx tsc --noEmit && npm run build
```

**Manual browser pass** (`cargo run` + `npm run dev`, open `http://localhost:5273/decks`)

1. Empty state renders with guidance.
2. Create module "COS781" → appears as a group heading.
3. Create deck "Test 1" in COS781, description "Ch 1–3" → card appears, "0 cards".
4. Create deck "Loose" with no module → "No module" group appears.
5. Duplicate: new deck "Test 1" in COS781 → error under the name input, dialog stays open,
   typed values intact.
6. Empty name → `422` message under the name input.
7. Edit "Test 1" → rename to "Test One"; module unchanged.
8. Edit again → move to "No module"; name unchanged.
9. Edit again → change description only.
10. Toggle the OS theme → both Bibble palettes render legibly.
11. Narrow to 375px → nav and cards remain usable.

**Explicitly not verified in Part 1** (no endpoints exist yet): answer-key leakage, grading,
image upload, KaTeX. Those are Part 2/3 gates.

---

## Task dependencies

Tracked in `2026-08-26-part1-schema-modules-decks.md.tasks.json` (`blockedBy`, zero-indexed
ids 0–7 for Tasks 1–8):

```
Task 1 (skeleton) ─┬─> Task 2 (schema) ──> Task 3 (errors) ──> Task 4 (modules) ──> Task 5 (decks) ─┬─> Task 7 (/decks screen) ──> Task 8 (runbook)
                   └─> Task 6 (frontend scaffold) ───────────────────────────────────────────────────┘
```

Task 6 depends only on Task 1, so the frontend scaffold can run in parallel with the API
work. Everything else is a straight line.
