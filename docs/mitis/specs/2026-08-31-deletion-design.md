# Deleting decks, modules and cards

Date: 2026-08-31

## Goal

Make deletion real for all three container levels — cards, decks, modules — and remove
the archive button from the deck screen.

This reverses a documented decision. `docs/HANDOVER.md:858` reads "Archive, never
delete", and explains that `reviews.card_id` deliberately carries no `ON DELETE CASCADE`
"so a stray delete fails loudly". After this change a delete is intended, cascades, and
takes review history with it.

## Decisions taken

| Question | Decision |
| --- | --- |
| A deleted card's `reviews` rows | Cascade. History goes with the card; deck and session accuracy figures shift retroactively. |
| A deleted module's decks | Orphaned, not deleted. The existing `ON DELETE SET NULL` already does this; the decks reappear under "No module". |
| The `archived` column and its endpoints | Kept. Only the button and the two frontend wrappers go. |
| Confirmation friction | An alert dialog naming exact counts, for all three deletes. Cancel holds initial focus. |
| Where the deck and module controls live | Inside the existing edit dialogs. No new icons on the browsing grid. |

Rejected: ripping archive out entirely. It would mean dropping the column, both
endpoints, every `archived = 0` filter across sessions, stats and decks, and their tests
— a far larger and riskier change than the one asked for.

## Data layer

### Migration `0004_delete_cascades.sql`

One job: rebuild `reviews` so `card_id` reads
`INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE`.

SQLite cannot alter a foreign key in place, so this is the standard rebuild — create
`reviews_rebuilt` with the identical schema plus the cascade, `INSERT … SELECT` every
row, `DROP TABLE reviews`, rename, then recreate `idx_reviews_card_time` and
`idx_reviews_session`.

Two properties make the rebuild safe, both worth stating because a future reader will
wonder:

- `PRAGMA foreign_keys` is not set on the migration runner's connection — only on real
  app connections, via `.foreign_keys(true)`. This is established by the header comment
  in `0001_init.sql`. So the drop-and-rename cannot trip enforcement mid-flight.
- Nothing in the schema references `reviews`, so there are no other foreign-key clauses
  for the rename to rewrite.

No other migration is needed. Every remaining cascade already exists:

| Deleting | Cascade path | Already present? |
| --- | --- | --- |
| card | → `choices`, `accepted`, `schedule` | yes |
| card | → `reviews` | new in 0004 |
| deck | → `cards`, and through them all of the above | yes |
| module | → `decks.module_id` set to null; the decks survive | yes |

### Two consequences, both accepted

**`sessions` rows outlive their decks.** `sessions.deck_ids` is a JSON text column with
no foreign key, so deleting a deck leaves sessions pointing at an id that is gone. Their
`reviews` cascade away, so such a session reads as zero-answered. They are left in place:
nothing in the interface lists sessions, a session with no reviews is already a
legitimate state (an abandoned one), and cleaning them up would mean parsing JSON arrays
in SQL to find the matches.

**Deleting mid-session is already-handled behaviour.** `/next` finds no candidate and the
session reaches the exhausted screen `SessionPage` renders today. For a mock test,
`answered_count` finishes below `target_count` — which `docs/HANDOVER.md:646` already
documents as a legitimate outcome of archiving mid-test. Same shape, no new work.

### Documentation

`docs/HANDOVER.md:858` is rewritten. Left as it stands it would actively mislead: it
tells the next reader that a delete fails loudly, which is no longer true. It becomes a
note that deletion is real and cascades, and that the `archived` column survives as a
session and stats filter with no interface driving it.

## Backend

Four routes. Each delete checks existence first, so an unknown id is a 404 through
`AppError::NotFound` rather than a silent success.

```
DELETE /api/cards/{id}                 → 204
DELETE /api/decks/{id}                 → 204
DELETE /api/modules/{id}               → 204
GET    /api/decks/{id}/deletion-impact → { card_count, review_count }
```

Each delete is a single `DELETE … WHERE id = ?`; the cascades do the rest. `cards.rs` and
`decks.rs` already have `fetch_summary` / `fetch_one` for the existence check;
`modules.rs` needs a small `fetch_one` added alongside its list query. `204 No Content`
because there is nothing meaningful to return, and `request()` in `lib/api.ts` already
maps 204 to `undefined`.

### Why `deletion-impact` is its own route

The confirmation promises an exact count and no existing response can supply an honest
one. `DeckResponse.card_count` and every figure in `DeckStatsSummary` filter
`archived = 0`, but a delete takes archived cards and their reviews too. With the archive
button gone, archived cards become permanently invisible, so a warning that undercounts
them is the worst kind of wrong on an irreversible action.

The endpoint is one query counting all cards in the deck and all reviews on those cards,
with no archived filter.

The alternative — adding `total_card_count` and `review_count` to `DeckResponse` — was
rejected: two extra subqueries on every row of the decks list to serve one dialog, and
two fields whose names invite confusion with the `card_count` beside them.

