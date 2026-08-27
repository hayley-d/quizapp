# Part 2b — image upload and the shared Markdown component

**Written:** 2026-08-27, at `main` = `2cb6636`.

Part 2b is infrastructure for screens that already exist, not a new screen. Two features:
uploading images so a diagram card can have one, and a single `<Markdown>` component so
prompts, choices and answers render markdown with KaTeX. Part 2a deliberately rendered raw
text everywhere so this gets built once — the card list, the editor preview and Part 3's
session runner all consume the same component. A second rendering path is the exact outcome
this split exists to prevent.

The parent spec is [`2026-08-26-quiz-study-app-design.md`](2026-08-26-quiz-study-app-design.md); everything below is
consistent with it except the one divergence recorded in §1.

## 1. Image upload — the endpoint

**Divergence from the parent spec.** Its API table says
`POST /api/cards/:id/image`. Part 2b instead uses a standalone **`POST /api/images`**, and
the parent spec's table has been amended to match.

The card-scoped endpoint needs a card to already exist. For a diagram card the image *is*
the prompt — a diagram with a label erased — so the author wants it on screen while writing
the accepted answers. Worse, a `short_answer` card cannot be saved without at least one
accepted answer, so a card-scoped upload forces the author to invent an answer before they
can attach the image they are writing the answer about. A standalone endpoint lets the
editor upload first, hold the returned path in form state, and send `image_path` with the
normal create or PATCH — which fits the existing full-replace `PATCH /api/cards/:id`
without adding anything.

**The cost, accepted:** an upload whose card is never saved leaves an orphan file in
`data/images/`. On a local single-user app that is a few stray KB, sweepable later by
comparing `data/images/` against `cards.image_path`. No sweeper is built in Part 2b.

### Request and response

`POST /api/images`, `multipart/form-data`, one part named `file`.

```
201  { "path": "images/ab12cd34ef567890.png" }
```

The returned value is stored verbatim in `cards.image_path`.

### Validation

Both halves of the parent spec's "images are size- and type-checked; a rejected upload
leaves the card intact" are load-bearing.

- **Type by magic bytes**, never the client-declared `Content-Type`: PNG, JPEG, WebP. A
  file named `.png` whose bytes are something else is rejected. The extension written to
  disk comes from the sniffed type, not from the uploaded filename — which is otherwise
  discarded entirely, so it cannot carry a path or a traversal sequence.
- **5 MiB maximum**, enforced in the handler so the rejection returns the standard
  `{error, message, fields}` envelope like every other failure. Axum's `DefaultBodyLimit`
  defaults to 2 MiB and rejects with a raw `text/plain` 413 that would bypass the envelope,
  so this route raises that limit above 5 MiB and the handler's own check is what actually
  rejects. A deliberately enormous upload still trips axum's limit; that is the backstop,
  not the mechanism.
- **A rejected upload cannot damage a card** structurally: the endpoint does not touch the
  `cards` table at all, and the editor only writes to form state on success.

### Storage

Files are written to `{data_dir}/images/`, where `data_dir` is the existing
`QUIZAPP_DATA_DIR` config value (default `data`). `main.rs` already does `create_dir_all`
on the data directory; that is extended to the `images/` subdirectory rather than inventing
a second path convention. `data/` is gitignored, so `data/images/` is too.

**Filenames are content-addressed:** the first 16 hex characters of the SHA-256 of the file
bytes, plus the sniffed extension. This needs no RNG dependency, cannot collide in
practice, and makes re-uploading the same diagram reuse the existing file. It adds one
crate, `sha2`.

### Serving

`tower-http` gains the **`fs`** feature — the one dependency the backend definitely needs —
and a `ServeDir` mounts `{data_dir}/images` at `/images`. The browser URL for a card's
image is therefore `/` + `image_path`. Vite's dev server gets an `/images` proxy alongside
the existing `/api` one.

### `image_path` on cards

`cards.image_path TEXT` already exists in `0001_init.sql` and is unused. **No migration is
needed, and editing an applied migration breaks its sqlx checksum — so do not.**

`CardInput` gains `image_path: Option<String>`, written by both create and PATCH under the
existing cards full-replace rule: an absent value means null. The editor always sends the
field, so clearing an image is just saving without it. This is the cards convention, not
the decks absent-vs-null one; see the doc comment on `cards::patch`.

The value is validated against `^images/[0-9a-f]{16}\.(png|jpg|webp)$`. The server assigns
every path it ever returns, so a value outside that shape did not come from an upload, and
the stored string ends up in the DOM as a URL. Rejecting it is a two-line guard.

