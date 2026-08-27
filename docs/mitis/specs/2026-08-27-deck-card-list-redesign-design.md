# Deck card list redesign

**Date:** 2026-08-27
**Status:** approved, not implemented
**Scope:** the `/decks/:id` screen, plus the card ordering it needs from the backend

Redesigns the deck detail screen from a flat row list into a deck of flippable cards, from a
supplied mockup. Four capabilities are new: drag-to-reorder with persisted order, click-to-flip
revealing the card's answer, a two-column multiple-choice grid, and a reveal/hide toggle for the
correct answer.

Colour roles come from the mockup; the actual colours come from the existing theme tokens in
`frontend/src/styles/globals.css`. The mockup's palette is not adopted.

## 1. Data layer

### 1.1 Migration `backend/migrations/0002_card_position.sql`

```sql
ALTER TABLE cards ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
UPDATE cards SET position = (
  SELECT COUNT(*) FROM cards c2
   WHERE c2.deck_id = cards.deck_id
     AND (c2.created_at < cards.created_at
          OR (c2.created_at = cards.created_at AND c2.id < cards.id))
);
CREATE INDEX idx_cards_deck_position ON cards(deck_id, position);
```

The backfill reproduces the current `created_at ASC, id ASC` ordering exactly, so existing decks
read unchanged across the migration.

### 1.2 Invariants

- `position` is **0-based and dense per deck**.
- **Archived cards occupy positions.** Archiving does not renumber. One ordering per deck is
  simpler than two views that can disagree, and un-archiving a card returns it to where it was.
- **No `UNIQUE(deck_id, position)` index.** A whole-deck renumber assigns positions row by row and
  would trip a unique constraint mid-transaction. The plain index above is for ordering only.
- Card create assigns `COALESCE(MAX(position), -1) + 1` within the deck, inside the existing
  create transaction.
- `position` is exposed on `CardSummary` and `Card`. It is **server-assigned**: card create and
  PATCH never accept it, in the same way `choices.position` and `accepted.normalised` do not.

### 1.3 Ordering

`GET /api/cards` changes `ORDER BY created_at ASC, id ASC` to `ORDER BY position ASC, id ASC`.
The `id ASC` tiebreak is retained for the same determinism reason recorded at that query today.

## 2. `POST /api/cards/:id/move`

Body: `{ "before": <card_id> | null }` — move this card to immediately before that card, or to the
end of its deck when `null`. The deck is derived from the card, matching the card-scoped shape of
`archive` and `unarchive`.

**Why relative, not a full permutation.** The list can be filtered: with "show archived" off the
client does not know where the hidden cards sit, so it cannot honestly send a complete order.
"Before card X" stays well-defined whatever is filtered out, and is idempotent.

**Implementation.** Read the deck's card ids in order, compute the new order in Rust, rewrite every
position in one transaction. O(n) writes per move, which is nothing for decks of a few hundred
cards, and is obviously correct where gap arithmetic is not.

**Validation** (`AppError::validation`, field `before`):

- 404 if `:id` does not exist.
- `before` must exist and belong to the same deck as `:id`.
- `before == id` is rejected.
- `before: null` is valid and means end of deck.

**Response:** the moved card, as `archive`/`unarchive` return theirs.

**`updated_at` is not bumped.** Position is list metadata, not a content edit; Part 3's scheduling
must not see a reorder as a revision.

## 3. Fetching the card back

`DeckPage` holds a `Map<number, Card>` cache. The first flip of a card calls the existing
`GET /api/cards/:id`, which already returns `choices` and `accepted`; later flips are instant. The
back renders a small skeleton while that first request is in flight.

- One `AbortController` per card id, cancelled if the card is flipped back before the response
  lands — otherwise a slow request repopulates a card the user has already closed.
- Archive/unarchive evicts that card's cache entry.
- A save in the card editor navigates back and remounts the page, so the cache cannot outlive its
  own data.
- On fetch failure: a toast, and the card returns to its front face rather than resting on a
  broken back.

`GET /api/cards` is deliberately **not** extended to carry children. Every deck load would pay for
answers nobody looked at, and the `CardSummary` / `Card` distinction would stop meaning anything.

## 4. The flip

`frontend/src/components/deck/useFlip.ts` — a half-flip, not a two-faced 3D flip.

A true two-faced flip needs both faces absolutely positioned, which needs a fixed card height. This
list renders unclamped multi-line markdown by an existing recorded decision, so heights vary.

Three states: `front`, `turning`, `back`.

1. Click → `turning`; `rotateY(90deg)` over 150ms.
2. On `transitionend` → swap the rendered face and jump the transform to `rotateY(-90deg)` with
   transitions suppressed for one frame.
3. Next frame → `rotateY(0)` over 150ms.

Only one face is mounted at a time, so the row keeps its natural height and the height change
happens while the card is edge-on and invisible. `perspective` sits on the wrapper.

Under `prefers-reduced-motion: reduce` the rotation is skipped and the face swaps immediately — the
same state machine with zero-duration transitions.

## 5. Row structure

One bordered card per row. A grip sits outside the card on the left.

**Grip** — the dnd-kit activator button, `cursor-grab`, keyboard-sortable.

**Header strip**, inside the card, above the body:

- kind badge
- edit pill and archive/unarchive pill: today's icons, `secondary` variant, `rounded-full`, per the
  mockup's filled pills rather than the current ghost icon buttons
