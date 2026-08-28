-- Flashcards are self-graded on a four-level scale (again / hard / good / easy), but
-- `reviews.correct` is a single bit. Storing only the bit would throw away the distinction
-- between "hard" and "easy" at the moment it is made, and SM-2 (build step 7) needs those
-- levels as its quality input -- by which time the history would already be gone.
--
-- Nullable, because the two auto-graded kinds have no self-grade. A NULL passes the CHECK:
-- `NULL IN (...)` evaluates to NULL, and SQLite accepts a CHECK that is not false.
--
-- `correct` stays NOT NULL and is derived on write: 'again' -> 0, the other three -> 1.
-- Two columns rather than one because they answer different questions -- "did I get it"
-- is queried by every statistic, "how hard was it" only by the scheduler.
ALTER TABLE reviews ADD COLUMN self_grade TEXT
  CHECK (self_grade IN ('again','hard','good','easy'));