A diagram question remains a `short_answer` card with `image_path` set, not a fourth kind.
Image hotspots and click-a-region answering stay non-goals.

## 2. The shared `<Markdown>` component

`frontend/src/components/Markdown.tsx`, built on `react-markdown` with `remark-math` and
`rehype-katex`. Four new frontend dependencies: `react-markdown`, `remark-math`,
`rehype-katex`, `katex`.

KaTeX's stylesheet is imported from the npm package (`katex/dist/katex.min.css`), which
brings its own fonts locally. **It must not come from a CDN.** `globals.css` already pulls
Quicksand and Inter over the network — a known defect deferred to build step 8 — and a
second network dependency makes that worse. Here it costs nothing to get right.

Raw HTML stays disabled, which is react-markdown's default: no `rehype-raw`.

Three consumers, one component: the card list, the editor preview, and Part 3's session
runner.

The real input is Obsidian markdown with inline LaTeX — `$X, Y, Z$`, `$10\ 000$` — so that
is what the component is exercised against, not `$x^2$`.

## 3. The editor

### Image control

A file picker that uploads immediately on selection, then shows the thumbnail and a
**Remove** button. The returned path lives in form state and goes out with the next save.

An upload failure renders inline beside the control, from the envelope's `fields`, exactly
like every other field error — and touches nothing else on the form. A rejected save must
never clear typed content, and that rule extends to a rejected upload.

### Edit ↔ Preview toggle

The whole form switches between source and rendered, rather than a side-by-side pane or
per-field render-on-blur. The editor is keyboard-first: a toggle adds no tab stops to the
typing loop and needs no second layout at 375px, where a preview pane would compete for
width with the choices rows. Part 2a rejected live side-by-side as out of scope for that
stage; this is the considered answer rather than a deferral.

Bound to `⌘/Ctrl+P`, with `preventDefault` — that is the browser's print shortcut.

Preview renders, through `<Markdown>`: the prompt, the choices with their correct marks,
the accepted answers, the flashcard answer, and the explanation. Plus the image. Returning
to Edit restores focus to the prompt field.

## 4. The card list

`DeckPage` currently shows `prompt_md.split('\n')[0]` as plain text. Rendering markdown
into a truncated single line is the awkward case — a heading marker or a half-open `$…$`
mid-truncation looks broken — so the row stops being a single line instead. Rows render
the full prompt through `<Markdown>`, unclamped and multi-line, and `firstLine()` is
removed. Multi-line bullets display as written.

The trade-off is a long page for a 100+ card deck, which is what COS781 will be. Accepted
deliberately; whether it reads well is on the browser-verification list below, and the kind
filter and prompt search deferred out of Part 2a Task 4 remain the fix if it does not.

A `<CardImage>` component renders a fixed ~64px-tall thumbnail beside the prompt — CSS
scaling of the same file, no resizing pipeline. Clicking it opens the full image in a
lightbox built on the existing `ui/dialog.tsx`. The same component serves the editor, so
the diagram is reachable at full size while authoring.

## 5. Testing

Backend, in the existing integration style against a temporary SQLite file:

- a valid upload returns a path and the file exists on disk at it
- an oversize upload is rejected with the envelope, and writes no file
- a file whose bytes do not match any accepted type is rejected — including one named
  `.png` that is not a PNG, which is what makes the check magic-byte-based rather than
  extension-based
- two uploads of identical bytes yield the same path
- `image_path` round-trips through create → GET → PATCH
- a PATCH omitting `image_path` clears it
- an `image_path` outside the assigned shape is rejected

Every one of these carries mutation evidence: delete or invert the specific line it claims
to pin, and the test must go red. Five of Part 2a's six fix rounds were the same defect — a
test naming a contract it did not actually pin down — so the mutation pass is not optional,
and "no mutation made this fail" is an acceptable honest result where a fabricated green is
not.

No frontend test framework. That is the parent spec's deliberate decision, not an omission.

## 6. What only a browser can settle

The Chrome extension is not connected on this machine, so these join `HANDOVER.md`'s
Outstanding list rather than being verified here:

- whether 100+ unclamped rows, each rendering KaTeX, stay responsive and scannable
- whether the Edit/Preview toggle feels right inside the keyboard loop, and whether focus
  lands where expected on the way back
- the thumbnail and lightbox at 375px
- KaTeX legibility against both Bibble palettes

## 7. Explicitly out of scope

- An orphan-file sweeper.
- Image resizing, cropping, or thumbnail generation — the thumbnail is CSS.
- Paste-from-clipboard or drag-and-drop upload. A file picker is enough for now.
- Image hotspots and click-a-region answering, which are parent-spec non-goals.
