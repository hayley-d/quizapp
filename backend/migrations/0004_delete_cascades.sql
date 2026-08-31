-- Deleting a card now removes its review history with it. This reverses the decision
-- recorded in 0001 and in docs/HANDOVER.md, where `reviews.card_id` deliberately carried
-- no cascade so that a stray delete would fail loudly. Deletion is now an intended action
-- offered by the interface, so the history goes with the card rather than blocking it.
--
-- SQLite cannot alter a foreign key in place, so the table is rebuilt. Two properties make
-- that safe here. The migration runner's connection does not set PRAGMA foreign_keys (see
-- the header of 0001_init.sql), so the drop and rename cannot trip enforcement part-way
-- through. And no table references `reviews`, so the rename has no other foreign-key
-- clauses to rewrite.
--
-- Rows are copied with their explicit ids, which keeps AUTOINCREMENT's sqlite_sequence
-- high-water mark correct on the rebuilt table.

CREATE TABLE reviews_rebuilt (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  card_id     INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
  session_id  INTEGER NOT NULL REFERENCES sessions(id),
  answered_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
  given       TEXT,
  correct     INTEGER NOT NULL CHECK (correct IN (0,1)),
  overridden  INTEGER NOT NULL DEFAULT 0 CHECK (overridden IN (0,1)),
  ms          INTEGER,
  self_grade  TEXT CHECK (self_grade IN ('again','hard','good','easy'))
);

INSERT INTO reviews_rebuilt
  (id, card_id, session_id, answered_at, given, correct, overridden, ms, self_grade)
SELECT id, card_id, session_id, answered_at, given, correct, overridden, ms, self_grade
FROM reviews;

DROP TABLE reviews;

ALTER TABLE reviews_rebuilt RENAME TO reviews;

CREATE INDEX idx_reviews_card_time ON reviews(card_id, answered_at);
CREATE INDEX idx_reviews_session ON reviews(session_id);
