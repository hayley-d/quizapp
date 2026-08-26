-- Foreign key enforcement is NOT set here: PRAGMA foreign_keys only affects
-- the connection that runs it, and the migration runner's connection isn't
-- reused for app queries. Enforcement is turned on for every real connection
-- via `.foreign_keys(true)` in backend/src/db.rs.

CREATE TABLE modules (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  name       TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);

CREATE TABLE decks (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  module_id   INTEGER REFERENCES modules(id) ON DELETE SET NULL,
  name        TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
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
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
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
  started_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
  ended_at     TEXT
);

-- Append-only: never UPDATEd or DELETEd, except the override endpoint flipping
-- correct/overridden on a single row (spec: "Reviews are append-only").
CREATE TABLE reviews (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  card_id     INTEGER NOT NULL REFERENCES cards(id),
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  answered_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
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
