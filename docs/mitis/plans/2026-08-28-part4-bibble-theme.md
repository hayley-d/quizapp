# Part 4: the Bibble Theme Pass — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use mitis:subagent-driven-development (recommended) or mitis:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the Bibble theme across every screen, add a Light/Dark/System toggle, land the sparkle-burst and wing-flutter animations behind `prefers-reduced-motion`, and fix a WCAG failure on the app's primary button plus three Part 2c defects.

**Architecture:** Almost all tokens already exist in `globals.css`; this pass consumes them rather than designing a palette. New CSS is small — one opaque `--brand` token pair, two `@keyframes`, one reduced-motion media query. The sparkle burst is pure CSS on fixed positions so that "must not block the keyboard loop" is structural rather than something an implementer must be careful about. The `.dark` class on `<html>` stays the single source of truth for theme; the new toggle becomes another writer of it, never a second store.

**Tech Stack:** React 19, TypeScript, Tailwind v4 (`@theme inline` tokens), shadcn (vendored, not modified), `lucide-react`, `react-router-dom`, `sonner`. No new dependencies.

**User decisions (already made):**
- Scope is "Part 4, plus the cheap 2c defects" — the theme pass with the markdown-link guard, the card-row accessible names, and TypeScript `strict` folded in.
- Theme control is a **3-way Light / Dark / System toggle**, `localStorage`-persisted, with the `.dark` class kept as the single source of truth.
- Animations are **CSS keyframes with deterministic sparkle positions** — no canvas, no randomised particles, no per-frame JS.
- The streak counter is **client-side React state** and resets on reload; server-side derivation was rejected.
- **Every screen** is themed (`/stats` is an 8-line stub and is skipped).
- `SessionPage.tsx` **extracts its two terminal screens only**; a full `useSessionRunner` refactor was rejected as a refactor riding along with a visual pass.
- **No Zustand this slice.** Part 5's mock test is its intended home. Do not introduce a store here.

**Design record:** [`../specs/2026-08-28-part4-bibble-theme-design.md`](../specs/2026-08-28-part4-bibble-theme-design.md)

---

## Notes that apply to every task

- **All `cargo` commands run from the repo root**, never from `backend/`.
- **pnpm, not npm.** If a `package-lock.json` appears, delete it.
- **`frontend/src/components/ui/` is vendored shadcn.** Task 1 touches `button.tsx` for one token swap and nothing else. If any shadcn command offers to overwrite `globals.css`, decline.
- **CLAUDE.md rules are enforced here:** no comments in code, no abbreviated identifiers, no `any`. Prose files and the colour annotations in `globals.css` are exempt.
- **The type-check command is `tsc -b --noEmit`.** A bare `tsc --noEmit` reads a solution file with `"files": []` and exits 0 whatever the code says.
- **One implementer at a time.** Git's index is not per-file; two concurrent writers lost work in Part 2a.

---

## Task 1: The `--brand` token, and the contrast fix

**Goal:** Give the `brand` button an opaque token that clears WCAG AA in both themes, replacing the 70%-alpha `--deck-card-header` it currently borrows.

**Files:**
- Modify: `frontend/src/styles/globals.css`
- Modify: `frontend/src/components/ui/button.tsx:16`
- Already present: `frontend/scripts/check-contrast.py`

**Acceptance Criteria:**
- [ ] `python3 frontend/scripts/check-contrast.py` exits 0 and reports the brand button at 4.88:1 in both themes
- [ ] `--brand` and `--brand-foreground` are declared in `:root` and re-exported through `@theme inline` as `--color-brand` / `--color-brand-foreground`
- [ ] `button.tsx`'s `brand` variant references the new token and no longer references `--deck-card-header`
- [ ] `DeckCard.tsx` is unmodified — its band and chips look exactly as they do today

**Verify:** `python3 frontend/scripts/check-contrast.py && cd frontend && pnpm exec tsc -b --noEmit && pnpm build`

**Steps:**

- [ ] **Step 1: Confirm the failure exists before fixing it**

```bash
sed 's/^BRAND = (158, 84, 170)$/BRAND = (221, 153, 232)/' frontend/scripts/check-contrast.py > /tmp/mutated-contrast.py
python3 /tmp/mutated-contrast.py; echo "exit: $?"
```

Expected: two `FAIL` lines at `2.14:1`, exit 1. That is the colour light mode renders today. If this does not fail, the script is not measuring what you think it is — stop and investigate before continuing.

- [ ] **Step 2: Add the token to `:root` in `globals.css`**

Add these two lines to the `:root` block, after `--accent-foreground`:

```css
  --brand: rgb(158 84 170);                  /* orchid — every create/add action */
  --brand-foreground: oklch(1 0 0);
```

Declare them in `:root` only. They are deliberately theme-independent: an opaque colour's contrast does not depend on its backdrop, which is the whole reason this fixes the bug. Do **not** add a `.dark` override.

- [ ] **Step 3: Re-export through `@theme inline`**

Add to the `@theme inline` block, after the `--color-accent-foreground` line:

```css
  --color-brand: var(--brand);
  --color-brand-foreground: var(--brand-foreground);
```

- [ ] **Step 4: Point the button variant at it**

In `frontend/src/components/ui/button.tsx`, replace the `brand` variant line:

```ts
        brand: "bg-brand text-brand-foreground hover:brightness-110",
```

Leave the existing explanatory comment above it in place — it is pre-existing, and CLAUDE.md rule 1 forbids *adding* comments, not preserving them. Update its wording only if it now says something false.

- [ ] **Step 5: Verify the fix**

```bash
python3 frontend/scripts/check-contrast.py; echo "exit: $?"
```

Expected: four `ok` lines, brand button `4.88:1` in both themes, exit 0.

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm build
```

Expected: both succeed.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/styles/globals.css frontend/src/components/ui/button.tsx
git commit -m "Task 1: an opaque --brand token, fixing 2.14:1 white-on-orchid in light mode"
```

