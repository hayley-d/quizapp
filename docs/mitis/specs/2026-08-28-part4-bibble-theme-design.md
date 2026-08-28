# Part 4: the Bibble theme pass — design decisions

**Date:** 2026-08-28
**Author:** Hayley Dodkins (design session with Claude)
**Status:** decided; implementation plan to follow in `../plans/`

Companion to [`2026-08-26-quiz-study-app-design.md`](2026-08-26-quiz-study-app-design.md),
which is the master record. Read this one for *why*; read the master for *what the app is*.

Part 4 is build step 4, "Bibble theme applied across the app". In scope: the theme toggle,
the two named animations, the visual pass over every screen, and three Part 2c defects that
touch the same files. Not in scope: mock test (5), stats (6), SM-2 (7), the phone layout
pass and font vendoring (8).

---

## 1. The palette already exists; only its application is missing

`frontend/src/styles/globals.css` has carried a complete light and dark Bibble palette since
Part 1 — turquoise `--primary`, lilac `--secondary`, magenta `--accent`, gold `--success` for
correct answers — each re-exported through `@theme inline` as a `--color-*` so Tailwind
utilities exist for all of them.

So this is not a palette design exercise. Part 3's runner and picker were built to bare
shadcn defaults and simply never consumed the tokens. `DeckCard.tsx` is the one screen that
did, and it is therefore the reference for what "themed" means here:

```
flex min-h-64 flex-col overflow-hidden rounded-xl bg-[var(--deck-card)]
```

with an orchid header band, chips, and `font-display text-3xl` titles. Part 4 extends that
vocabulary to the rest of the app rather than inventing a second one.

**Consequence for the plan:** most tasks are class-string work against existing tokens. The
genuinely new CSS is small — a few motion tokens, two `@keyframes`, and one media query.

## 2. The `brand` button fails contrast in light mode

Three tokens are declared under a shared selector, identical in both themes:

```css
:root, .dark {
  --deck-card: var(--primary);
  --deck-card-header: rgb(211 112 224 / 0.7);
  --deck-card-chip: rgb(27 31 57 / 0.7);
}
```

This *looks* like a theming bug and the design session initially recorded it as one. It was
then measured, and the first reading was wrong in an instructive way.

Both literal tokens are at 70% alpha, so what they actually render depends on what they
composite over. Inside `DeckCard.tsx` the chain is chip → header band → `--deck-card` →
`--primary`, and `--primary` *is* per-theme. The shared declaration is therefore
self-correcting there:

| Surface | Light | Dark |
| --- | --- | --- |
| Deck card chip, white text | 10.10:1 | 9.24:1 |

**The bug is one token further out.** `components/ui/button.tsx`'s custom `brand` variant
reuses `--deck-card-header` — but on a button sitting on the *page*, with no turquoise card
beneath it to composite against:

```
brand: "bg-[var(--deck-card-header)] text-white hover:brightness-110",
```

| Surface | Light | Dark |
| --- | --- | --- |
| `brand` button, white text | **2.14:1 — fails WCAG AA** | 4.68:1 — passes |

In light mode that is white text on pale orchid `rgb(221 153 232)`. AA wants 4.5:1 for body
text and 3:1 even for large text; 2.14:1 clears neither.

This matters more than the chip would have, because `brand` is the app's primary action
everywhere: "Start practising", "Check", "Next card", "Study again", the deck edit button,
and all three icon buttons on every card row. **In light mode the main call to action across
the entire app is close to illegible** — which is exactly the class of thing that goes
unnoticed for three parts when there is no way to switch themes (decision 3).

**Decision: give the `brand` button its own opaque token, `--brand` / `--brand-foreground`,
and leave `--deck-card-header` alone.**

Two findings drive this, and both make the fix smaller than it first looked.

*The alpha is the entire bug.* An opaque colour's contrast does not depend on its backdrop,
so once the 70% is dropped, one value serves both themes and no per-theme split is needed.

*Dark mode already renders that colour.* `rgb(211 112 224 / 0.7)` composited over the dark
page is `rgb(159 88 174)`. Choosing `rgb(158 84 170)` — **4.88:1 with white text, comfortable
headroom over the 4.5:1 floor** — therefore leaves dark mode visually unchanged and fixes
light mode alone. A regression in the one theme that has actually been looked at is the main
risk in this whole pass, and this sidesteps it.

*Why a new token rather than editing `--deck-card-header`.* The two uses have genuinely
different backdrops: the band sits on turquoise inside `DeckCard`, the button sits on the
page. One token cannot be correct for both, and `DeckCard` is a screen that has already been
reviewed and approved — retuning its band to fix a button is a visual regression traded for
a contrast fix. Splitting keeps `DeckCard.tsx` untouched. The `brand` comment's intent
("the orchid band colour … shared by every create/add affordance") is preserved: it is the
same hue at a legible lightness, not a different colour.

