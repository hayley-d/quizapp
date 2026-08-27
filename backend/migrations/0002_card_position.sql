-- Cards carry an explicit per-deck order so the deck screen can be reordered
-- by hand. Backfilled from `created_at ASC, id ASC` — the ordering the list
-- query used before this column existed — so every existing deck reads exactly
-- as it did before the migration.
--
-- Archived cards occupy positions like any other: one ordering per deck is
-- simpler than two views that can disagree, and it means un-archiving a card
-- returns it to where it was.
ALTER TABLE cards ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

UPDATE cards SET position = (
  SELECT COUNT(*) FROM cards c2
   WHERE c2.deck_id = cards.deck_id
     AND (c2.created_at < cards.created_at
          OR (c2.created_at = cards.created_at AND c2.id < cards.id))
);

-- Ordering only. Deliberately NOT unique on (deck_id, position): a whole-deck
-- renumber assigns positions row by row and would trip a unique constraint
-- part-way through the transaction.
CREATE INDEX idx_cards_deck_position ON cards(deck_id, position);
