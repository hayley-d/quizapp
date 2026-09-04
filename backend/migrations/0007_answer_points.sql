-- An answer that is written as a list is learned point by point, not as one opaque string.
-- Before this migration the only way to grade "list the 7 requirements" was
-- `grade_text_answer`, which demands exact equality against a whole accepted key, so a
-- seven-point answer could only ever be marked wrong and rescued by the manual override.
-- That override additionally writes the typed attempt back as a new accepted row, which
-- pollutes the very cards it is used most on.
--
-- `multi_point_mode` is the author's say over the automatic detection. 'auto' lets the
-- splitter decide from the list markers in the answer, and is correct for every card that
-- exists today. 'on' forces a list that the splitter would read as prose; 'off' rescues a
-- prose answer that happens to look like a list. The default makes this migration a no-op
-- for existing rows.
ALTER TABLE cards ADD COLUMN multi_point_mode TEXT NOT NULL DEFAULT 'auto'
  CHECK (multi_point_mode IN ('auto','on','off'));

-- The score of a multi-point review, denormalised out of `review_answer_points` so that the
-- session runner, the verdict and the results screen can read "5 of 7" without joining the
-- detail table. NULL on every review of a card that is not multi-point, which is every
-- review recorded before this migration -- hence nullable rather than defaulted, following
-- the reasoning in 0006: `NULL >= 0` is NULL, and SQLite accepts a CHECK that is not false.
ALTER TABLE reviews ADD COLUMN points_total INTEGER CHECK (points_total > 0);
ALTER TABLE reviews ADD COLUMN points_recalled INTEGER CHECK (points_recalled >= 0);

-- Which points were recalled, one row per point offered on that attempt. `point_key` is the
-- normalised point text, which is what makes a point identifiable across reviews without a
-- points table of its own: the answer is still authored as one markdown string, so a point
-- has no stable id to reference. Rewording a point therefore starts its history over, which
-- is honest -- a reworded point is a different thing to recall. `point_text` is stored
-- alongside so that a past review still renders after the answer has been edited.
--
-- A focus repetition -- the second attempt at a card you got 5 of 7 on -- writes only the
-- points it asked about, so `points_total` for that review is the size of the focus subset
-- rather than the size of the whole list.
CREATE TABLE review_answer_points (
  review_id  INTEGER NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
  point_key  TEXT NOT NULL,
  point_text TEXT NOT NULL,
  recalled   INTEGER NOT NULL CHECK (recalled IN (0,1)),
  PRIMARY KEY (review_id, point_key)
);

CREATE INDEX idx_review_answer_points_key ON review_answer_points(point_key);
