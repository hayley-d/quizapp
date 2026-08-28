# Part 7 (SM-2) — mid-execution handover

**Written:** 2026-08-28, on branch `feat/part7-sm2`, after Task 4 of 8.
**Why this exists:** the session executing Part 7 ran out of context window. This file is what
a fresh session needs to finish the job. Delete it when Part 7 merges — everything durable in
it belongs in `docs/HANDOVER.md` by then.

## Read these first, in this order

1. `docs/mitis/specs/2026-08-28-part7-sm2-design.md` — the design record. Twelve sections,
   every decision and its cost. **This is the authority on intent.**
2. `docs/mitis/plans/2026-08-28-part7-sm2.md` — the eight-task implementation plan, with
   complete code per task. **Read the caveat below before trusting its test code.**
3. `.mitis/sdd/2026-08-28-part7-sm2/progress.md` — the execution ledger. Untracked and local.
   It records every review finding, every adjudication and every ruling made so far.
4. `docs/HANDOVER.md` — the project-wide handover. Its "Environment quirks", "The verification
   gate" and "Conventions and traps" sections each prevent a specific hour-long mistake.

## Where things stand

**Tasks 1-4 are complete and reviewed clean.** Nothing is merged; the branch is
`feat/part7-sm2`, cut from `main` at `1b8e466` (the Part 6 merge).

| Task | State | Commits |
| --- | --- | --- |
| 1 — the pure SM-2 module | complete, review clean (1 fix round) | `0d1b0a9..7b893df` |
| 2 — sm2 session creation | complete, review clean | `7b893df..9f49a8e` |
| 3 — due-ordered serving | complete, review clean, **zero findings** | `9f49a8e..ebbe936` |
| 4 — the schedule write | complete, review clean (1 minor deferred) | `40e87e4` |
| 5 — override replay | not started | — |
| 6 — stats bucket and due counts | not started | — |
| 7 — frontend | not started | — |
| 8 — docs and the full gate | not started | — |

**The suite is at 355 tests, all passing** (317 before Part 7). `SQLX_OFFLINE=true cargo build`
is clean; clippy `--all-targets -D warnings` is clean.

### Your first action

**Start at Task 5 (the override replay).** Tasks 1-4 are done and reviewed; the review verdict
for Task 4 arrived after the handover was first written and is recorded in the ledger.

Two properties Task 4 established, which Task 5 builds directly on — verified rather than
assumed, so you can rely on them:

- **The answer write is atomic.** The `reviews` insert and the `schedule` write share one
  transaction. `a_failed_schedule_write_rolls_back_the_review` drops the `schedule` table
  before `/answer`, so the insert completes and the failure lands *after* it — a rollback is
  genuinely exercised, and moving the commit earlier turns the test red.
- **`due_at` is midnight UTC of the due day**, from the review's own `answered_at`, never
  `now`. The `assert_eq!(due_at.len(), 20)` is what pins this: `date()` yields 20 characters
  and the `datetime()` mutation yields 29.

Note one incidental widening from Task 4, accepted at review: **all** modes now insert their
`reviews` row inside a transaction, not only sm2. It was the minimal way to satisfy the
requirement.

## The caveat that matters most

**The plan's own test code has shipped four hollow tests so far — tests that passed and proved
nothing.** Every one was caught by mutation testing or by an implementer refusing to write
what the plan said. Treat the plan's *implementation* code as sound and its *test* code as
suspect:

1. **Task 1, three tautologies.** Tests asserted against the same named constants the
   implementation used (`MINIMUM_EASE`, `FIRST_INTERVAL_DAYS`, `SECOND_INTERVAL_DAYS`,
   `QUALITY_CORRECT`). Moving the constant moved both sides. Fixed by asserting literals.
2. **Task 2, an unasserted message.** A test checked only the error's *field name*, never its
   message text, so the acceptance criterion "with an sm2-specific message" was unpinned.
3. **Task 4, an unfailable suffix check.** `assert!(due_at.ends_with("T00:00:00Z"))` could not
   fail, because the SQL appends that literal unconditionally — `datetime()` would produce
   `...21:00:00T00:00:00Z` and still pass. Fixed with a length assertion (20 versus 29
   characters), which the reviewer confirmed does discriminate.

**The standing rule, now recorded in the plan: where a constant IS the specification, assert
the literal.** And run the mutations — one change at a time.

## Rulings already made — do not re-litigate these

- **`repetitions` in Rust, `reps` in SQL.** The `schedule` column keeps its name because Part 7
  ships **no migration**, deliberately. Every Rust and TypeScript identifier is `repetitions`;
  SQL aliases at the boundary (`reps AS "repetitions!: i64"`). Task 8 must record this split in
  `docs/HANDOVER.md` so nobody later "fixes" one half of it.
- **`schedule_for` was NOT replaced.** The plan said to widen it; that would have silently
  gutted `backend/tests/cards.rs:267`, which destructures its 2-tuple and asserts the row count
  is 1. A new `schedule_state_for` was added alongside, and Part 5's mock canary
  (`backend/tests/mock.rs`, "mock mode must leave the sm-2 schedule alone") was pointed at the
  wider helper so it now proves `interval_days`, `ease`, `repetitions` and `lapses` are
  untouched too.
- **The `answer` handler keeps its inline `can_override` predicate.** The plan's Task 4 Step 4
  said to swap it for `can_override_result`. That is **wrong**: `can_override_result` returns
  true for an incorrect flashcard and breaks the pinned test at `backend/tests/sessions.rs:1023`
  ("a flashcard grader can simply grade again"). In practice and SM-2 you re-grade a flashcard;
  you do not override it. `can_override_result` remains correct for the `/results` path
  (`sessions.rs:387`), which is a different, post-hoc context. **Ignore that step of the plan.**
