# Part 2b handoff — images and the shared Markdown component

**Written:** 2026-08-27, at `main` = `f9811a1` (Part 2a merged).
**Read [`HANDOVER.md`](HANDOVER.md) first.** It carries the project state, the environment
quirks, the verification gate and the conventions. This document only covers what is
specific to Part 2b and is not written down anywhere else.

Nothing in Part 2b has been started. No branch, no plan, no code.

## Ask these three questions before planning

Part 2b looks small — two features — but each carries a design decision that changes the
work, and one of them contradicts the spec. Get answers before writing a plan.

### A. Where does the image upload endpoint live? (This one contradicts the spec.)

The spec's API table says:

```
POST /api/cards/:id/image     multipart upload -> data/images/, returns path
```

That endpoint needs a card to already exist. But consider how a diagram card is actually
written: the image **is** the prompt — a diagram with a label blanked out — so the author
wants it in front of them while writing the accepted answers. With a card-scoped endpoint
the flow becomes: write the prompt, save the card, upload the image, carry on editing. Worse,
a `short_answer` card cannot be saved until it has at least one accepted answer, so the
author has to invent an answer before they can attach the image they are writing the answer
about.

Three ways out:

1. **Follow the spec.** Card-scoped upload. The editor saves first, then uploads. Simplest
   server side, and every image is owned by a card so orphans are impossible. Worst
   authoring flow, on the app's most-used screen.
2. **Standalone `POST /api/images`** returning a stored path; the editor holds the path in
   form state and sends `image_path` with the normal create or patch. Best authoring flow,
   and it fits the existing full-replace `PATCH /api/cards/:id` cleanly. Cost: an upload
   whose card is never saved leaves an orphan file. On a local single-user app that is a few
   stray KB, sweepable later by comparing `data/images/` against `cards.image_path`.
3. **Card-scoped, with the editor auto-saving a draft first.** Hides the awkwardness behind
   draft state that nothing else in the app has. Most machinery, least clarity.

**My lean is 2**, with the orphan risk accepted and written into the spec as a deliberate
divergence — the same way the normalisation step order and `POST /api/cards/:id/unarchive`
were recorded in Part 2a. But it is a spec change, so it is Hayley's call, not an
implementer's.

### B. What does the editor's preview look like?

The spec says prompts, choices and answers all render markdown with KaTeX. It does not say
what the editor shows while typing. Options: a live side-by-side pane, a toggle between
source and rendered, or render-on-blur per field.

This matters more than it sounds because the editor is keyboard-first. A preview pane joins
the tab order and competes for width with the choices rows at 375px. Part 2a's design
session explicitly rejected "live side-by-side preview" as too much for that stage — that
was a scoping call at the time, not a permanent ruling, so it is genuinely open now.

### C. Does the card list render markdown, or stay raw?

`DeckPage` currently shows `prompt_md.split('\n')[0]` as plain text. Rendering markdown into
a truncated single line is awkward — a heading marker, a half-open `$…$`, or a list bullet
mid-truncation all look broken. Options: leave the list raw, render it properly, or strip
markdown to plain text for the list only. Note that stripping is a third rendering path, so
it argues against itself.

## Facts already checked, so you do not have to

- **`axum` already has the `multipart` feature** (`backend/Cargo.toml`). Nothing to add for
  upload parsing.
- **`tower-http` has only `trace`.** Serving `data/images/` over HTTP needs the **`fs`**
  feature for `ServeDir`. This is the one dependency change the backend definitely needs.
- **`cards.image_path TEXT`** exists in migration `0001_init.sql` and is entirely unused. No
  migration is needed for Part 2b — and editing an applied migration breaks its sqlx
  checksum, so do not.
- **The frontend has no markdown or maths dependencies yet.** It will need
  `react-markdown`, `remark-math`, `rehype-katex` and `katex`.
- **`data/` is gitignored**, so `data/images/` is too. `Config` already exposes
  `QUIZAPP_DATA_DIR`, defaulting to `data`, and `main.rs` already does `create_dir_all` on
  it — extend that rather than inventing a second path convention.
- **KaTeX ships its own fonts in the npm package**, so bundling them is local by default.
  Do not pull KaTeX CSS from a CDN: the Google Fonts problem in `globals.css` is already a
  known defect deferred to build step 8, and adding a second network dependency makes that
  worse. This is the one place where getting it right now costs nothing.

## Spec constraints that bind this work

- **"Images are size- and type-checked; a rejected upload leaves the card intact."** Both
  halves matter — the check, and the guarantee that a failed upload does not damage an
  existing card.
- **A diagram question is not a fourth card kind.** It is a `short_answer` card with
  `image_path` set. The image is prepared externally with the label already erased. Image
  hotspots and click-a-region answering are explicit non-goals.
- **Source notes are Obsidian markdown with inline LaTeX** — `$X, Y, Z$`, `$10\ 000$`. That
  is the actual input this has to handle, so test with it rather than with `$x^2$`.

## The whole point of the shared component

Part 2a deliberately rendered raw text everywhere so this gets built **once**. One
`<Markdown>` component — `react-markdown` + `remark-math` + `rehype-katex` — serving the
card list, the editor preview, and later the session runner in Part 3. If you find yourself
writing a second rendering path, stop: that is the exact outcome the 2a/2b split exists to
prevent.

## Process notes for whoever runs this

`HANDOVER.md`'s closing section has the workflow (`mitis:brainstorming` →
`mitis:writing-plans` → `mitis:subagent-driven-development`) and the two habits that earned
their cost. Three more from the Part 2a run:

- **Run one implementer at a time.** In Part 2a two implementers were dispatched
  concurrently because their *file lists* were disjoint. Git's index is not per-file: one
  agent's `git add`/`commit` swept the other's staged work into a commit labelled as
  something else. It was recoverable only because the branch was local and unpushed.
  Read-only reviewers can safely run in parallel with an implementer; two writers cannot.
- **Front-load the known failure pattern into the brief.** Five of Part 2a's six fix rounds
  were the same defect — a test naming a contract it did not pin down. Once the brief said
  so explicitly and made the mutation pass non-optional with a required results table, the
  remaining tasks cleared review first time.
- **An honest "no test failed" beats a fabricated green.** Two Part 2a tasks reported
  mutations that discriminated nothing. One was upheld as genuinely untestable; the other
  was refuted by a reviewer who found the harness could reach it after all. Neither
  conversation could have happened if the implementer had quietly claimed success.

## What is still owed from Part 2a

`HANDOVER.md` § Outstanding has the full list. The browser-only checks are genuinely
unverified — the Chrome extension is not connected on this machine, so no agent could drive
a browser for Part 1 or Part 2a. If you get a browser, that list is worth an hour before
building more on top.
