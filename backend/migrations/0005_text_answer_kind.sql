-- The `short_answer` kind is renamed to `text_answer`. The type was never about the answer
-- being short: it is a free-text answer graded by matching against the accepted answers, and
-- the interface now offers a multi-line box for it.
--
-- SQLite cannot alter a CHECK constraint in place, so `cards` is rebuilt, as 0004 rebuilt
-- `reviews`. Rebuilding `cards` is harder than rebuilding `reviews` was, because four tables
-- reference `cards(id)` -- `choices`, `accepted`, `reviews` and `schedule` -- and every one of
-- them cascades on delete. `DROP TABLE cards` therefore performs an implicit delete that runs
-- those cascades and empties all four.
--
-- The cascade cannot simply be switched off. `PRAGMA foreign_keys` is a no-op inside a
-- transaction, and sqlx's SQLite migrator always wraps a migration in one: its `apply` ignores
-- the `-- no-transaction` marker, which only the Postgres and MySQL drivers honour. Renaming
-- the old table aside instead does not help either, because `ALTER TABLE ... RENAME TO`
-- rewrites the four children to reference the new name, so dropping it still cascades.
--
-- So the child rows are carried across the rebuild in ordinary tables, and put back once
-- `cards` exists again under its own name. The whole migration runs in sqlx's transaction, so
-- it either lands complete or not at all. Restoring the rows satisfies the foreign keys
-- because every carried row's parent card is copied first, under its original id. The
-- carriers are made with `CREATE TABLE ... AS SELECT *`, so their columns match their source's
-- order and the plain `INSERT ... SELECT *` back is exact.
--
-- Rows are copied with their explicit ids, which keeps AUTOINCREMENT's sqlite_sequence
-- high-water mark correct on the rebuilt table. The children keep their own marks throughout:
-- a cascade delete never lowers sqlite_sequence.

CREATE TABLE choices_carried AS SELECT * FROM choices;
CREATE TABLE accepted_carried AS SELECT * FROM accepted;
CREATE TABLE reviews_carried AS SELECT * FROM reviews;
CREATE TABLE schedule_carried AS SELECT * FROM schedule;

CREATE TABLE cards_rebuilt (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  deck_id        INTEGER NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
  kind           TEXT NOT NULL CHECK (kind IN ('mc_single','text_answer','flashcard')),
  prompt_md      TEXT NOT NULL,
  image_path     TEXT,
  answer_md      TEXT,
  explanation_md TEXT,
  archived       INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0,1)),
  created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
  updated_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
  position       INTEGER NOT NULL DEFAULT 0
);

INSERT INTO cards_rebuilt
  (id, deck_id, kind, prompt_md, image_path, answer_md, explanation_md, archived,
   created_at, updated_at, position)
SELECT id,
       deck_id,
       CASE WHEN kind = 'short_answer' THEN 'text_answer' ELSE kind END,
       prompt_md,
       image_path,
       answer_md,
       explanation_md,
       archived,
       created_at,
       updated_at,
       position
FROM cards;

DROP TABLE cards;

ALTER TABLE cards_rebuilt RENAME TO cards;

CREATE INDEX idx_cards_deck_archived ON cards(deck_id, archived);
CREATE INDEX idx_cards_deck_position ON cards(deck_id, position);

INSERT INTO choices SELECT * FROM choices_carried;
INSERT INTO accepted SELECT * FROM accepted_carried;
INSERT INTO reviews SELECT * FROM reviews_carried;
INSERT INTO schedule SELECT * FROM schedule_carried;

DROP TABLE choices_carried;
DROP TABLE accepted_carried;
DROP TABLE reviews_carried;
DROP TABLE schedule_carried;