- **A lapse leaves the ease factor unchanged.** This is the original SM-2 behaviour and is
  deliberate (design §3a). A majority of implementations on the internet do the opposite, so
  this will look like a bug to a future reader. It is pinned by a unit test and is on the
  mutation list. Do not "fix" it.
- **`correct_count` belongs in the sm2 serve payload.** Part 5 stripped it from *mock* because a
  running score is withheld information there. SM-2 gives per-question feedback like practice,
  so it leaks nothing. Reviewed and confirmed in Task 3.

## Remaining work

Tasks 5-8 are fully specified in the plan with complete code. Summary of intent:

- **Task 5 — override replay.** Overriding an sm2 review recomputes that card's schedule by
  replaying every **sm2** review for the card in `answered_at, id` order, with `due_at` based on
  the **last review's `answered_at`**, not `now`. Practice and mock overrides must not touch
  `schedule`. An sm2 flashcard override stays refused (see the ruling above).
- **Task 6 — stats.** `DeckStatsSummary` gains `sm2_accuracy`, `sm2_review_count` (the third
  strip figure — this is the answer to Part 6 §10) plus `due_count` and `next_due_at`, which are
  **tile data, not strip figures**. Watch the placeholder ordering: the new `due` CTE binds
  `deck_id` a second time and `query!` counts placeholders by occurrence.
- **Task 7 — frontend.** `Sm2NextResponse` joins the `NextResponse` union; `SessionPage` widens
  its `served` state and gets a mode-aware header (`n of m due`); the deck tile enables on
  `due_count`; the strip grows a third figure. **No new colour token** — `check-contrast.py`
  must still report **16 ENFORCED rows with an empty RECORDED tier**.
- **Task 8 — docs and the full gate.** Answer Part 6 §10 *in that document*; amend the master
  spec; add a Part 7 section to `docs/HANDOVER.md`; run the whole gate; record the **actual**
  test count, not a carried-forward figure (it has been stale twice before).

## How to continue

The plan was being executed with `mitis:subagent-driven-development`: one implementer subagent
per task, then a task reviewer, then a scoped re-review for any fix round. To resume:

```
/mitis:executing-plans docs/mitis/plans/2026-08-28-part7-sm2.md
```

or re-invoke `mitis:subagent-driven-development` with the same plan path. The
`.tasks.json` beside the plan tracks per-task status; Tasks 1-3 are marked `completed`.

Per-task dispatch recipe that worked:

- Generate the brief with the skill's `scripts/task-brief PLAN_FILE N`, hand the implementer
  the **path**, and never paste the plan into a prompt.
- Give every dispatch the four CLAUDE.md global constraints verbatim (no comments, no
  abbreviations, no `any`, no `Co-Authored-By`), the environment quirks, and the rulings above.
- Demand mutation evidence, one change at a time, with `touch` after every restore.
- **Run one implementer at a time.** Git's index is not per-file; in Part 2a two concurrent
  writers produced a mislabelled commit.

### Two environment facts that cost time here

- **The sqlx offline cache must be regenerated against a scratch database**, never
  `data/quizapp.db`. Every task from 2 onward needed this:
  ```bash
  export PATH="$HOME/.cargo/bin:$PATH"
  SCRATCH="$(mktemp -d)"
  for migration in backend/migrations/*.sql; do sqlite3 "$SCRATCH/prepare.db" < "$migration"; done
  DATABASE_URL="sqlite://$SCRATCH/prepare.db" cargo sqlx prepare --workspace
  rm -rf "$SCRATCH"
  ```
- **Changing a query's text invalidates that cache**, so mutation experiments on SQL must run
  against a scratch `DATABASE_URL` rather than the offline path. Task 3 worked this out.
- The editor's rustc diagnostics repeatedly claimed a stale sqlx cache when the cache was fine.
  `SQLX_OFFLINE=true cargo build` is the authority; it was clean every time.

## The gate

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
python3 frontend/scripts/check-contrast.py
cd frontend && pnpm exec tsc -b --noEmit && pnpm build && pnpm exec oxlint
```

`tsc -b --noEmit`, not `tsc --noEmit` — the bare form reads a solution file with `"files": []`,
finds nothing and exits 0 whatever the code says.

## Deferred minors, for the final whole-branch review to triage

- **Task 4:** `load_schedule_state` and `write_schedule` sit after the `answer` handler
  (`sessions.rs:1118+`) rather than beside `count_due` as the plan said. Purely organizational,
  no functional effect.

## Still unresolved for Part 7

- **The browser walkthrough has not been driven**, and the Chrome extension has not been
  available on this machine for Parts 5 or 6 either. Task 8's step 5 lists nine points. If it
  cannot be driven, **record it as undriven and itemised** — do not fold it into "Part 7
  complete". The gate cannot see a layout and it cannot see a leak.
- **375px has never been rendered in any part of this project.** Task 7 adds a third figure to
  a stats strip that has never been seen at phone width. It belongs to build step 8.
- **A fragility worth naming in Task 8's handover entry:** `NextResponse` is
  `#[serde(untagged)]` and derives only `Serialize`, so there is no ambiguity today. If
  `Deserialize` is ever added, the three variants still discriminate by required-field presence
  (Practice has no `target_count`; Mock requires `started_at`; Sm2 requires `correct_count`) —
  but it becomes a real hazard.