---

## Task 2: Motion tokens, keyframes, and the global reduced-motion rule

**Goal:** Add the CSS the animations need, plus a global `prefers-reduced-motion` rule that neutralises animation for anything a later part adds and forgets.

**Files:**
- Modify: `frontend/src/styles/globals.css`

**Acceptance Criteria:**
- [ ] `--sparkle` and `--streak` tokens declared per theme and re-exported through `@theme inline`
- [ ] `@keyframes sparkle-burst` and `@keyframes wing-flutter` defined
- [ ] `.sparkle` and `.wing-flutter` utility classes apply them
- [ ] A `@media (prefers-reduced-motion: reduce)` block reduces animation and transition durations to near-zero app-wide
- [ ] `pnpm build` succeeds and the CSS bundle contains both keyframe names

**Verify:** `cd frontend && pnpm build && grep -c "sparkle-burst\|wing-flutter" dist/assets/*.css`

**Steps:**

- [ ] **Step 1: Add the motion tokens**

In `globals.css`, add to `:root`:

```css
  --sparkle: oklch(0.86 0.16 88);            /* gold, matching --success */
  --streak: oklch(0.68 0.19 335);            /* magenta, matching --accent */
```

and to `.dark`:

```css
  --sparkle: oklch(0.90 0.16 90);
  --streak: oklch(0.74 0.20 335);
```

- [ ] **Step 2: Re-export them**

Add to `@theme inline`:

```css
  --color-sparkle: var(--sparkle);
  --color-streak: var(--streak);
```

- [ ] **Step 3: Add the keyframes and utility classes**

Append to `globals.css`, after the `.markdown` block:

```css
@keyframes sparkle-burst {
  0% {
    opacity: 0;
    transform: translate(0, 0) scale(0.2) rotate(0deg);
  }
  30% {
    opacity: 1;
  }
  100% {
    opacity: 0;
    transform: translate(var(--sparkle-x), var(--sparkle-y)) scale(1) rotate(140deg);
  }
}

@keyframes wing-flutter {
  0%, 100% {
    transform: rotate(0deg) scale(1);
  }
  20% {
    transform: rotate(-9deg) scale(1.08);
  }
  45% {
    transform: rotate(7deg) scale(1.04);
  }
  70% {
    transform: rotate(-4deg) scale(1.02);
  }
}

.sparkle {
  animation: sparkle-burst 700ms ease-out forwards;
}

.wing-flutter {
  animation: wing-flutter 900ms ease-in-out;
}
```

`--sparkle-x` and `--sparkle-y` are supplied per element as inline custom properties by `SparkleBurst` in Task 6. That is what makes eight identical elements fly in eight directions from one keyframe rule.

- [ ] **Step 4: Add the global reduced-motion rule**

Append:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

This is the safety net. It fails safe: anything animated in a later part is covered without the author remembering. It is **not** sufficient on its own for `useFlip`, which needs a different code path rather than a shorter duration — that is Task 3.

- [ ] **Step 5: Verify**

```bash
cd frontend && pnpm build && grep -c "sparkle-burst" dist/assets/*.css
```

Expected: build succeeds, grep reports at least 1.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/styles/globals.css
git commit -m "Task 2: motion tokens, the two keyframes, and a global reduced-motion rule"
```

---

## Task 3: `usePrefersReducedMotion`, and make `useFlip` reactive

**Goal:** Extract the codebase's one reduced-motion check into a shared, reactive hook and point `useFlip` at it.

**Files:**
- Create: `frontend/src/hooks/usePrefersReducedMotion.ts`
- Modify: `frontend/src/components/deck/useFlip.ts:42`

**Acceptance Criteria:**
- [ ] The hook subscribes to the media query and re-renders on change, rather than sampling it once
- [ ] `useFlip` no longer calls `window.matchMedia` directly
- [ ] `useFlip`'s reduced-motion path still swaps the face immediately rather than running a zero-duration rotation
- [ ] Flipping a card with reduced motion off still animates normally

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

**Steps:**

- [ ] **Step 1: Write the hook**

Create `frontend/src/hooks/usePrefersReducedMotion.ts`:

```ts
import { useEffect, useState } from 'react'

const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)'

export function usePrefersReducedMotion(): boolean {
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(
    () => window.matchMedia(REDUCED_MOTION_QUERY).matches,
  )

  useEffect(() => {
    const mediaQuery = window.matchMedia(REDUCED_MOTION_QUERY)
    function handleChange(changeEvent: MediaQueryListEvent) {
      setPrefersReducedMotion(changeEvent.matches)
    }
    mediaQuery.addEventListener('change', handleChange)
    return () => mediaQuery.removeEventListener('change', handleChange)
  }, [])

  return prefersReducedMotion
}
```

- [ ] **Step 2: Use it in `useFlip`**

In `frontend/src/components/deck/useFlip.ts`, add the import:

```ts
import { usePrefersReducedMotion } from '@/hooks/usePrefersReducedMotion'
```

Inside `useFlip()`, after the existing `useState` declarations, add:

```ts
  const prefersReducedMotion = usePrefersReducedMotion()
```

Then replace the inline check inside `goTo` — currently:

```ts
      if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
        setFace(next)
        return
      }
```

with:

```ts
      if (prefersReducedMotion) {
        setFace(next)
        return
      }
```

- [ ] **Step 3: Add the new value to `goTo`'s dependency array**

`goTo` is wrapped in `useCallback`. Its dependency array is currently `[face, later, nextFrame]`. It must become:

```ts
    [face, later, nextFrame, prefersReducedMotion],
```

Missing this is the one way to get this task wrong: `goTo` would close over a stale `prefersReducedMotion` and the hook's reactivity would be silently lost — the exact bug this task exists to fix. `oxlint`'s `react/rules-of-hooks` will not catch it.

- [ ] **Step 4: Verify**

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
```

