# Decks Search, Module Filter and Date Sort — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `mitis:subagent-driven-development`
> (recommended) or `mitis:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking. Tasks:
> `docs/mitis/plans/2026-08-26-decks-search-filter-sort.md.tasks.json`

## Context

Part 1 shipped a `/decks` screen that groups decks into a `<section>` per module. With a
handful of decks that reads fine; at twenty-plus it becomes a long scroll where you cannot
see everything at once, and there is no way to find a deck by name.

This replaces the grouping with a flat list of deck cards, each carrying a module badge,
plus a toolbar of search / module filter / date sort. It builds on `part1-schema-modules-decks`
(Part 1 is complete, reviewed and merge-ready at 31 tests; the user chose to extend the
branch rather than merge first).

**Goal:** Find any deck quickly — by name, by module, or by recency — from one flat list.

**Architecture:** Server-side. `GET /api/decks` gains `q`, `sort` and an extended
`module_id`, implemented as ONE parameterized `query_as!` rather than a query per option
combination. The page fetches on every criteria change, debounced for typing.

**Tech Stack:** unchanged — axum, sqlx (compile-time macros, offline cache at the root
`.sqlx/`), React 19, Vite, Tailwind v4, shadcn/ui, pnpm.

**User decisions (already made):**
- Search matches **deck name only**, case-insensitive.
- Sort is **date only**: newest first (default) / oldest first. No name sort.
- Module filter is a **single dropdown**: All modules (default) / each module / No module.
- Search, filter and sort all run **server-side**.
- Flat list, no module sections. Module rendered as a **badge on each deck card**.

---

## Why one query and not twelve

`query_as!` needs a literal SQL string, so the obvious implementation branches per
combination: 3 module cases × 2 sort directions × 2 search states = 12 hand-written
queries. That is how the endpoint rots. Instead, one literal query takes the criteria as
parameters and lets SQL do the branching:

```sql
WHERE (? IS NULL OR d.name LIKE '%' || ? || '%' ESCAPE '\')
  AND (? = 'all'
       OR (? = 'none' AND d.module_id IS NULL)
       OR d.module_id = ?)
ORDER BY CASE WHEN ? = 'oldest' THEN d.created_at END ASC,
         CASE WHEN ? = 'newest' THEN d.created_at END DESC,
         d.id DESC
```

Four facts that make this correct, each of which is a bug if missed:

1. **Use plain `?` placeholders, repeated — not `?1`/`?2`.** The sqlx macro counts
   parameters by occurrence, so numbered placeholders confuse it. Bind the same value
   twice where it appears twice. Order is: `q, q, mode, mode, module_id, sort, sort`.
2. **`created_at` is now ISO-8601 `...Z` TEXT** (changed in Part 1's fix wave), which still
   sorts lexicographically as chronological. No date parsing anywhere.
3. **`d.id DESC` is a mandatory tiebreak.** Timestamps have one-second resolution, so two
   decks created in the same second have no deterministic order — a date-ordering test
   without this tiebreak flakes intermittently, which is worse than failing.
4. **`%` and `_` must be escaped in `q`** before binding, with `ESCAPE '\'` in the SQL, or
   a search for `100%` matches every deck.

SQLite's `LIKE` is already case-insensitive for ASCII, so no `COLLATE` is involved here —
and therefore, unlike the name-ordering tests, no collation-discriminating inputs are
needed. (See the Part 1 plan's convention note on that trap.)

---

## Task 1: Backend — `q`, `sort`, and `module_id=all` on `GET /api/decks`

**Goal:** `GET /api/decks?q=&module_id=&sort=` filters by name substring, filters by module,
and orders by `created_at` in either direction, in one macro-checked query.

**Files:**
- Modify: `backend/src/routes/decks.rs`
- Test: `backend/tests/decks.rs`
- Regenerate: `.sqlx/` (root)

**Acceptance Criteria:**
- [ ] `?q=kine` matches "Chapter 1 - Kinematics" case-insensitively; absent or empty `q`
      applies no name filter
- [ ] `q` does NOT match against `description` (name-only was the explicit decision)
- [ ] `?q=100%` is treated literally — it does not behave as a wildcard
- [ ] `?sort=oldest` returns the exact reverse of `?sort=newest`; absent `sort` == `newest`
- [ ] Decks created within the same second still order deterministically (`id` tiebreak)
- [ ] `?module_id=all` and an absent `module_id` behave identically; `none` and `<n>` keep
      their Part 1 behaviour
- [ ] `?sort=sideways` → 422 with a `sort` field error (NOT a silent fallback)
- [ ] `?module_id=abc` → 422 with a `module_id` field error (unchanged from Part 1)
- [ ] All three criteria combine correctly — filter ∩ search, correctly ordered
- [ ] The three existing list queries are replaced by ONE `query_as!`

**Verify:** `cargo test --test decks` → all pass;
`cargo clippy --all-targets -- -D warnings` → clean

**Steps:**

- [ ] **Step 1: Write the failing tests** in `backend/tests/decks.rs`

```rust
#[tokio::test]
async fn search_matches_name_case_insensitively() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "Chapter 1 - Kinematics"})).await;
    app.post("/api/decks", json!({"name": "Thermodynamics"})).await;

    let (status, list) = app.get("/api/decks?q=kine").await;
    assert_eq!(status, StatusCode::OK);
    let names = names_of(&list);
    assert_eq!(names, vec!["Chapter 1 - Kinematics"]);

    // Empty q applies no filter.
    let (_, all) = app.get("/api/decks?q=").await;
    assert_eq!(all.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn search_does_not_match_description() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "Deck One", "description": "clustering"})).await;

    let (_, list) = app.get("/api/decks?q=clustering").await;
    assert_eq!(list.as_array().unwrap().len(), 0, "q must match name only");
}

#[tokio::test]
async fn search_treats_wildcards_literally() {
    let app = common::spawn_app().await;
    app.post("/api/decks", json!({"name": "Scored 100% overall"})).await;
    app.post("/api/decks", json!({"name": "Unrelated"})).await;

    let (_, pct) = app.get("/api/decks?q=100%25").await; // %25 is an encoded '%'
    assert_eq!(names_of(&pct), vec!["Scored 100% overall"]);

    // A bare '%' must not behave as "match everything".
    let (_, underscore) = app.get("/api/decks?q=_nrelated").await;
    assert_eq!(underscore.as_array().unwrap().len(), 0, "_ must be literal");
}

#[tokio::test]
async fn sort_newest_is_default_and_oldest_reverses_it() {
    let app = common::spawn_app().await;
    // Same-second creation is the normal case here, so this also exercises the id tiebreak.
    app.post("/api/decks", json!({"name": "First"})).await;
    app.post("/api/decks", json!({"name": "Second"})).await;
    app.post("/api/decks", json!({"name": "Third"})).await;

    let (_, default) = app.get("/api/decks").await;
    assert_eq!(names_of(&default), vec!["Third", "Second", "First"]);

    let (_, newest) = app.get("/api/decks?sort=newest").await;
    assert_eq!(names_of(&newest), names_of(&default), "absent sort == newest");

    let (_, oldest) = app.get("/api/decks?sort=oldest").await;
    assert_eq!(names_of(&oldest), vec!["First", "Second", "Third"]);
}

#[tokio::test]
async fn unknown_sort_is_422_with_field_error() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/decks?sort=sideways").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"][0]["field"], "sort");
}

#[tokio::test]
async fn module_all_equals_absent() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "In module"})).await;
    app.post("/api/decks", json!({"name": "Loose"})).await;

    let (_, absent) = app.get("/api/decks").await;
    let (_, all) = app.get("/api/decks?module_id=all").await;
    assert_eq!(names_of(&absent), names_of(&all));
    assert_eq!(all.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn criteria_combine() {
    let app = common::spawn_app().await;
    let mid = module(&app, "COS781").await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Alpha test"})).await;
    app.post("/api/decks", json!({"module_id": mid, "name": "Beta test"})).await;
    app.post("/api/decks", json!({"name": "Alpha loose"})).await;

    let (_, list) = app
        .get(&format!("/api/decks?q=alpha&module_id={mid}&sort=oldest"))
        .await;
    assert_eq!(names_of(&list), vec!["Alpha test"]);
}
```

Add this helper near the top of `backend/tests/decks.rs`:

```rust
fn names_of(list: &serde_json::Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect()
}
```

- [ ] **Step 2: Run — expect failure**

Run: `cargo test --test decks` → the new tests FAIL (`q` and `sort` are ignored today, so
ordering and filtering assertions come back wrong).

- [ ] **Step 3: Extend the query struct and add the LIKE escaper** in
      `backend/src/routes/decks.rs`

```rust
#[derive(Deserialize)]
pub struct ListQuery {
    /// Numeric module id, the literal "none" for unparented decks, or "all"/absent.
    pub module_id: Option<String>,
    /// Case-insensitive substring match on the deck NAME only.
    pub q: Option<String>,
    /// "newest" (default) or "oldest", by created_at.
    pub sort: Option<String>,
}

/// Escapes LIKE metacharacters so a user searching for "100%" does not match everything.
/// Pairs with `ESCAPE '\'` in the SQL.
fn escape_like(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
```

- [ ] **Step 4: Replace the three list queries with one** — delete the whole
      `match q.module_id.as_deref()` block in `list` and use this:

```rust
async fn list(
    State(st): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<DeckDto>>> {
    // An empty q means "no filter", same as absent — an empty search box must not
    // filter everything out.
    let needle = q
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(escape_like);

    let sort = q.sort.as_deref().unwrap_or("newest").to_string();
    if sort != "newest" && sort != "oldest" {
        return Err(AppError::validation([(
            "sort",
            "sort must be \"newest\" or \"oldest\"",
        )]));
    }

    // mode selects the module branch; module_id is only consulted when mode is "id".
    let (mode, module_id) = match q.module_id.as_deref() {
        None | Some("") | Some("all") => ("all".to_string(), None),
        Some("none") => ("none".to_string(), None),
        Some(raw) => {
            let mid: i64 = raw.parse().map_err(|_| {
                AppError::validation([(
                    "module_id",
                    "module_id must be a number, \"none\" or \"all\"",
                )])
            })?;
            ("id".to_string(), Some(mid))
        }
    };

    let rows = sqlx::query_as!(
        DeckDto,
        r#"SELECT d.id AS "id!: i64",
                  d.module_id AS "module_id?: i64",
                  m.name      AS "module_name?: String",
                  d.name, d.description, d.created_at,
                  (SELECT COUNT(*) FROM cards c
                    WHERE c.deck_id = d.id AND c.archived = 0) AS "card_count!: i64"
           FROM decks d
           LEFT JOIN modules m ON m.id = d.module_id
           WHERE (? IS NULL OR d.name LIKE '%' || ? || '%' ESCAPE '\')
             AND (? = 'all'
                  OR (? = 'none' AND d.module_id IS NULL)
                  OR d.module_id = ?)
           ORDER BY CASE WHEN ? = 'oldest' THEN d.created_at END ASC,
                    CASE WHEN ? = 'newest' THEN d.created_at END DESC,
                    d.id DESC"#,
        needle,
        needle,
        mode,
        mode,
        module_id,
        sort,
        sort
    )
    .fetch_all(&st.pool)
    .await?;

    Ok(Json(rows))
}
```

Note the bind order matches the `?` order exactly: `needle, needle, mode, mode,
module_id, sort, sort`. If sqlx complains about a nullability annotation, adjust the
annotation — do not fall back to a runtime query.

- [ ] **Step 5: Delete the now-dead ordering test**

`unfiltered_list_orders_by_module_then_deck_name_case_insensitively` asserts the OLD
module-then-name ordering, which this task deliberately replaces with date ordering.
Delete it — leaving it would either fail or, worse, lock a contract no consumer reads.
Its collation coverage is not lost: `modules.rs`'s
`list_is_ordered_by_name_case_insensitively` still proves `COLLATE NOCASE` on module names.

- [ ] **Step 6: Regenerate the offline cache and run**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export DATABASE_URL="sqlite://data/quizapp.db?mode=rwc"
cargo sqlx prepare --workspace
cargo test                                  # expect the full suite green
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
```

- [ ] **Step 7: Commit**

```bash
git add backend/src/routes/decks.rs backend/tests/decks.rs .sqlx
git commit -m "feat(api): search, module filter and date sort on GET /api/decks"
```

```json:metadata
{"files":["backend/src/routes/decks.rs","backend/tests/decks.rs",".sqlx"],"verifyCommand":"cargo test --test decks && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build","acceptanceCriteria":["q matches name substring case-insensitively; empty or absent q applies no filter","q does not match description","q treats % and _ literally","sort=oldest reverses sort=newest; absent sort equals newest","same-second decks order deterministically via the id tiebreak","module_id=all equals absent; none and numeric keep Part 1 behaviour","sort=sideways returns 422 with a sort field error","module_id=abc returns 422 with a module_id field error","criteria combine correctly","exactly one query_as! replaces the previous three"],"modelTier":"standard"}
```

---

## Task 2: Frontend — toolbar, flat list, module badge

**Goal:** `/decks` renders one flat, searchable, filterable, sortable list of deck cards,
each showing its module as a badge.

**Files:**
- Modify: `frontend/src/lib/api.ts`, `frontend/src/pages/DecksPage.tsx`
- Add: `frontend/src/components/ui/badge.tsx` (via the shadcn CLI)

**Acceptance Criteria:**
- [ ] No module `<section>` grouping remains — one flat `grid` of deck cards
- [ ] Each card shows a module badge, or a muted "No module" badge when unparented
- [ ] Clicking a card's module badge sets the module filter to that module
- [ ] Toolbar: search input, module `Select` (All modules / each module / No module), sort
      `Select` (Newest first / Oldest first); stacks vertically below `sm:`
- [ ] Typing is debounced ~250ms — not one request per keystroke
- [ ] A slow earlier response cannot overwrite a newer one (no flicker back to stale results)
- [ ] Two distinct empty states: "no decks yet" vs "no decks match", the latter offering to
      clear the filters
- [ ] `pnpm exec tsc --noEmit` and `pnpm build` both clean
- [ ] Editing/creating a deck still works and the list reflects the change

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build`, then a browser pass at
`http://localhost:5273/decks`

**Steps:**

- [ ] **Step 1: Add the Badge primitive**

```bash
cd frontend && pnpm dlx shadcn@latest add badge
```

Decline any offer to overwrite `src/styles/globals.css` — it carries the verified Bibble
palette. If it overwrites it anyway, restore with
`git checkout -- frontend/src/styles/globals.css`.

- [ ] **Step 2: Extend the API client** in `frontend/src/lib/api.ts`

```ts
export type DeckSort = 'newest' | 'oldest'
/** 'all' | 'none' | a module id */
export type ModuleFilter = 'all' | 'none' | number

export type DeckQuery = {
  q?: string
  moduleId?: ModuleFilter
  sort?: DeckSort
}

function deckQueryString({ q, moduleId, sort }: DeckQuery): string {
  const params = new URLSearchParams()
  if (q && q.trim() !== '') params.set('q', q.trim())
  if (moduleId !== undefined && moduleId !== 'all') params.set('module_id', String(moduleId))
  if (sort) params.set('sort', sort)
  const s = params.toString()
  return s === '' ? '' : `?${s}`
}
```

Then change `listDecks` to take the query and accept an `AbortSignal`:

```ts
  listDecks: (query: DeckQuery = {}, signal?: AbortSignal) =>
    request<Deck[]>('GET', `/decks${deckQueryString(query)}`, undefined, signal),
```

and thread `signal` through `request` — add it as a fourth parameter and pass it to
`fetch(..., { signal })`. An aborted fetch throws `AbortError`; `request` must let that
propagate untouched so the caller can ignore it (see Step 3).

- [ ] **Step 3: Rewrite `DecksPage`** — the substantive change. Delete the `groups`
      `useMemo` and the per-module `<section>` rendering entirely.

```tsx
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { api, type Deck, type DeckSort, type Module, type ModuleFilter } from '@/lib/api'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { ModuleDialog } from '@/components/ModuleDialog'
import { DeckDialog } from '@/components/DeckDialog'

const ALL = 'all'
const NONE = 'none'

export function DecksPage() {
  const [modules, setModules] = useState<Module[]>([])
  const [decks, setDecks] = useState<Deck[]>([])
  const [editing, setEditing] = useState<Deck | 'new' | null>(null)

  // `search` is what the user is typing; `debounced` is what we actually query with.
  const [search, setSearch] = useState('')
  const [debounced, setDebounced] = useState('')
  const [moduleFilter, setModuleFilter] = useState<ModuleFilter>(ALL)
  const [sort, setSort] = useState<DeckSort>('newest')
  const [loading, setLoading] = useState(false)

  const filtersActive = debounced.trim() !== '' || moduleFilter !== ALL

  useEffect(() => {
    const t = setTimeout(() => setDebounced(search), 250)
    return () => clearTimeout(t)
  }, [search])

  const loadModules = useCallback(async () => {
    try {
      setModules(await api.listModules())
    } catch {
      toast.error('Could not load modules')
    }
  }, [])

  useEffect(() => { void loadModules() }, [loadModules])

  // One in-flight deck request at a time. Aborting the previous one is what stops a
  // slow earlier response from overwriting a newer one.
  const inFlight = useRef<AbortController | null>(null)

  const loadDecks = useCallback(async () => {
    inFlight.current?.abort()
    const controller = new AbortController()
    inFlight.current = controller
    setLoading(true)
    try {
      const rows = await api.listDecks(
        { q: debounced, moduleId: moduleFilter, sort },
        controller.signal,
      )
      setDecks(rows)
    } catch (e) {
      if ((e as Error)?.name === 'AbortError') return   // superseded; not an error
      toast.error('Could not load decks')
    } finally {
      if (inFlight.current === controller) setLoading(false)
    }
  }, [debounced, moduleFilter, sort])

  useEffect(() => { void loadDecks() }, [loadDecks])

  function clearFilters() {
    setSearch('')
    setDebounced('')
    setModuleFilter(ALL)
  }

  const moduleName = (m: ModuleFilter) =>
    m === ALL ? 'All modules'
      : m === NONE ? 'No module'
        : (modules.find((x) => x.id === m)?.name ?? 'Unknown module')

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-display text-2xl font-bold">Decks</h1>
        <div className="flex gap-2">
          <ModuleDialog onSaved={() => { void loadModules(); void loadDecks() }} />
          <Button onClick={() => setEditing('new')}>New deck</Button>
        </div>
      </div>

      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <Input
          className="sm:max-w-xs"
          placeholder="Search deck names…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <Select
          value={String(moduleFilter)}
          onValueChange={(v) =>
            setModuleFilter(v === ALL || v === NONE ? (v as ModuleFilter) : Number(v))
          }
        >
          <SelectTrigger className="sm:w-52"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value={ALL}>All modules</SelectItem>
            <SelectItem value={NONE}>No module</SelectItem>
            {modules.map((m) => (
              <SelectItem key={m.id} value={String(m.id)}>{m.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={sort} onValueChange={(v) => setSort(v as DeckSort)}>
          <SelectTrigger className="sm:w-44"><SelectValue /></SelectTrigger>
          <SelectContent>
            <SelectItem value="newest">Newest first</SelectItem>
            <SelectItem value="oldest">Oldest first</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {decks.length === 0 && !loading && (
        filtersActive ? (
          <div className="space-y-2">
            <p className="text-muted-foreground">
              No decks match “{debounced}” in {moduleName(moduleFilter)}.
            </p>
            <Button variant="secondary" size="sm" onClick={clearFilters}>
              Clear filters
            </Button>
          </div>
        ) : (
          <p className="text-muted-foreground">
            No decks yet. Create a module (e.g. COS781), then a deck for each test.
          </p>
        )
      )}

      <div className="grid gap-3 sm:grid-cols-2">
        {decks.map((d) => (
          <Card key={d.id}>
            <CardHeader className="flex flex-row items-start justify-between gap-2">
              <div className="space-y-1">
                <CardTitle className="font-display text-base">{d.name}</CardTitle>
                <div className="flex items-center gap-2">
                  {d.module_id === null ? (
                    <Badge variant="outline" className="text-muted-foreground">
                      No module
                    </Badge>
                  ) : (
                    <button
                      type="button"
                      onClick={() => setModuleFilter(d.module_id as number)}
                      title={`Filter by ${d.module_name}`}
                    >
                      <Badge variant="secondary">{d.module_name}</Badge>
                    </button>
                  )}
                  <span className="text-sm text-muted-foreground">
                    {d.card_count} card{d.card_count === 1 ? '' : 's'}
                  </span>
                </div>
              </div>
              <Button variant="ghost" size="sm" onClick={() => setEditing(d)}>
                Edit
              </Button>
            </CardHeader>
            {d.description && (
              <CardContent className="text-sm text-muted-foreground">
                {d.description}
              </CardContent>
            )}
          </Card>
        ))}
      </div>

      {editing && (
        <DeckDialog
          key={editing === 'new' ? 'new' : editing.id}
          modules={modules}
          deck={editing === 'new' ? undefined : editing}
          open
          onOpenChange={(o) => { if (!o) setEditing(null) }}
          onSaved={() => { void loadDecks() }}
        />
      )}
    </div>
  )
}
```

- [ ] **Step 4: Verify**

```bash
cd frontend
pnpm exec tsc --noEmit
pnpm build
```

Then, with `cargo run` from the repo root and `pnpm dev` in `frontend/`, open
`http://localhost:5273/decks` and walk: type a partial name and watch it narrow; clear it;
pick a module from the dropdown; click a card's module badge and confirm the filter follows;
switch to Oldest first and confirm the order reverses; filter to something with no matches
and use "Clear filters"; create and edit a deck and confirm the list updates; narrow the
window to 375px and confirm the toolbar stacks.

- [ ] **Step 5: Commit**

```bash
git add frontend
git commit -m "feat(ui): flat deck list with search, module filter and date sort"
```

```json:metadata
{"files":["frontend/src/lib/api.ts","frontend/src/pages/DecksPage.tsx","frontend/src/components/ui/badge.tsx"],"verifyCommand":"cd frontend && pnpm exec tsc --noEmit && pnpm build","acceptanceCriteria":["no module section grouping remains; one flat card grid","each card shows a module badge or a muted No module badge","clicking a module badge sets the module filter","toolbar has search, module select and sort select; stacks below sm:","typing is debounced ~250ms","a slow earlier response cannot overwrite a newer one","two distinct empty states, with a clear-filters action on the filtered one","tsc and build both clean","creating and editing a deck still refreshes the list"],"modelTier":"standard"}
```

---

## Verification — the feature as a whole

**Automated**

```bash
cargo test                                   # Part 1's suite plus the new decks tests
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

**Live API**

```bash
curl -s 'localhost:3000/api/decks?q=kine'
curl -s 'localhost:3000/api/decks?module_id=none&sort=oldest'
curl -s 'localhost:3000/api/decks?q=100%25'          # literal %, matches only real "100%"
curl -s -o /dev/null -w '%{http_code}\n' 'localhost:3000/api/decks?sort=sideways'   # 422
```

**Browser** — the walkthrough in Task 2 Step 4. The debounce and the stale-response guard
are the two things only a human can really judge: typing should feel immediate with no
flicker back to previous results.

## Task dependencies

Task 2 is blocked by Task 1 — the page cannot query parameters the API does not accept yet.