### The other two confirmations need no backend work

- **Cards.** `CardStats.attempt_count` is already loaded by `DeckPage` and already passed
  into `CardRow`. It comes from the `counts` CTE in `load_card_stats`, which is a full
  `COUNT(*)` per card, not the recency-limited `recent` CTE.
- **Modules.** `ModuleResponse.deck_count` already exists, and since modules orphan
  rather than cascade, "its 4 decks move to No module" is the whole story.

### Tests

Extending the existing files rather than adding new ones. Each of these is written so it
would fail against the wrong implementation — the standing rule from
`docs/HANDOVER.md`, where roughly one test per task has historically been hollow.

`backend/tests/cards.rs`

- Delete returns 204 and the card is gone.
- Its `choices`, `accepted`, `schedule` and `reviews` rows are gone with it. The reviews
  half is the assertion that proves migration 0004 works, so the card under test must
  genuinely have a review row — deleting a never-answered card would pass on the old
  schema too.
- An unknown id is 404.

`backend/tests/decks.rs`

- Delete cascades to the deck's cards and through them to their reviews.
- An unknown id is 404.
- `deletion-impact` counts an archived card and that card's reviews. This is the
  assertion that fails if someone copies the `archived = 0` filter in from the
  neighbouring queries.

`backend/tests/modules.rs`

- Delete returns 204 and the module is gone.
- **Its decks still exist, with `module_id` now null.** That second half is the point;
  asserting only the 204 would pass against a cascade.

`.sqlx` is regenerated against a scratch database, never `data/quizapp.db` — per the
warning at `docs/HANDOVER.md:855`.

## Frontend

### Two new files

`components/ui/alert-dialog.tsx` — shadcn's alert dialog, over the `radix-ui` package
already in `package.json`. Vendored, so it is exempt from the naming and `any` rules per
CLAUDE.md, and must not be hand-edited afterwards.

`components/ConfirmDeleteDialog.tsx` — the single confirmation used by all three deletes.
Props: `open`, `onOpenChange`, `title`, `lines`, `confirmLabel`, `busy`, `onConfirm`.
Cancel holds initial focus; confirm is destructive-styled. One component rather than
three inline dialogs, because the three deletes differ only in their text.

### Cards

`components/deck/CardRow.tsx` — `onArchiveToggle` becomes `onDelete`, the button becomes
`Trash2`, and the `Archive` / `ArchiveRestore` imports go.

The `Archived` badge and the `card.archived && 'opacity-60'` class are **left alone**.
They are unreachable today and stay unreachable; `docs/HANDOVER.md:1417` already records
them as deliberately-kept dead paths behind an API that still serves `archived=all`.
Removing them is a different decision than the one taken here.

`pages/DeckPage.tsx` — `archive()` and `unarchive()` are replaced by one `deleteCard()`.
The confirmation reuses the existing truncated `promptLabel` and adds
`cardStats.attempt_count` when stats have loaded: "Delete this card? 12 recorded answers
go with it." When `cardStats` is `undefined` or `null` the answers clause is omitted
rather than guessed at. On success: `loadCards()`, `reloadDeck()`, and a stats reload,
since accuracy figures shift the moment reviews cascade.

### Decks

`components/DeckDialog.tsx` — a destructive `Delete deck` in the footer, left of `Save`,
rendered only when `deck` is present. Clicking it fetches `deletion-impact` and opens the
confirmation.

A new `onDeleted` prop, distinct from `onSaved`: `DecksPage` must reload modules as well
as decks, because the deck counts shown on the module list change.

### Modules

`components/ModuleDialog.tsx` grows from create-only into a manage dialog. There is no
module list surface in the app today, so one has to exist for deletion to be reachable.

Trigger label goes from `Create module` to `Modules`. Body: the existing name input and
add button, a separator, then the module list with deck counts and a trash per row.

It takes `modules` as a prop — `DecksPage` already holds them, and re-fetching inside
would give two sources of truth for one list.

Confirmation: "Delete COS781? Its 4 decks will be kept, and move to No module." No dire
warning, because nothing is actually lost.

One consequence: deleting the module that is currently the active filter in `DecksPage`
leaves `moduleFilter` pointing at a gone id, and `moduleName()` would render "Unknown
module". So the changed-modules callback resets the filter to `all` when the deleted
module was the selected one.

### `lib/api.ts`

Adds `deleteCard`, `deleteDeck`, `deleteModule`, `getDeckDeletionImpact` and a
`DeckDeletionImpact` type.

`archiveCard` and `unarchiveCard` are removed. With the button gone they have no caller,
and a wrapper for a screen that no longer exists is worse than reaching for the endpoint
again later. The backend endpoints and their tests stay, so the capability is intact.

## Out of scope

- Undo, a trash bin, or any soft-delete tier. Deletion is immediate and final.
- Bulk delete or multi-select.
- Cleaning up `sessions` rows that reference deleted decks.
- Removing the `archived` column, its endpoints, or its filters.