Expected: all three succeed. `oxlint` may print pre-existing `set-state-in-effect` and `only-export-components` warnings; those are not introduced by this task and are not errors.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/hooks/usePrefersReducedMotion.ts frontend/src/components/deck/useFlip.ts
git commit -m "Task 3: a shared reduced-motion hook, and make useFlip react to it"
```

---

## Task 4: The theme toggle

**Goal:** A Light / Dark / System control in the header, persisted to `localStorage`, writing the same `.dark` class the app already treats as the source of truth.

**Files:**
- Create: `frontend/src/hooks/useTheme.ts`
- Create: `frontend/src/components/ThemeToggle.tsx`
- Modify: `frontend/src/components/AppShell.tsx`
- Modify: `frontend/index.html`

**Acceptance Criteria:**
- [ ] Three reachable states: Light, Dark, System
- [ ] The choice survives a reload
- [ ] With System selected, changing the OS theme changes the app live
- [ ] No flash of the wrong theme on first paint in any of the three states
- [ ] `components/ui/sonner.tsx` is unmodified and toasts still follow the theme
- [ ] The storage key is the same string in `index.html` and `useTheme.ts`

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build`

**Steps:**

- [ ] **Step 1: Write the hook**

Create `frontend/src/hooks/useTheme.ts`:

```ts
import { useCallback, useEffect, useState } from 'react'

export type ThemePreference = 'light' | 'dark' | 'system'

export const THEME_STORAGE_KEY = 'quizapp-theme'

const LIGHT_QUERY = '(prefers-color-scheme: light)'

function readStoredPreference(): ThemePreference {
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY)
  return stored === 'light' || stored === 'dark' ? stored : 'system'
}

function applyPreference(preference: ThemePreference) {
  const isLight =
    preference === 'light' ||
    (preference === 'system' && window.matchMedia(LIGHT_QUERY).matches)
  document.documentElement.classList.toggle('dark', !isLight)
}

export function useTheme() {
  const [preference, setPreferenceState] = useState<ThemePreference>(readStoredPreference)

  useEffect(() => {
    applyPreference(preference)
    if (preference !== 'system') return
    const mediaQuery = window.matchMedia(LIGHT_QUERY)
    function handleChange() {
      applyPreference('system')
    }
    mediaQuery.addEventListener('change', handleChange)
    return () => mediaQuery.removeEventListener('change', handleChange)
  }, [preference])

  const setPreference = useCallback((next: ThemePreference) => {
    if (next === 'system') {
      window.localStorage.removeItem(THEME_STORAGE_KEY)
    } else {
      window.localStorage.setItem(THEME_STORAGE_KEY, next)
    }
    setPreferenceState(next)
  }, [])

  return { preference, setPreference }
}
```

Storing `system` as *absent* rather than as the literal string keeps one representation of "no explicit choice" and makes the `index.html` fallback in Step 3 trivially correct.

- [ ] **Step 2: Write the toggle**

Create `frontend/src/components/ThemeToggle.tsx`:

```tsx
import { Monitor, Moon, Sun } from 'lucide-react'

import { useTheme, type ThemePreference } from '@/hooks/useTheme'
import { cn } from '@/lib/utils'

const THEME_OPTIONS: { value: ThemePreference; label: string; Icon: typeof Sun }[] = [
  { value: 'light', label: 'Light', Icon: Sun },
  { value: 'dark', label: 'Dark', Icon: Moon },
  { value: 'system', label: 'System', Icon: Monitor },
]

export function ThemeToggle() {
  const { preference, setPreference } = useTheme()

  return (
    <div
      role="radiogroup"
      aria-label="Colour theme"
      className="ml-auto flex items-center gap-0.5 rounded-lg bg-muted p-0.5"
    >
      {THEME_OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={preference === option.value}
          aria-label={option.label}
          title={option.label}
          onClick={() => setPreference(option.value)}
          className={cn(
            'rounded-md p-1.5 transition-colors',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
            preference === option.value
              ? 'bg-background text-foreground shadow-sm'
              : 'text-muted-foreground hover:text-foreground',
          )}
        >
          <option.Icon className="size-4" />
        </button>
      ))}
    </div>
  )
}
```

- [ ] **Step 3: Teach the inline script to read the stored preference**

In `frontend/index.html`, replace the body of the existing IIFE with:

```js
      (function () {
        var STORAGE_KEY = 'quizapp-theme'
        var media = window.matchMedia('(prefers-color-scheme: light)')
        function resolve() {
          var stored = null
          try {
            stored = window.localStorage.getItem(STORAGE_KEY)
          } catch (error) {
            stored = null
          }
          if (stored === 'light') return true
          if (stored === 'dark') return false
          return media.matches
        }
        function applyTheme() {
          document.documentElement.classList.toggle('dark', !resolve())
        }
        applyTheme()
        // No `addListener`/`removeListener` fallback: this app only targets
        // browsers new enough to already need `import.meta.dirname` (vite.config.ts).
        media.addEventListener('change', applyTheme)
      })()
```

Three things this must keep doing, all load-bearing:

1. **It stays inline and dependency-free.** It runs before first paint to prevent a flash of the wrong theme, and a module import cannot. This is the one piece of theme logic deliberately duplicated between `index.html` and `useTheme.ts`; a later tidying pass will want to remove it, and must not.
2. **`localStorage` access is wrapped in `try`/`catch`.** It throws outright in some privacy modes, and an exception here runs before React mounts — it would white-screen the whole app to save a theme preference.
3. **`STORAGE_KEY` must equal `THEME_STORAGE_KEY` in `useTheme.ts`.** They are two literals that must agree; if you change one, change the other.

- [ ] **Step 4: Mount it in the shell**

In `frontend/src/components/AppShell.tsx`, add the import:

```tsx
import { ThemeToggle } from '@/components/ThemeToggle'
```

and place `<ThemeToggle />` as the last child of the `<nav>`, after the `navigationLinks.map(...)` block. The component already carries `ml-auto`, so it pushes itself to the right of the nav links without any change to the nav's own classes.

- [ ] **Step 5: Verify**

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm build
```

Expected: both succeed. Live behaviour is checked in Task 10's walkthrough.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/hooks/useTheme.ts frontend/src/components/ThemeToggle.tsx frontend/src/components/AppShell.tsx frontend/index.html
git commit -m "Task 4: a Light/Dark/System toggle, with the .dark class still the source of truth"
```

---

## Task 5: Extract the runner's two terminal screens

**Goal:** Move the "Session finished" summary and the "Nothing left to practise" screen out of `SessionPage.tsx` into their own components, with no behaviour change.

**Files:**
- Create: `frontend/src/components/session/SessionSummary.tsx`
- Create: `frontend/src/components/session/SessionExhausted.tsx`
- Modify: `frontend/src/pages/SessionPage.tsx:258-315`

**Acceptance Criteria:**
- [ ] `SessionPage.tsx` drops by roughly 60 lines
- [ ] `formatDuration` moves to `SessionSummary.tsx` — it has no other caller
- [ ] Rendered markup is byte-identical to before; this task changes structure only
- [ ] No new props beyond what the extracted markup already reads

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

**Steps:**

- [ ] **Step 1: Create `SessionSummary.tsx`**

```tsx
import { Link } from 'react-router-dom'

import { Button } from '@/components/ui/button'
import type { SessionSummary as SessionSummaryData } from '@/lib/api'

function formatDuration(totalMilliseconds: number): string {
  const totalSeconds = Math.round(totalMilliseconds / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return minutes === 0 ? `${seconds}s` : `${minutes}m ${seconds}s`
}

type SessionSummaryProps = {
  summary: SessionSummaryData
}

export function SessionSummary({ summary }: SessionSummaryProps) {
  return (
    <div className="max-w-xl space-y-6">
      <h1 className="font-display text-2xl font-bold">Session finished</h1>
      <dl className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        <div>
          <dt className="text-sm text-muted-foreground">Answered</dt>
          <dd className="font-display text-2xl font-bold">{summary.answered_count}</dd>
        </div>
        <div>
          <dt className="text-sm text-muted-foreground">Correct</dt>
          <dd className="font-display text-2xl font-bold">{summary.correct_count}</dd>
        </div>
        <div>
          <dt className="text-sm text-muted-foreground">Accuracy</dt>
          <dd className="font-display text-2xl font-bold">
            {summary.accuracy === null ? '—' : `${Math.round(summary.accuracy * 100)}%`}
          </dd>
        </div>
        <div>
          <dt className="text-sm text-muted-foreground">Time</dt>
          <dd className="font-display text-2xl font-bold">
            {formatDuration(summary.total_ms)}
          </dd>
        </div>
      </dl>
      {summary.overridden_count > 0 && (
        <p className="text-sm text-muted-foreground">
          {summary.overridden_count} counted correct by override.
        </p>
      )}
      <div className="flex gap-3">
        <Button variant="brand" asChild className="h-10 px-6">
          <Link to="/study">Study again</Link>
        </Button>
        <Button variant="secondary" asChild className="h-10 px-6">
          <Link to="/decks">Back to decks</Link>
        </Button>
      </div>
    </div>
  )
}
```

The import is aliased (`SessionSummary as SessionSummaryData`) because the component and the API type would otherwise both be called `SessionSummary` in this file.

- [ ] **Step 2: Create `SessionExhausted.tsx`**

```tsx
import { Link } from 'react-router-dom'

import { Button } from '@/components/ui/button'

type SessionExhaustedProps = {
  message: string
}

export function SessionExhausted({ message }: SessionExhaustedProps) {
  return (
    <div className="max-w-xl space-y-4">
      <h1 className="font-display text-2xl font-bold">Nothing left to practise</h1>
      <p className="text-muted-foreground">{message}</p>
      <div className="flex gap-3">
        <Button variant="brand" asChild className="h-10 px-6">
          <Link to="/study">Back to study</Link>
        </Button>
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Use them in `SessionPage.tsx`**

Add the imports:

```tsx
import { SessionExhausted } from '@/components/session/SessionExhausted'
import { SessionSummary as SessionSummaryScreen } from '@/components/session/SessionSummary'
```

**The alias is required, not stylistic.** `SessionPage.tsx` already imports
`type SessionSummary` from `@/lib/api` (line 18) and uses it for the `summary` state. Importing
the component under its bare name collides with that type and fails the build.

Replace the whole `if (summary) { ... }` block (lines 258–301) with:

```tsx
  if (summary) return <SessionSummaryScreen summary={summary} />
```

Replace the whole `if (exhausted !== null || !card) { ... }` block (lines 303–315) with:

```tsx
  if (exhausted !== null || !card) {
    return <SessionExhausted message={exhausted ?? 'This session has no cards.'} />
  }
```

- [ ] **Step 4: Delete the now-unused `formatDuration`**

Remove the `formatDuration` function from `SessionPage.tsx` (lines 33–38). It moved to `SessionSummary.tsx` and has no other caller. `noUnusedLocals` is on, so leaving it behind fails the type-check — which is the check that this step was done.

- [ ] **Step 5: Verify**

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
wc -l src/pages/SessionPage.tsx
```