`--deck-card-chip` stays exactly as it is. It measures 10.10:1 and 9.24:1; changing it would
be a restyle dressed up as a fix.

**This is verified by arithmetic, not by eye.** `frontend/scripts/check-contrast.py` computes
the ratios from the token values and exits non-zero below 4.5:1, so it belongs in the gate
rather than in a walkthrough step. A screenshot cannot tell you 4.4 from 4.6.

## 3. A 3-way theme toggle, with the `.dark` class still the single source of truth

Today `frontend/index.html` runs an inline script that follows the OS and toggles `.dark` on
`<html>`. `components/ui/sonner.tsx` observes that class and its comment names it the source
of truth.

**Decision: add a Light / Dark / System toggle to the `AppShell` header, persisted to
`localStorage`, which writes the same `.dark` class.** Three states, not two: "System" must
remain reachable, and a two-way toggle silently strands anyone who wants to follow the OS.

The class stays the single source. The toggle becomes another *writer* of it; nothing else
gains a way to know the theme. Sonner keeps working untouched, and a future screen reads the
theme the same way sonner does.

**The inline script stays inline and dependency-free.** It exists to prevent a flash of the
wrong theme before React mounts, so it cannot become a module. It gains one responsibility:
read the stored preference first, fall back to the OS query. This is the one piece of theme
logic that is deliberately duplicated between `index.html` and `useTheme.ts`, and the reason
is worth stating because a later tidying pass will want to remove it: **a module import
cannot run before first paint.**

Considered and rejected: `next-themes`. It solves exactly this, but it would displace the
`.dark` contract that sonner already depends on, for a hook the app can write in about 25
lines.

## 4. Reduced motion gets two layers, deliberately

`useFlip.ts` currently holds the codebase's only reduced-motion handling:

```js
if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
  setFace(next)
  return
}
```

Two problems. It is not reactive — it samples the query at flip time and never subscribes,
so toggling the OS setting mid-session has no effect until the next flip. And it is
per-component, so every new animation must remember to repeat it.

**Decision: both a global CSS media query and a shared `usePrefersReducedMotion` hook.**

They are not redundant. The CSS block neutralises animation and transition durations for
everything, including anything a later part adds and forgets — it is the safety net, and it
fails safe. The hook is for the cases where the correct reduced-motion behaviour is *a
different code path* rather than a shorter duration: `useFlip` must swap the face
immediately, because a zero-duration rotation would leave the card mid-flip at 90° with no
callback to finish it. CSS cannot express that; the hook can.

`useFlip` moves onto the hook, which also makes it reactive.

## 5. The sparkle burst must be structurally incapable of blocking the loop

The master spec is explicit that neither animation "blocks advancing to the next question —
during a study run, responsiveness beats flourish". In the runner, pressing Enter on a graded
card calls `loadNext()` immediately (`SessionPage.tsx:211`).

**Decision: pure CSS `@keyframes` on a fixed set of absolutely-positioned elements.** No
canvas, no animation library, no `requestAnimationFrame`, no per-burst randomisation.

This makes the guarantee structural rather than a thing the implementer must be careful
about. There is no JS running during the burst, so there is nothing that could delay the key
handler; and when `loadNext()` re-renders, the burst unmounts mid-animation and simply stops.
No cancellation logic, no cleanup, no `useEffect` teardown to get wrong.

The cost is that the burst is identical every time. Over a hundred-card COS781 session that
is a real downside, and randomised particles were considered for exactly that reason. They
were rejected because the variety would be bought with per-frame JS at precisely the moment
the user is pressing the next key — the one moment the spec singles out as protected.

The burst is `pointer-events-none` and `aria-hidden`; it is decoration over the verdict, and
`AnswerVerdict` already carries the accessible statement of the result.

## 6. The streak is client-side, and an override restores it

No consecutive-correct counter exists. `served.correct_count` is cumulative, not consecutive.

**Decision: a `consecutiveCorrect` number in `SessionPage` state, reset to 0 on a wrong
answer, lost on reload.**

This preserves the invariant `docs/HANDOVER.md` calls out — *"Session state lives only in
`reviews`"* — which is what lets a mid-session reload resume correctly with no client state.
Deriving the streak server-side from `reviews` would also honour that rule and would survive
reloads, but it is a backend endpoint change and a new query with new tests, spent on a
flourish. A streak is a moment of delight, not a durable fact.