- reveal-eye button on the right, multiple choice only

**Body** — the flip target. `role="button"`, `tabIndex=0`, Enter and Space, with an `aria-label`
alternating "Show answer" and "Show question".

- **Front:** prompt markdown left, image thumbnail right.
- **Back:** per kind, see §6.

**Click targets.** All controls live in the header strip, *outside* the flip target — which also
avoids nesting buttons inside a button. The image thumbnail keeps its existing lightbox and calls
`stopPropagation`, so it is a deliberate non-flipping region inside the body.

### 5.1 Deviations from the mockup

Both are deliberate and recorded here so they are not read as omissions:

1. **The kind badge shows on both faces.** The mockup shows it only on flipped rows. The badge is
   orientation-independent information, and its absence from the front reads as a missing element
   rather than a choice.
2. **Kind wording comes from `KIND_LABEL`** (`Multiple choice` / `Short answer` / `Flashcard`), not
   the mockup's "Single". `KIND_LABEL` is the recorded single source of truth and `CardEditorPage`
   renders from it; new wording here would let the two screens drift.

## 6. Back faces by kind

- **Flashcard** — `answer_md` through `<Markdown>`, then `explanation_md` in muted text if present.
- **Short answer** — accepted answers, primary first: the primary emphasised, alternates beneath as
  muted chips.
- **Multiple choice** — a grid, `sm:grid-cols-2`, single column on narrow screens, choices lettered
  A–D by `position`.

**Reveal states.** Per card, reset when the card flips back.

| State | Choice styling |
|---|---|
| Unrevealed (default) | all uniform `bg-accent/85 text-accent-foreground` — the mockup's purple |
| Revealed, correct | `bg-success text-success-foreground` — the token is already annotated *"gold — correct answers"* |
| Revealed, incorrect | `bg-muted text-muted-foreground` |

Hidden by default, so the deck can be used for self-testing without an answer key on screen.

The eye button carries `aria-pressed` and labels "Reveal answer" / "Hide answer". The revealed
correct choice also carries a visually-hidden "Correct answer", so the state is never
colour-only.

## 7. Drag and drop

`@dnd-kit/core` + `@dnd-kit/sortable`, new dependencies. They handle variable-height rows, the drop
indicator, auto-scroll, and pointer and touch input, and they give keyboard reorder (grip → Space →
arrows) with `aria-live` announcements — none of which a hand-rolled implementation would get for
free, and native HTML5 drag has no keyboard path at all.

- `onDragEnd` gives the active id and the id it landed over. **`before` is not `over.id`.**
  Dragging downwards, the over-row is the one the card displaces, so sending `over.id` as `before`
  is off by one. The client applies the move to its local array first, then reads `before` as the id
  that *follows* the moved card in the new order — `null` if it is now last. That is correct in both
  directions and needs no special case.
- The move is applied **optimistically** to local state, then sent. On failure: a toast, revert,
  and refetch the list.
- Dropping past the last row sends `before: null`.
- Archived rows are draggable when visible; when hidden they keep their positions untouched.

## 8. Page header

Gains the mockup's back arrow to `/decks`. Keeps the deck name, module badge, description and card
count. "New card" becomes the accent-filled "Add Card". The show-archived switch stays, and
archived rows keep their dimming and "Archived" badge.

## 9. Files

| Path | Change |
|---|---|
| `backend/migrations/0002_card_position.sql` | new |
| `backend/src/routes/cards.rs` | `move` handler, `position` in the DTO, list ordering, create assignment |
| `backend/tests/cards.rs` | move endpoint, validation, ordering, archive interaction |
| `frontend/package.json` | `@dnd-kit/core`, `@dnd-kit/sortable` |
| `frontend/src/lib/api.ts` | `moveCard`, `position` on `CardSummary` |
| `frontend/src/pages/DeckPage.tsx` | header, `DndContext`, card cache, row extraction |
| `frontend/src/components/deck/CardRow.tsx` | new — sortable row, header strip, flip target |
| `frontend/src/components/deck/CardBack.tsx` | new — per-kind back, MC grid, reveal state |
| `frontend/src/components/deck/useFlip.ts` | new — half-flip state machine |

`DeckPage` carries the whole list inline today; extracting the row is what keeps it readable once a
row has a header strip, two faces and a flip state machine. No other refactoring is in scope.

## 10. Testing

Backend, in `backend/tests/cards.rs` (the project has no frontend test framework, by an existing
spec decision):

- migration backfill preserves creation order
- list returns `position ASC`
- create appends at the end of the deck
- move to the middle, to the front, and `before: null` to the end, each verified by a follow-up list
- move with a `before` in another deck → 400 with field `before`
- move with `before == id` → 400
- move a nonexistent card → 404
- positions stay dense and 0-based after a sequence of moves
- archive then unarchive leaves position unchanged
- moving a card while archived cards are interleaved keeps the archived cards' relative order

Frontend verification is the existing gate: `pnpm exec tsc --noEmit && pnpm build`, plus a manual
walkthrough of flip, reveal, drag, keyboard reorder, and reduced-motion.

## 11. Out of scope

- Study/practice order consuming `position` — that is Part 3's business.
- Reordering across decks, or moving a card to another deck.
- Bulk selection or multi-card drag.
- Any card editor change. `position` is server-assigned and absent from `CardInput`, so the editor
  needs none.