Expected: all pass; `SessionPage.tsx` is around 355 lines, down from 417.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/session/SessionSummary.tsx frontend/src/components/session/SessionExhausted.tsx frontend/src/pages/SessionPage.tsx
git commit -m "Task 5: extract the runner's summary and exhausted screens"
```

---

## Task 6: The sparkle burst and the streak

**Goal:** A gold sparkle burst over a correct verdict, and a magenta streak badge that flutters — neither able to delay the next card.

**Files:**
- Create: `frontend/src/components/session/SparkleBurst.tsx`
- Create: `frontend/src/components/session/StreakBadge.tsx`
- Modify: `frontend/src/pages/SessionPage.tsx`

**Acceptance Criteria:**
- [ ] Sparkles render only when the verdict is correct, including after an override
- [ ] The burst is `aria-hidden` and `pointer-events-none`
- [ ] `consecutiveCorrect` increments on a correct answer, resets to 0 on a wrong one
- [ ] An override restores the streak (design §6) and does not replay the burst
- [ ] The streak badge appears from 3 consecutive correct answers
- [ ] No `setTimeout`, `requestAnimationFrame`, or animation `await` is added to the answer or advance path

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

**Steps:**

- [ ] **Step 1: Write `SparkleBurst.tsx`**

```tsx
import type { CSSProperties } from 'react'

const SPARKLE_COUNT = 8
const BURST_RADIUS_PX = 46

type SparkleStyle = CSSProperties & {
  '--sparkle-x': string
  '--sparkle-y': string
}

const SPARKLES: SparkleStyle[] = Array.from({ length: SPARKLE_COUNT }, (_, sparkleIndex) => {
  const angleRadians = (sparkleIndex / SPARKLE_COUNT) * Math.PI * 2
  const distancePx = BURST_RADIUS_PX * (sparkleIndex % 2 === 0 ? 1 : 0.65)
  return {
    '--sparkle-x': `${Math.round(Math.cos(angleRadians) * distancePx)}px`,
    '--sparkle-y': `${Math.round(Math.sin(angleRadians) * distancePx)}px`,
    animationDelay: `${sparkleIndex * 25}ms`,
  }
})

export function SparkleBurst() {
  return (
    <div
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 overflow-visible"
    >
      {SPARKLES.map((sparkleStyle, sparkleIndex) => (
        <span
          key={sparkleIndex}
          style={sparkleStyle}
          className="sparkle absolute top-1/2 left-8 block size-2 rounded-full bg-sparkle"
        />
      ))}
    </div>
  )
}
```

`SPARKLES` is computed once at module load, not per render. The typed `SparkleStyle` intersection is how the custom properties are passed without an `as` cast — CLAUDE.md rule 3 forbids `as any` and `as unknown as T`, and this is the honest way to type a CSS custom property.

- [ ] **Step 2: Write `StreakBadge.tsx`**

```tsx
import { Sparkles } from 'lucide-react'

type StreakBadgeProps = {
  streak: number
}

export function StreakBadge({ streak }: StreakBadgeProps) {
  return (
    <span
      key={streak}
      className="wing-flutter inline-flex items-center gap-1 rounded-full bg-streak px-2.5 py-0.5 text-xs font-semibold text-accent-foreground"
    >
      <Sparkles className="size-3" />
      {streak} in a row
    </span>
  )
}
```

`key={streak}` is what makes the flutter replay. React remounts the element whenever the number changes, restarting the CSS animation; without it the animation runs once and never again. This is the whole mechanism — do not remove it as a redundant key.

- [ ] **Step 3: Add the streak state to `SessionPage.tsx`**

Add the imports:

```tsx
import { SparkleBurst } from '@/components/session/SparkleBurst'
import { StreakBadge } from '@/components/session/StreakBadge'
```

Add to the state declarations, after `const [overridden, setOverridden] = useState(false)`:

```tsx
  const [consecutiveCorrect, setConsecutiveCorrect] = useState(0)
```

Add the threshold constant beside `SELF_GRADES` at the top of the file:

```tsx
const STREAK_THRESHOLD = 3
```

- [ ] **Step 4: Update the counter where the verdict lands**

In `send()`, inside the `try` block, immediately after `setVerdict(result)`:

```tsx
      setConsecutiveCorrect((current) => (result.correct ? current + 1 : 0))
```

In `override()`, immediately after `setOverridden(true)`:

```tsx
      setConsecutiveCorrect((current) => current + 1)
```

This is design decision §6 made concrete: the override restores the streak, because `override()` already increments `correct_count` and a streak that ignored it would contradict the accuracy figure rendered inches away. It does **not** re-render `SparkleBurst`, because the burst is keyed off `verdict.correct` (Step 6) rather than off `overridden`.

- [ ] **Step 5: Render the badge in the header**

In the runner's `<header>`, between the stats `<p>` and the End session `<Button>`:

```tsx
        {consecutiveCorrect >= STREAK_THRESHOLD && (
          <StreakBadge streak={consecutiveCorrect} />
        )}