**The override extends the streak, but does not re-fire the burst.** `override()`
(`SessionPage.tsx:178`) already increments `correct_count`, so the header accuracy figure
treats an overridden answer as correct. A streak that ignored the override would contradict
the percentage displayed a few inches away, and two counters telling different stories about
the same answer is a bug however it is rationalised. The burst is a reaction to the *moment*
of answering, and that moment has passed — restoring the count without replaying the
animation is the honest reading of both.

## 7. Zustand is deferred to Part 5, on purpose

Raised during the design session and worth recording so it is not re-litigated.

No state in Part 4 spans components. The streak is one integer read by one component. The
theme preference looks like a candidate, but the `.dark` class is already the source of truth
(decision 3) and a store would create a second one that can disagree with it.

**Part 5, the mock test, is the intended home.** It has a stable serve order, a
`target_count`, no per-question feedback, and a resume story that practice mode explicitly
does not have — genuinely cross-component state with a real problem for a store to solve.
Nothing in Part 4 forecloses it.

## 8. Which Part 2c defects ride along, and which do not

`docs/HANDOVER.md` records four known defects from Part 2c's review. Three are visual or
frontend-config and touch files this pass is already opening:

- **Markdown links flip the card.** `CardRow.tsx`'s flip `onClick` has no target check, so a
  click on an `<a>` inside a prompt bubbles into `flip()`. The keyboard path was already
  guarded; only the mouse path is exposed.
- **Two accessible-name problems on the card row.** The flip target's `aria-label` becomes the
  element's whole accessible name, so a focused card announces "Show answer" and nothing about
  *which* card; and the drag grip's label slices raw markdown, reading `$$…$$` and `# ` aloud
  as literal syntax. One `aria-labelledby` fix covers both.
- **`strict` is off for the frontend**, which `HANDOVER.md` calls "the highest-value frontend
  follow-up".

The other two are backend concurrency — `BEGIN IMMEDIATE` for `move_card` and `create`, and
the stale-list-fetch race during `moveCard`. Unrelated to this work and left where they are.

**Both config fixes were measured before being planned, not assumed:**

| Change | Errors today | Probe |
| --- | --- | --- |
| `strict: true` | **0** | `pnpm exec tsc -p tsconfig.app.json --noEmit --strict` |
| `typescript/no-explicit-any` | **0** | `pnpm exec oxlint --deny typescript/no-explicit-any src/` |

The `strict` zero was itself verified rather than trusted, because this repo has been bitten
once already by a type-check that silently checked nothing: the same command with
`--noUncheckedIndexedAccess` reports 4 errors, proving the probe can fail. (For the same
reason the gate uses `tsc -b --noEmit`; a bare `tsc --noEmit` reads a solution file with
`"files": []` and exits 0 whatever the code says.)

`no-explicit-any` makes CLAUDE.md rule 3 mechanically enforced instead of prose-only — but
only if `pnpm lint` joins the gate. Adding the rule without the command changes nothing, so
Part 4 does both or neither.

## 9. Small contract decisions

- **`/stats` is skipped.** It is an 8-line `StubPage`; theming a placeholder is work thrown
  away at build step 6.
- **`frontend/src/components/ui/` is not touched.** shadcn-generated, vendored, and exempt
  from the CLAUDE.md naming and `any` rules. The existing custom `brand` button variant is
  the sanctioned way to extend it, and any shadcn command offering to overwrite
  `globals.css` is declined.
- **Glow is `box-shadow` on a token, not `filter: blur()`.** "Softly glowing rather than
  sharp-edged" is a spec requirement; `filter` on a scrolling list of a hundred KaTeX-bearing
  rows is a rendering cost the COS781 deck cannot afford.
- **`SessionPage.tsx` sheds its two terminal screens** (the summary and the "nothing left to
  practise" state) into `components/session/`, beside the existing `ChoiceList` and
  `AnswerVerdict`. Both need theming regardless, so they get touched either way; moving them
  is nearly free and keeps a 417-line file from crossing 500. A full `useSessionRunner`
  extraction was rejected: it would put a refactor and a visual pass in one diff, and with no
  frontend test framework nothing but the browser would catch a mistake.

## Open questions for later parts

- **375px is still unverified**, across Parts 1, 2b, 2c and 3. `resize_window` reports
  success in this environment but the viewport does not change, so no agent has rendered a
  phone width. Build step 8 is the phone layout pass and is the right place to resolve it —
  but it needs a human at a browser, not another agent attempt.
- **The Google Fonts `@import`** in `globals.css` still means typography falls back to the
  system stack offline or on a LAN-only phone. Deferred to step 8, which already owns
  vendoring Quicksand and Inter locally.