```

- [ ] **Step 6: Render the burst over the verdict**

Replace the `graded ? (<AnswerVerdict ... />) : (...)` opening so the verdict is wrapped:

```tsx
      {graded ? (
        <div className="relative">
          {verdict.correct && <SparkleBurst />}
          <AnswerVerdict
            verdict={verdict}
            overridden={overridden}
            overriding={overriding}
            onOverride={() => void override()}
            onNext={() => void loadNext()}
            nextButtonRef={nextButton}
          />
        </div>
      ) : (
```

`verdict.correct`, not the `correct || overridden` that `AnswerVerdict` computes internally: the burst reacts to the moment of answering, and an override happens after that moment has passed.

`relative` on the wrapper is required — `SparkleBurst` is `absolute inset-0` and would otherwise position against the page.

- [ ] **Step 7: Verify nothing was added to the advance path**

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
grep -n "setTimeout\|requestAnimationFrame\|await new Promise" src/pages/SessionPage.tsx src/components/session/*.tsx
```

Expected: the three commands pass, and the grep returns **nothing**. A hit means an animation has been given the chance to gate the keyboard loop, which is the one thing the spec forbids here.

- [ ] **Step 8: Commit**

```bash
git add frontend/src/components/session/SparkleBurst.tsx frontend/src/components/session/StreakBadge.tsx frontend/src/pages/SessionPage.tsx
git commit -m "Task 6: the sparkle burst and the streak badge, both CSS-only"
```

---

## Task 7: Theme the runner and the study picker

**Goal:** Bring `/session/:id` and `/study` onto the same surface vocabulary as `DeckCard`.

**Files:**
- Modify: `frontend/src/pages/SessionPage.tsx`
- Modify: `frontend/src/pages/StudyPage.tsx`
- Modify: `frontend/src/components/session/ChoiceList.tsx`
- Modify: `frontend/src/components/session/AnswerVerdict.tsx`
- Modify: `frontend/src/components/session/SessionSummary.tsx`

**Acceptance Criteria:**
- [ ] The runner's prompt sits on a rounded `bg-card` surface with a soft shadow, not on bare page background
- [ ] The summary's four figures sit on card surfaces
- [ ] `/study`'s mode and deck groups sit on card surfaces
- [ ] Every colour comes from a token — no raw hex, no `text-white` outside the vendored `ui/` directory
- [ ] Focus rings remain visible on every interactive element in both themes
- [ ] No behavioural change: the keyboard loop, the counts and the grading are untouched

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint && grep -rn "text-white\|#[0-9a-fA-F]\{3,6\}" src/pages src/components --include=*.tsx | grep -v "components/ui/"`

**Steps:**

- [ ] **Step 1: Give the runner's prompt a card surface**

In `SessionPage.tsx`, replace the prompt block:

```tsx
      <div className="space-y-4 rounded-xl border bg-card p-5 shadow-sm">
        <Markdown className="text-lg">{card.prompt_md}</Markdown>
        {card.image_path && <CardImage path={card.image_path} altText="Card image" />}
      </div>
```

- [ ] **Step 2: Lift the runner header**

Replace the stats paragraph's class with a card-surface strip:

```tsx
      <header className="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-card px-4 py-2.5 shadow-sm">
```

and change `items-baseline` to `items-center` as shown, so the streak badge aligns with the text rather than hanging below it.

- [ ] **Step 3: Give the flashcard reveal and self-grade buttons the theme**

Replace the revealed-answer `Markdown` class:

```tsx
            <Markdown className="rounded-xl border bg-card px-4 py-3 shadow-sm">
```

and give the four self-grade buttons a shared minimum width so they read as one control group rather than four differently-sized ones:

```tsx
                  className="min-w-24"
```

added to each `<Button>` in the `SELF_GRADES.map(...)`.

- [ ] **Step 4: Put the summary figures on surfaces**

In `SessionSummary.tsx`, give each of the four `<div>`s inside the `<dl>`:

```tsx
        <div className="rounded-xl border bg-card px-4 py-3 shadow-sm">
```

- [ ] **Step 5: Put the study picker on surfaces**

In `StudyPage.tsx`, wrap each `<section>`'s contents. Change the mode `RadioGroup`:

```tsx
        <RadioGroup value="practice" className="space-y-2 rounded-xl border bg-card p-4 shadow-sm">
```

and each deck group's `<ul>`:

```tsx
                <ul className="space-y-2 rounded-xl border bg-card p-4 shadow-sm">
```

- [ ] **Step 6: Verify no raw colours crept in**

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
grep -rn "text-white\|#[0-9a-fA-F]\{3,6\}" src/pages src/components --include=*.tsx | grep -v "components/ui/"
```

Expected: the first line passes. The grep should return **only** `DeckCard.tsx`'s existing `text-white` on its chips, which is pre-existing and sits on the measured 10.10:1 chip surface. Any new hit is a token you should have used instead.

- [ ] **Step 7: Commit**

```bash
git add frontend/src/pages/SessionPage.tsx frontend/src/pages/StudyPage.tsx frontend/src/components/session/
git commit -m "Task 7: theme the runner and the study picker onto card surfaces"
```

---

## Task 8: The lighter pass over the remaining screens

**Goal:** Bring `/decks`, `/decks/:id` and the card editor into the same vocabulary, without restyling `DeckCard` itself.

**Files:**
- Modify: `frontend/src/pages/DecksPage.tsx`
- Modify: `frontend/src/pages/DeckPage.tsx`
- Modify: `frontend/src/pages/CardEditorPage.tsx`

**Acceptance Criteria:**
- [ ] Page headings use `font-display` consistently with `/study` and the runner
- [ ] The card editor's form sits on a card surface rather than bare background
- [ ] `DeckCard.tsx` is **not** modified — it is the reference, and its contrast was measured in Task 1
- [ ] The `/decks` toolbar (search, filter, sort) remains fully keyboard-operable
- [ ] No behavioural change to search, filter, sort, drag-reorder or archive

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint && git diff --name-only | grep -c DeckCard.tsx`

**Steps:**

- [ ] **Step 1: Read the three files before editing**

```bash
cd frontend && wc -l src/pages/DecksPage.tsx src/pages/DeckPage.tsx src/pages/CardEditorPage.tsx
```

These are 169, 273 and 441 lines. Read each in full before changing it — this task is the one with the widest blast radius and the least prescriptive spec, because the markup varies.

- [ ] **Step 2: Give the card editor's form a surface**

In `CardEditorPage.tsx`, wrap the form fields in the same surface used elsewhere:

```tsx
      <div className="space-y-4 rounded-xl border bg-card p-5 shadow-sm">
```

Apply it to the main field group only. Do **not** wrap the action bar — it should stay visually outside the form surface, as it does now.

- [ ] **Step 3: Give the `/decks` toolbar a surface**

In `DecksPage.tsx`, give the search/filter/sort toolbar container:

```tsx
        className="flex flex-wrap items-center gap-3 rounded-xl border bg-card p-3 shadow-sm"
```

preserving whatever layout classes it already carries.

- [ ] **Step 4: Make the headings consistent**

Every page-level `<h1>` across the three files should read:

```tsx
className="font-display text-2xl font-bold"
```

matching `/study` and the runner. Section-level `<h2>` should read `font-display text-lg font-semibold`.

- [ ] **Step 5: Verify `DeckCard` was left alone**

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
git diff --name-only | grep DeckCard.tsx && echo "REGRESSION: DeckCard was modified" || echo "ok: DeckCard untouched"
```

Expected: the three commands pass and the message is `ok: DeckCard untouched`.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/pages/DecksPage.tsx frontend/src/pages/DeckPage.tsx frontend/src/pages/CardEditorPage.tsx
git commit -m "Task 8: the lighter theme pass over decks, deck detail and the editor"
```

---

## Task 9: The Part 2c card-row defects

**Goal:** Stop markdown links flipping the card, and give the card row real accessible names.

**Files:**
- Modify: `frontend/src/components/deck/CardRow.tsx:93`, `:155-156`

**Acceptance Criteria:**
- [ ] Clicking an `<a>` or `<button>` inside a card body does not flip the card
- [ ] Clicking the card body anywhere else still flips it
- [ ] A focused card announces the prompt text, not just "Show answer"
- [ ] The drag grip's accessible name contains no raw markdown syntax
- [ ] The keyboard flip path (Enter, Space) still works and still ignores events from children

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

**Steps:**

- [ ] **Step 1: Add a plain-text prompt for labelling**

`card.prompt_md` is markdown, so slicing it puts `$$` and `#` into the accessible name. Add above the `return` in `CardRow`:

```tsx
  const promptLabel = card.prompt_md
    .replace(/[#*_`~>$\\[\]()!-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 80)
```

This is deliberately crude. It is a screen-reader label, not a rendering path — `Markdown.tsx` remains the app's one renderer, and running markdown through a parser to build an `aria-label` would be the second renderer the 2a/2b split exists to prevent.

- [ ] **Step 2: Fix the drag grip's name**

Replace line 93:

```tsx
        aria-label={`Reorder ${promptLabel}`}
```

- [ ] **Step 3: Guard the flip target's `onClick`**

`onClick={flip}` passes the event straight to `flip()`, which takes no arguments — so this needs a wrapper, not just an inserted line. Replace line 156:

```tsx
            onClick={(clickEvent) => {
              if ((clickEvent.target as HTMLElement).closest('a,button')) return
              flip()
            }}
```

- [ ] **Step 4: Give the flip target a real accessible name**

Replace the `aria-label` on line 155 with a `title`-plus-label pairing, so the announced name carries both the card and the action:

```tsx
            aria-label={`${showingAnswer ? 'Show question' : 'Show answer'}: ${promptLabel}`}
```

- [ ] **Step 5: Verify**

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
```

Expected: all pass. The live click behaviour is checked in Task 10's walkthrough, point 6.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/deck/CardRow.tsx
git commit -m "Task 9: stop markdown links flipping cards, and fix the row's accessible names"
```

---

## Task 10: Turn on `strict`, enforce rule 3, and put lint in the gate

**Goal:** Make two CLAUDE.md rules mechanically enforced instead of prose-only, and add the checks that prove it to the gate.

**Files:**
- Modify: `frontend/tsconfig.app.json`
- Modify: `frontend/.oxlintrc.json`
- Modify: `docs/HANDOVER.md`

**Acceptance Criteria:**
- [ ] `"strict": true` is set and `pnpm exec tsc -b --noEmit` still passes
- [ ] `"typescript/no-explicit-any": "error"` is set and `pnpm exec oxlint` reports no errors
- [ ] `docs/HANDOVER.md`'s verification gate section lists `pnpm exec oxlint` and `python3 frontend/scripts/check-contrast.py`
- [ ] Both config changes are proven to be load-bearing, not no-ops

**Verify:** `cd frontend && pnpm exec tsc -b --noEmit && pnpm exec oxlint && cd .. && python3 frontend/scripts/check-contrast.py`

**Steps:**

- [ ] **Step 1: Turn on `strict`**

In `frontend/tsconfig.app.json`, add to `compilerOptions`, above the `/* Linting */` group:

```json
    "strict": true,
```

- [ ] **Step 2: Add the lint rule**

In `frontend/.oxlintrc.json`, add to `rules`:

```json
    "typescript/no-explicit-any": "error"
```

- [ ] **Step 3: Prove both are load-bearing**

A config flag that changes no outcome is worse than none — it reads as protection that is not there. Prove each independently, one change at a time:

```bash
cd frontend
printf 'const brokenOnPurpose: string = 1\nexport default brokenOnPurpose\n' > src/strict-probe.ts
pnpm exec tsc -b --noEmit; echo "expect non-zero: $?"
rm src/strict-probe.ts

printf 'export function probe(value: any) { return value }\n' > src/any-probe.ts
pnpm exec oxlint src/any-probe.ts; echo "expect non-zero: $?"
rm src/any-probe.ts
```

Expected: each probe fails while present, and both files are removed afterwards. Confirm `git status` is clean of them before committing.

- [ ] **Step 4: Confirm the real tree still passes**

```bash
cd frontend && pnpm exec tsc -b --noEmit && pnpm exec oxlint
```

Expected: both pass. `oxlint` will still print the pre-existing `set-state-in-effect` and `only-export-components` **warnings**; those are warnings, not errors, and are out of scope.

- [ ] **Step 5: Update the gate in `docs/HANDOVER.md`**

Replace the fenced command block under "## The verification gate" with:

```bash
cargo test
cargo clippy --all-targets -- -D warnings        # --all-targets matters, see below
SQLX_OFFLINE=true cargo build
python3 frontend/scripts/check-contrast.py
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
```

Then add this paragraph immediately after that block:

> **`pnpm exec oxlint` and the contrast script joined the gate in Part 4.** The lint run is
> what makes CLAUDE.md rule 3 (never use `any`) mechanically enforced rather than prose —
> adding the rule without running it in the gate would have changed nothing. The contrast
> script proves the `brand` button clears WCAG AA in both themes; it caught a 2.14:1
> white-on-orchid failure in light mode that had survived three parts unnoticed, because
> until Part 4 there was no way to switch themes without visiting System Settings.

- [ ] **Step 6: Commit**

```bash
git add frontend/tsconfig.app.json frontend/.oxlintrc.json docs/HANDOVER.md
git commit -m "Task 10: strict on, no-explicit-any enforced, lint and contrast in the gate"
```

---

## Task 11: Full gate, browser walkthrough, and handover

**Goal:** Run the complete gate, drive every Part 4 behaviour in a real browser, and record honestly what was and was not observed.

> **USER-ORDERED GATE — NON-SKIPPABLE.** This task was requested by the user in the current conversation. It MUST NOT be closed by walking around it, by declaring it "verified inline", or by substituting a cheaper check. Close only after every item in `acceptanceCriteria` has been re-validated independently, with output captured.

**Files:**
- Modify: `docs/HANDOVER.md`

**Acceptance Criteria:**
- [ ] Every gate command exits 0, with output captured
- [ ] Theme toggle: all three states reached, choice survives a reload, System follows a live OS change, and no flash of the wrong theme on first paint
- [ ] The `brand` button ("Start practising") is legible in **light** mode — the regression this pass exists to fix
- [ ] A correct answer bursts sparkles; holding Enter through five cards is no slower than before
- [ ] Three consecutive correct answers raise the streak badge and it flutters; a wrong answer resets it
- [ ] An override restores the streak without replaying the burst
- [ ] With OS Reduce Motion on: no sparkle animation, no flutter, and the deck card flip swaps faces without rotating
- [ ] A markdown link inside a card navigates and does **not** flip the card
- [ ] KaTeX renders legibly in both palettes
- [ ] `docs/HANDOVER.md` records what was driven **and** what was not

**Verify:** `cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build && python3 frontend/scripts/check-contrast.py && cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint`

**Steps:**

- [ ] **Step 1: Run the full gate and capture the output**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
python3 frontend/scripts/check-contrast.py
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
```

Expected: every command exits 0; `cargo test` reports 119 passing. Paste the real output into the execution ledger — do not summarise it as "gate green".

- [ ] **Step 2: Start both servers the way that actually works here**

```bash
export SQLX_OFFLINE=true
cargo run &
cd frontend && pnpm dev --host 0.0.0.0
```

Then navigate to the **Network** address vite prints (e.g. `http://192.168.2.161:5273`), **not** `localhost`. `pnpm dev` without `--host` binds only to `localhost`, which resolves to IPv6 `::1`, and Chrome asks for IPv4. This combination is what unblocked Part 3's walkthrough after four parts of "the browser could not reach the dev server".

- [ ] **Step 3: Drive the theme toggle**

Cycle Light → Dark → System on `/decks`. Reload in each. Then, with System selected, change the OS appearance and confirm the app follows live.

Look specifically at the **"Start practising" button on `/study` in light mode.** That is the 2.14:1 failure this pass fixes; it should now be a solid orchid with legible white text.

- [ ] **Step 4: Drive the runner**

Create a deck with at least one card of each kind, then run a session:

- Answer correctly — sparkles burst over the verdict.
- Hold Enter through five cards — the loop is no slower than before.
- Get three right in a row — the badge appears and flutters. Get one wrong — it resets.
- Get one wrong, press "I was right" — the streak comes back, the burst does **not** replay.

- [ ] **Step 5: Drive reduced motion**

System Settings → Accessibility → Display → **Reduce motion on**. Then:

- Answer correctly: sparkles do not animate.
- Build a streak: the badge appears without fluttering.
- Flip a card on `/decks/:id`: the face swaps with no rotation and does not strand mid-flip at 90°.

That last point is why Task 3 exists — the global CSS rule alone would leave the card edge-on.

- [ ] **Step 6: Check the Part 2c fix**

On `/decks/:id`, author a card whose prompt contains a markdown link, then click the link. It must navigate and the card must **not** flip. Click elsewhere in the body — it must still flip.

- [ ] **Step 7: Record the outcome honestly in `docs/HANDOVER.md`**

Update the **Last updated** line, replace "Next up" with Part 5, and add a Part 4 section to Outstanding recording what was driven.

**Two things must be written down as NOT done, because they cannot be done here:**

1. **375px phone width.** `resize_window` reports success in this environment but the viewport does not change. Do not claim it. It is now outstanding across Parts 1, 2b, 2c, 3 and 4, and belongs to build step 8's phone layout pass — with a human at a browser.
2. **Anything not actually clicked.** Part 2c's nine-point walkthrough is still outstanding unless it was genuinely driven in this session. If it was, say so; if it was not, leave it on the list. The handover's value is that it has never yet claimed a walkthrough that did not happen.

- [ ] **Step 8: Commit and merge**

```bash
git add docs/HANDOVER.md
git commit -m "Task 11: full gate, browser walkthrough, handover"
git checkout main && git merge --no-ff feat/part4-bibble-theme -m "Merge Part 4: the Bibble theme pass"
```

```json:metadata
{"userGate": true, "tags": ["user-gate"], "modelTier": "standard"}
```

---

## Dependencies

```
Task 1 ──┬─> Task 7 ──> Task 8
Task 2 ──┴─> Task 6
Task 3 ──> Task 4
Task 5 ──> Task 6
Task 9 (independent)
Task 10 (independent)
Tasks 1-10 ──> Task 11
```

Task 5 must land before Task 6: both edit `SessionPage.tsx`, and doing the extraction after the sparkle work means re-doing it.
