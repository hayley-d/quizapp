# Part 2b — Images and the Shared Markdown Component — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `mitis:subagent-driven-development`
> (recommended) or `mitis:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking. Tasks:
> `docs/mitis/plans/2026-08-27-part2b-images-markdown.md.tasks.json`
>
> Read `docs/HANDOVER.md` before starting. Its "Environment quirks" and "Conventions and
> traps" sections each cost a fix round to learn and are assumed throughout this plan.

**Goal:** A card can carry an uploaded image, and every place a card's text appears renders
markdown with KaTeX through one shared component.

**Architecture:** A standalone `POST /api/images` writes content-addressed files into
`{data_dir}/images/` and returns the path the editor stores on the card; a `ServeDir` serves
that directory read-only at `/images`. On the client, one `<Markdown>` component
(`react-markdown` + `remark-math` + `rehype-katex`) is the app's only rendering path, used by
the card list, the editor's preview, and later Part 3's session runner.

**Tech Stack:** Rust/axum/sqlx (SQLite), `sha2`, `tower-http` `fs`; React 19 + Vite +
Tailwind v4, `react-markdown`, `remark-math`, `rehype-katex`, `katex`.

**User decisions (already made):**
- Upload lives at a **standalone `POST /api/images`**, not the spec's
  `POST /api/cards/:id/image`. Orphan files from an abandoned upload are accepted; the parent
  spec's API table has already been amended.
- The editor gets a **whole-form Edit ↔ Preview toggle**, not a live side-by-side pane and
  not per-field render-on-blur.
- The card list renders the **full prompt as markdown, unclamped and multi-line** —
  `firstLine()` goes away. The long-page trade-off at 100+ cards is accepted deliberately.
- A card's image appears in the list as a **~64px thumbnail that opens a lightbox on click**.
- Size cap **5 MiB**; accepted types **PNG, JPEG, WebP**; `sha2` is the one new backend crate.

Full design: [`docs/mitis/specs/2026-08-27-part2b-images-markdown-design.md`](../specs/2026-08-27-part2b-images-markdown-design.md).

---

## Context

Parts 1 and 2a are on `main`. Part 2a deliberately rendered raw text everywhere so the
markdown renderer gets built exactly once — that is the entire reason this plan exists rather
than being folded into 2a. **If you find yourself writing a second rendering path, stop.**

`cards.image_path TEXT` already exists in `backend/migrations/0001_init.sql` and is unused.
**No migration is written in this plan.** Editing an applied migration changes its checksum
and sqlx then refuses to run against the existing database — a comment-only edit is enough.

**Deliberately out of scope:** an orphan-file sweeper; image resizing or thumbnail
generation (the thumbnail is CSS); paste-from-clipboard and drag-and-drop upload; GitHub
Flavored Markdown tables and strikethrough (no `remark-gfm` — add it when a real card needs
one); image hotspots and click-a-region answering, which are parent-spec non-goals.

---

## Cross-cutting conventions

Everything in `docs/HANDOVER.md` § "Conventions and traps" applies. The ones this plan leans
on hardest:

**Every failure returns the envelope** `{"error","message","fields"}` as `application/json`.
That now includes upload failures: they arrive with `fields[0].field == "file"` and render
inline beside the file picker. **A rejected upload must leave every other typed field
untouched**, exactly like a rejected save.

**Run cargo from the repo root, never from `backend/`**, and `export PATH="$HOME/.cargo/bin:$PATH"`
before any `cargo sqlx` command.

**Regenerate the sqlx cache after any SQL change** (Task 4 is the only task that changes SQL):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
git add .sqlx
```

**The verification gate**, run before declaring any task done:

```bash
cargo test
cargo clippy --all-targets -- -D warnings        # --all-targets is not optional
SQLX_OFFLINE=true cargo build
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

### Mutation evidence is required, not optional

Five of Part 2a's six fix rounds were the same defect: **a test that names a contract it does
not actually pin down.** For every test you write, delete or invert the single line of
production code it claims to prove, run it, and record the result. Each task below carries a
results table to fill in. Mutate **one thing at a time** — "I changed X and Y and it went
red" is evidence for neither.

An honest "I mutated this and no test failed" is a good outcome and must be reported as such.
A fabricated green is the one unacceptable answer. Two Part 2a tasks reported mutations that
discriminated nothing; one was upheld as genuinely untestable and one was refuted by a
reviewer — neither conversation could have happened if the implementer had quietly claimed
success.

### The image contract (both sides of the seam)

Tasks 1–4 build the server half and Tasks 6–8 the client half. A per-task review structurally
cannot catch a mismatch across this seam, so it is written down once, here.

`POST /api/images`, `multipart/form-data`, exactly one part named `file`:

```json
201  { "path": "images/ab12cd34ef567890.png" }
```

- `path` is **relative to the data directory** and is stored verbatim in `cards.image_path`.
- The browser URL for it is `/` + `path` — i.e. `/images/ab12cd34ef567890.png`.
- The stem is 16 lowercase hex characters (the first 8 bytes of the SHA-256 of the file).
- The extension comes from the **sniffed bytes**, never from the uploaded filename:
  `png`, `jpg`, `webp`.
- Failures are the normal envelope with `fields[0].field == "file"`.

`cards.image_path` on every card DTO is `string | null`, and `CardInput` accepts
`image_path` under the existing **cards full-replace rule: an absent value means null.** The
editor always sends the key explicitly, so removing an image is just saving with `null`.

---

## Task 1: `AppState` carries the images directory, served at `/images`

**Goal:** The server knows where uploaded images live, creates that directory on startup, and
serves it read-only at `/images` — with the test harness wired the same way, so later tasks
have somewhere to write.

**Files:**
- Modify: `backend/Cargo.toml` (add the `fs` feature to `tower-http`)
- Modify: `backend/src/state.rs`
- Modify: `backend/src/config.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/src/main.rs`
- Modify: `backend/tests/common/mod.rs`
- Create: `backend/tests/images.rs`

**Acceptance Criteria:**
- [ ] `AppState` carries an `images_dir: PathBuf`, so the writer and the reader cannot disagree about the path
- [ ] `Config::images_dir()` derives it from the existing `QUIZAPP_DATA_DIR`, not from a second env var
- [ ] `main.rs` creates the directory on startup
- [ ] A file placed in the directory is served at `/images/<name>` with its bytes intact
- [ ] A request for a file that is not there is a 404, not a 500
- [ ] A traversal path such as `/images/../test.db` does not escape the directory
- [ ] `spawn_app()` gives every test its own images directory inside its tempdir

**Verify:** `cargo test --test images && cargo clippy --all-targets -- -D warnings` → all pass

**Steps:**

- [ ] **Step 1: Add the `fs` feature to `tower-http`**

In `backend/Cargo.toml`, replace the existing `tower-http` line:

```toml
tower-http = { version = "0.6", features = ["trace", "fs"] }
```

`ServeDir` lives behind `fs`. This is the only backend dependency change in this task.

- [ ] **Step 2: Put the directory in `AppState` — `backend/src/state.rs`**

```rust
use std::path::PathBuf;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    /// Where uploaded images live. `POST /api/images` writes here and the
    /// `/images` ServeDir reads from here; holding it in one place is what
    /// stops the writer and the reader drifting apart.
    pub images_dir: PathBuf,
}
```

- [ ] **Step 3: Derive it from the data dir — `backend/src/config.rs`**

Add the import at the top of the file:

```rust
use std::path::{Path, PathBuf};
```

and this `impl` block below the existing `from_env`:

```rust
impl Config {
    /// Images go in a subdirectory of the data dir rather than behind a second
    /// env var: one convention, one directory to back up, one thing to
    /// gitignore (`data/` already is).
    pub fn images_dir(&self) -> PathBuf {
        Path::new(&self.data_dir).join("images")
    }
}
```

Keep `from_env` in its existing `impl Config` block or merge the two — either is fine, but do
not duplicate `from_env`.

- [ ] **Step 4: Mount the static route — `backend/src/lib.rs`**

```rust
use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub fn app(state: state::AppState) -> Router {
    // Read-only, outside `/api`, and deliberately not behind the AppJson
    // extractor: these responses are image bytes, not the error envelope.
    // ServeDir rejects paths that escape its root, which is the only reason
    // it is safe to hand it a directory of client-supplied filenames.
    let images = ServeDir::new(state.images_dir.clone());

    Router::new()
        .nest("/api", routes::api_router())
        .nest_service("/images", images)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

- [ ] **Step 5: Create the directory on startup — `backend/src/main.rs`**

Replace the two lines around the existing `create_dir_all`:

```rust
    let config = Config::from_env();
    std::fs::create_dir_all(&config.data_dir)?;
    let images_dir = config.images_dir();
    std::fs::create_dir_all(&images_dir)?;
    let pool = quizapp::db::connect(&config.database_url).await?;
    let app = quizapp::app(AppState { pool, images_dir });
```

- [ ] **Step 6: Wire the test harness — `backend/tests/common/mod.rs`**

Add `use std::path::PathBuf;` to the imports, then replace `TestApp` and `spawn_app`:

```rust
pub struct TestApp {
    pub router: Router,
    pub pool: sqlx::SqlitePool,
    pub images_dir: PathBuf,
    _dir: tempfile::TempDir,
}

pub async fn spawn_app() -> TestApp {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = format!("sqlite://{}/test.db?mode=rwc", dir.path().display());
    let pool = quizapp::db::connect(&url).await.expect("db connect");
    // Inside the same tempdir as the database, so a test's uploads are torn
    // down with it and no test can see another's files.
    let images_dir = dir.path().join("images");
    std::fs::create_dir_all(&images_dir).expect("images dir");
    let router = quizapp::app(quizapp::state::AppState {
        pool: pool.clone(),
        images_dir: images_dir.clone(),
    });
    TestApp { router, pool, images_dir, _dir: dir }
}
```

Then add this method inside `impl TestApp`, next to `get`:

```rust
    /// A GET returning the raw body: `request()` parses JSON, and the image
    /// route returns image bytes.
    pub async fn get_raw(&self, uri: &str) -> (StatusCode, Vec<u8>) {
        let req = Request::builder().method("GET").uri(uri).body(Body::empty()).unwrap();
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        (status, bytes)
    }

    /// Number of files in this app's images directory.
    pub async fn image_count(&self) -> usize {
        std::fs::read_dir(&self.images_dir).expect("read images dir").count()
    }
```

- [ ] **Step 7: Write the failing tests — `backend/tests/images.rs`**

```rust
mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn serves_a_file_from_the_images_directory() {
    let app = common::spawn_app().await;
    std::fs::write(app.images_dir.join("diagram.png"), b"pretend-image-bytes").unwrap();

    let (status, body) = app.get_raw("/images/diagram.png").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"pretend-image-bytes");
}

#[tokio::test]
async fn an_absent_image_is_404_not_500() {
    let app = common::spawn_app().await;
    let (status, _) = app.get_raw("/images/nothing-here.png").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_traversal_path_cannot_escape_the_images_directory() {
    let app = common::spawn_app().await;
    // test.db is the sibling of images/ inside the same tempdir, so if the
    // static route resolved `..` this would hand out the database file.
    let (status, body) = app.get_raw("/images/../test.db").await;
    assert_ne!(status, StatusCode::OK, "the database must not be reachable over HTTP");
    assert!(
        !body.starts_with(b"SQLite format 3"),
        "served the database file: the static route escaped its root",
    );
}
```

- [ ] **Step 8: Run them and watch them fail**

Run: `cargo test --test images`
Expected: compile error — `images_dir` is not a field of `AppState` until Steps 2–6 are in.
If you did the steps in order, expect instead: FAIL on `serves_a_file_from_the_images_directory`
with 404 until Step 4's `nest_service` exists.

- [ ] **Step 9: Prove the tests can fail (mutation pass)**

| Mutation | Expected | Result |
|---|---|---|
| Delete `.nest_service("/images", images)` from `lib.rs` | `serves_a_file_from_the_images_directory` fails (404) | |
| Change `ServeDir::new(state.images_dir.clone())` to `ServeDir::new(".")` | `serves_a_file_from_the_images_directory` fails | |
| Remove `create_dir_all(&images_dir)` from `spawn_app` | `serves_a_file_...` fails, or `image_count` panics | |

Restore each mutation before moving on. The traversal test is a guard against a future
hand-rolled handler, not against today's `ServeDir` — say so honestly if no mutation of
today's code makes it red.

- [ ] **Step 10: Full gate and commit**

```bash
cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build
git add backend/Cargo.toml Cargo.lock backend/src/state.rs backend/src/config.rs \
        backend/src/lib.rs backend/src/main.rs backend/tests/common/mod.rs backend/tests/images.rs
git commit -m "feat(api): serve data/images at /images, with images_dir in AppState"
```

---

## Task 2: `images.rs` — what we accept and what we call it on disk

**Goal:** A pure, dependency-light module holding the upload policy — the size cap, the
magic-byte type check, and the content-addressed filename — unit-tested without a request or
a database.

**Files:**
- Modify: `backend/Cargo.toml` (add `sha2`)
- Create: `backend/src/images.rs`
- Modify: `backend/src/lib.rs` (`pub mod images;`)

**Acceptance Criteria:**
- [ ] PNG, JPEG and WebP are each identified from their signature bytes
- [ ] A text file is rejected, and so is a file whose signature is truncated
- [ ] A RIFF container that is not WebP (e.g. a WAV) is rejected — the check reads the `WEBP` tag, not just `RIFF`
- [ ] `stored_name` returns 16 lowercase hex characters plus the extension of the sniffed type
- [ ] Identical bytes always produce the same name; different bytes produce different names
- [ ] The extension comes from the sniffed type, never from a caller-supplied filename

**Verify:** `cargo test images:: && cargo clippy --all-targets -- -D warnings` → all pass

**Steps:**

- [ ] **Step 1: Add the dependency**

In `backend/Cargo.toml`, under `[dependencies]`:

```toml
sha2 = "0.10"
```

This is the whole reason filenames can be content-addressed instead of random: no RNG crate,
no collision bookkeeping, and re-uploading the same diagram reuses the file already on disk.

- [ ] **Step 2: Write the failing tests — the bottom of `backend/src/images.rs`**

Create the file with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A file with a valid signature and `filler` bytes of arbitrary payload.
    /// The sniffer reads signatures, not pixels, so this is exactly as much
    /// file as the code under test can distinguish.
    fn with_signature(sig: &[u8], filler: usize) -> Vec<u8> {
        let mut v = sig.to_vec();
        v.resize(sig.len() + filler, 0xAB);
        v
    }

    const PNG_SIG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    const JPEG_SIG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];

    fn webp(tag: &[u8; 4]) -> Vec<u8> {
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&[0x24, 0x00, 0x00, 0x00]); // little-endian size
        v.extend_from_slice(tag);
        v
    }

    #[test]
    fn sniffs_the_three_accepted_types() {
        assert_eq!(sniff(&with_signature(PNG_SIG, 32)), Some(ImageType::Png));
        assert_eq!(sniff(&with_signature(JPEG_SIG, 32)), Some(ImageType::Jpeg));
        assert_eq!(sniff(&webp(b"WEBP")), Some(ImageType::Webp));
    }

    #[test]
    fn rejects_a_file_that_is_not_an_image() {
        assert_eq!(sniff(b"just some notes about k-means"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn rejects_a_truncated_signature() {
        // Seven of the PNG signature's eight bytes. A prefix check that was
        // one byte short would accept this.
        assert_eq!(sniff(&PNG_SIG[..7]), None);
    }

    #[test]
    fn rejects_a_riff_container_that_is_not_webp() {
        // A WAV file also starts "RIFF". Checking only the container would
        // let it through and write it out as `.webp`.
        assert_eq!(sniff(&webp(b"WAVE")), None);
        // RIFF with nothing after the size field is not long enough to judge.
        assert_eq!(sniff(b"RIFF\x24\x00\x00\x00"), None);
    }

    #[test]
    fn a_name_is_sixteen_hex_characters_plus_the_sniffed_extension() {
        let name = stored_name(&with_signature(PNG_SIG, 10), ImageType::Png);
        let (stem, ext) = name.rsplit_once('.').expect("name has an extension");
        assert_eq!(ext, "png");
        assert_eq!(stem.len(), 16);
        assert!(
            stem.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f')),
            "stem must be lowercase hex, got {stem}",
        );
    }

    #[test]
    fn identical_bytes_get_identical_names() {
        let a = with_signature(PNG_SIG, 64);
        assert_eq!(stored_name(&a, ImageType::Png), stored_name(&a.clone(), ImageType::Png));
    }

    #[test]
    fn different_bytes_get_different_names() {
        let a = with_signature(PNG_SIG, 64);
        let b = with_signature(PNG_SIG, 65);
        assert_ne!(stored_name(&a, ImageType::Png), stored_name(&b, ImageType::Png));
    }

    #[test]
    fn the_extension_follows_the_type_not_the_bytes_length() {
        let bytes = with_signature(JPEG_SIG, 8);
        assert!(stored_name(&bytes, ImageType::Jpeg).ends_with(".jpg"));
        assert!(stored_name(&bytes, ImageType::Webp).ends_with(".webp"));
    }
}
```

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test images::`
Expected: compile failure — `sniff`, `stored_name` and `ImageType` do not exist yet.

- [ ] **Step 4: Write the module — the top of `backend/src/images.rs`**

Put this **above** the `#[cfg(test)]` block:

```rust
//! Upload policy: what counts as an image, and what it is called on disk.
//!
//! Free of axum and sqlx on purpose, so the rules can be unit-tested without
//! a request or a database. `routes::images` is the thin HTTP wrapper.

use sha2::{Digest, Sha256};

/// 5 MiB. A diagram cropped out of a lecture slide is tens of kilobytes;
/// anything approaching this is a phone photo pasted in by accident.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// The types the app accepts, identified by signature rather than by the
/// client's `Content-Type` or the uploaded filename. Both of those are
/// caller-controlled, and nothing downstream re-checks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageType {
    Png,
    Jpeg,
    Webp,
}

impl ImageType {
    /// The extension written to disk. This set must stay in step with the
    /// `image_path` guard in `routes::cards::is_uploaded_image_path` — that
    /// guard rejects any path this function could not have produced.
    pub fn extension(self) -> &'static str {
        match self {
            ImageType::Png => "png",
            ImageType::Jpeg => "jpg",
            ImageType::Webp => "webp",
        }
    }
}

/// Identifies an image by its leading bytes.
///
/// A signature check, not a decode: it proves the file claims to be a PNG,
/// JPEG or WebP, not that the remainder is well-formed. That is the right
/// depth here, because the app never decodes these files — it writes them to
/// disk and lets the browser render them. What it does buy is that a `.png`
/// full of something else cannot be stored under a `.png` name.
pub fn sniff(bytes: &[u8]) -> Option<ImageType> {
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    if bytes.starts_with(PNG) {
        return Some(ImageType::Png);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageType::Jpeg);
    }
    // `RIFF` <4-byte little-endian size> `WEBP`. WAV and AVI are also RIFF
    // containers, so the tag at offset 8 is the part that matters.
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(ImageType::Webp);
    }
    None
}

/// Content-addressed filename: the first 8 bytes of the SHA-256 of the
/// contents as hex, plus the sniffed extension.
///
/// No RNG dependency, no collision bookkeeping, and uploading the same
/// diagram twice reuses the file already written. 64 bits of hash over a
/// personal card deck will not collide.
pub fn stored_name(bytes: &[u8], kind: ImageType) -> String {
    let digest = Sha256::digest(bytes);
    let mut stem = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write;
        let _ = write!(stem, "{byte:02x}");
    }
    format!("{stem}.{}", kind.extension())
}
```

- [ ] **Step 5: Register the module — `backend/src/lib.rs`**

Add, keeping the list alphabetical:

```rust
pub mod images;
```

- [ ] **Step 6: Run the tests and watch them pass**

Run: `cargo test images:: -- --nocapture`
Expected: 7 passed.

- [ ] **Step 7: Prove the tests can fail (mutation pass)**

| Mutation | Expected | Result |
|---|---|---|
| Change the PNG constant to the first 7 bytes only | `rejects_a_truncated_signature` fails | |
| Drop the `&bytes[8..12] == b"WEBP"` clause, keeping `starts_with(b"RIFF")` | `rejects_a_riff_container_that_is_not_webp` fails | |
| Change `.take(8)` to `.take(4)` | `a_name_is_sixteen_hex_characters...` fails on length | |
| Return a constant name from `stored_name` | `different_bytes_get_different_names` fails | |
| Swap `ImageType::Jpeg => "jpg"` to `"jpeg"` | `the_extension_follows_the_type...` fails | |

- [ ] **Step 8: Commit**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
git add backend/Cargo.toml Cargo.lock backend/src/images.rs backend/src/lib.rs
git commit -m "feat(api): image upload policy — magic-byte sniffing and content-addressed names"
```

---

## Task 3: `POST /api/images`

**Goal:** A working upload endpoint that writes an accepted image into the images directory
and returns its stored path, rejecting everything else through the standard error envelope.

**Files:**
- Modify: `backend/src/error.rs` (add `AppError::Internal`)
- Create: `backend/src/routes/images.rs`
- Modify: `backend/src/routes/mod.rs`
- Modify: `backend/tests/common/mod.rs` (a multipart helper)
- Modify: `backend/tests/images.rs`

**Acceptance Criteria:**
- [ ] A valid PNG returns 201 and `{ "path": "images/<16 hex>.png" }`, and the file is on disk at that path
- [ ] The stored extension comes from the bytes, not the uploaded filename
- [ ] Uploading identical bytes twice returns the same path and leaves exactly one file
- [ ] A non-image is rejected 422 with `fields[0].field == "file"` and writes no file
- [ ] An oversize upload is rejected with the envelope (not axum's raw `text/plain` 413) and writes no file
- [ ] A multipart body with no `file` part is rejected 422 naming `file`
- [ ] The endpoint issues no query against the `cards` table

**Verify:** `cargo test --test images && cargo clippy --all-targets -- -D warnings` → all pass

**Steps:**

- [ ] **Step 1: Add an internal-error variant — `backend/src/error.rs`**

A failed filesystem write has no useful detail for the client, and `AppError::Db` is the
wrong shape for it. Add to the `AppError` enum, after `UnsupportedMediaType`:

```rust
    /// A server-side failure with nothing useful to tell the client (a
    /// refused filesystem write, say). Always logged at the call site,
    /// because this variant deliberately carries no detail onwards.
    #[error("internal error")]
    Internal,
```

and to the `match` in `parts()`, before the `AppError::Db` arm:

```rust
            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorBody { error: "internal", message: "Something went wrong".into(),
                            fields: vec![] },
            ),
```

Add this test to `error.rs`'s `mod tests`:

```rust
    #[test]
    fn internal_is_500_with_the_envelope_and_no_detail() {
        let (status, body) = AppError::Internal.parts();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "internal");
        assert!(body.fields.is_empty());
        assert_eq!(body.message, "Something went wrong", "must not leak the cause");
    }
```

- [ ] **Step 2: Add a multipart helper — `backend/tests/common/mod.rs`**

Inside `impl TestApp`:

```rust
    /// A multipart POST carrying a single part. Hand-rolled rather than
    /// pulled from a crate: the boundary format is four lines and the test
    /// harness is otherwise dependency-free.
    ///
    /// `field` is the part name, so a test can send something other than
    /// `file` and check the endpoint notices.
    pub async fn post_file(
        &self, uri: &str, field: &str, filename: &str, bytes: &[u8],
    ) -> (StatusCode, Value) {
        const BOUNDARY: &str = "XTESTBOUNDARYX";

        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

        let req = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", format!("multipart/form-data; boundary={BOUNDARY}"))
            .body(Body::from(body))
            .unwrap();

        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }
```

- [ ] **Step 3: Write the failing tests — append to `backend/tests/images.rs`**

```rust
const PNG_SIG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const JPEG_SIG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];

/// A file with a real signature and `filler` bytes of payload. The server
/// checks signatures, not pixels, so this is a genuine test input.
fn image(sig: &[u8], filler: usize) -> Vec<u8> {
    let mut v = sig.to_vec();
    v.resize(sig.len() + filler, 0xAB);
    v
}

#[tokio::test]
async fn uploads_a_png_and_serves_it_back() {
    let app = common::spawn_app().await;
    let bytes = image(PNG_SIG, 64);

    let (status, body) = app.post_file("/api/images", "file", "diagram.png", &bytes).await;
    assert_eq!(status, StatusCode::CREATED);

    let path = body["path"].as_str().expect("response carries a path");
    let name = path.strip_prefix("images/").expect("path is under images/");
    assert!(name.ends_with(".png"), "got {path}");
    assert_eq!(name.len(), "0123456789abcdef.png".len(), "16 hex characters plus .png");
    assert!(app.images_dir.join(name).exists(), "the file was not written");

    // The stored path is also the URL, minus the leading slash.
    let (status, served) = app.get_raw(&format!("/{path}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(served, bytes, "the bytes served back are the bytes uploaded");
}

#[tokio::test]
async fn the_extension_comes_from_the_bytes_not_the_filename() {
    let app = common::spawn_app().await;

    let (status, body) = app
        .post_file("/api/images", "file", "diagram.png", &image(JPEG_SIG, 32))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        body["path"].as_str().unwrap().ends_with(".jpg"),
        "a JPEG named .png must be stored as .jpg, got {}", body["path"],
    );
}

#[tokio::test]
async fn identical_bytes_reuse_one_file() {
    let app = common::spawn_app().await;
    let bytes = image(PNG_SIG, 100);

    let (_, first) = app.post_file("/api/images", "file", "a.png", &bytes).await;
    let (_, second) = app.post_file("/api/images", "file", "b.png", &bytes).await;

    assert_eq!(first["path"], second["path"], "content-addressed names must match");
    assert_eq!(app.image_count().await, 1, "the same bytes must not be stored twice");
}

#[tokio::test]
async fn different_images_get_different_paths() {
    let app = common::spawn_app().await;

    let (_, a) = app.post_file("/api/images", "file", "a.png", &image(PNG_SIG, 10)).await;
    let (_, b) = app.post_file("/api/images", "file", "b.png", &image(PNG_SIG, 11)).await;

    assert_ne!(a["path"], b["path"]);
    assert_eq!(app.image_count().await, 2);
}

#[tokio::test]
async fn rejects_a_file_that_is_not_an_image() {
    let app = common::spawn_app().await;

    let (status, body) = app
        .post_file("/api/images", "file", "notes.png", b"just some notes about k-means")
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"][0]["field"], "file", "the editor renders this beside the picker");
    assert_eq!(app.image_count().await, 0, "a rejected upload must write nothing");
}

#[tokio::test]
async fn rejects_an_oversize_image_through_the_envelope() {
    let app = common::spawn_app().await;
    // One byte past the cap, with a valid signature, so size is the only
    // reason it can be refused.
    let bytes = image(PNG_SIG, 5 * 1024 * 1024 + 1 - PNG_SIG.len());

    let (status, body) = app.post_file("/api/images", "file", "huge.png", &bytes).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "not axum's raw 413");
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"][0]["field"], "file");
    assert_eq!(app.image_count().await, 0);
}

#[tokio::test]
async fn rejects_a_multipart_body_with_no_file_part() {
    let app = common::spawn_app().await;

    let (status, body) = app
        .post_file("/api/images", "notthefield", "diagram.png", &image(PNG_SIG, 16))
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "file");
    assert_eq!(app.image_count().await, 0);
}

#[tokio::test]
async fn an_upload_touches_no_card() {
    // The spec's "a rejected upload leaves the card intact" is true here by
    // construction — the endpoint has no access to a card id at all. This
    // pins that: if someone later gives the upload route a card to mutate,
    // the count stops being zero.
    let app = common::spawn_app().await;
    let (_, deck) = app.post("/api/decks", serde_json::json!({ "name": "Clustering" })).await;
    let deck_id = deck["id"].as_i64().unwrap();
    let (_, card) = app
        .post("/api/cards", serde_json::json!({
            "deck_id": deck_id, "kind": "flashcard",
            "prompt_md": "Define support.", "answer_md": "A fraction of transactions."
        }))
        .await;
    let before = card["updated_at"].as_str().unwrap().to_string();

    let (status, _) = app.post_file("/api/images", "file", "d.png", &image(PNG_SIG, 8)).await;
    assert_eq!(status, StatusCode::CREATED);

    let (_, after) = app.get(&format!("/api/cards/{}", card["id"].as_i64().unwrap())).await;
    assert_eq!(after["updated_at"], before, "an upload must not touch any card");
    assert!(after["image_path"].is_null(), "an upload must not attach itself to a card");
}
```

- [ ] **Step 4: Run them and watch them fail**

Run: `cargo test --test images`
Expected: the eight new tests fail with 404 — `/api/images` does not exist yet.

- [ ] **Step 5: Write the handler — `backend/src/routes/images.rs`**

```rust
//! Image upload.
//!
//! A standalone endpoint rather than the design's original
//! `POST /api/cards/:id/image`: the editor has to be able to upload *before*
//! the card exists, because for a diagram card the image IS the prompt, and a
//! `short_answer` card cannot be saved without an accepted answer — which the
//! author is writing while looking at the image. See
//! `docs/mitis/specs/2026-08-27-part2b-images-markdown-design.md` §1, and note
//! the accepted cost: an upload whose card is never saved leaves an orphan
//! file. Nothing sweeps them yet.
//!
//! Because this route never touches the `cards` table, the spec's "a rejected
//! upload leaves the card intact" holds by construction rather than by care.

use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::images::{sniff, stored_name, MAX_IMAGE_BYTES};
use crate::state::AppState;

#[derive(Serialize)]
pub struct UploadedImage {
    /// Relative to the data directory — `images/<16 hex>.<ext>` — and stored
    /// verbatim in `cards.image_path`. Prefix it with `/` for the URL.
    pub path: String,
}

async fn upload(
    State(st): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<UploadedImage>)> {
    let mut data: Option<axum::body::Bytes> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!(error = %e, "malformed multipart upload");
        AppError::BadRequest("The upload could not be read".into())
    })? {
        if field.name() == Some("file") {
            data = Some(field.bytes().await.map_err(|e| {
                tracing::warn!(error = %e, "could not read the upload field");
                AppError::BadRequest("The upload could not be read".into())
            })?);
            break;
        }
    }

    // Every rejection below names `file`, because that is the form control the
    // editor renders the message beside.
    let bytes = data
        .ok_or_else(|| AppError::validation([("file", "Choose an image to upload")]))?;

    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(AppError::validation([("file", "That image is larger than 5 MB")]));
    }

    let kind = sniff(&bytes).ok_or_else(|| {
        AppError::validation([("file", "That file is not a PNG, JPEG or WebP image")])
    })?;

    let name = stored_name(&bytes, kind);
    let dest = st.images_dir.join(&name);

    // The name is a hash of the contents, so a file already sitting there has
    // identical bytes and rewriting it would be churn.
    if !dest.exists() {
        tokio::fs::write(&dest, &bytes).await.map_err(|e| {
            tracing::error!(error = ?e, path = ?dest, "could not write the uploaded image");
            AppError::Internal
        })?;
    }

    Ok((StatusCode::CREATED, Json(UploadedImage { path: format!("images/{name}") })))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/images",
        post(upload)
            // Axum's default body limit is 2 MiB and rejects with a raw
            // `text/plain` 413, which would be the one failure in the app that
            // does not arrive as the error envelope. Raise it clear of our own
            // 5 MiB check so the handler is what actually refuses an oversize
            // upload; this stays as the backstop for a deliberately enormous
            // body that we should not buffer at all.
            .layer(DefaultBodyLimit::max(MAX_IMAGE_BYTES + 1024 * 1024)),
    )
}
```

- [ ] **Step 6: Mount it — `backend/src/routes/mod.rs`**

```rust
pub mod cards;
pub mod decks;
pub mod health;
pub mod images;
pub mod modules;

use axum::Router;
use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(modules::router())
        .merge(decks::router())
        .merge(cards::router())
        .merge(images::router())
}
```

- [ ] **Step 7: Run the tests and watch them pass**

Run: `cargo test --test images`
Expected: all 11 tests in the file pass.

- [ ] **Step 8: Prove the tests can fail (mutation pass)**

| Mutation | Expected | Result |
|---|---|---|
| Use the uploaded filename's extension instead of `kind.extension()` | `the_extension_comes_from_the_bytes...` fails | |
| Delete the `bytes.len() > MAX_IMAGE_BYTES` check | `rejects_an_oversize_image_through_the_envelope` fails (413 or 201, not 422) | |
| Remove the `DefaultBodyLimit` layer | `rejects_an_oversize_image...` fails with axum's 413 instead of the envelope | |
| Delete the `sniff(...).ok_or_else(...)` guard | `rejects_a_file_that_is_not_an_image` fails | |
| Change the rejection field name from `"file"` to `"image"` | the three rejection tests fail | |
| Make `stored_name` include the filename | `identical_bytes_reuse_one_file` fails (two files) | |

- [ ] **Step 9: Full gate and commit**

```bash
cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build
git add backend/src/error.rs backend/src/routes/images.rs backend/src/routes/mod.rs \
        backend/tests/common/mod.rs backend/tests/images.rs
git commit -m "feat(api): POST /api/images — validated, content-addressed image upload"
```

---

## Task 4: `image_path` on cards

**Goal:** A card can be created and updated with an `image_path`, validated to be a path this
server actually issued, under the existing full-replace rule.

**Files:**
- Modify: `backend/src/routes/cards.rs`
- Modify: `backend/tests/cards.rs`
- Modify: `.sqlx/` (regenerated cache — commit it)

**Acceptance Criteria:**
- [ ] `POST /api/cards` accepts `image_path` and the value comes back on the card
- [ ] `PATCH /api/cards/:id` sets it, and a PATCH that omits it clears it — the full-replace rule
- [ ] A path the server could not have issued is rejected 422 naming `image_path`
- [ ] The accepted shape is exactly `images/<16 lowercase hex>.<png|jpg|webp>`
- [ ] An empty-string `image_path` is treated as absent, not stored
- [ ] The sqlx offline cache is regenerated and committed

**Verify:** `cargo test --test cards && cargo clippy --all-targets -- -D warnings` → all pass

**Steps:**

- [ ] **Step 1: Write the failing tests — append to `backend/tests/cards.rs`**

```rust
/// A path shaped exactly like one `POST /api/images` returns. The card
/// endpoints deliberately do not check that the file exists: a swept or
/// hand-deleted file should render as a broken image, not block every save of
/// the card that references it.
const UPLOADED: &str = "images/0123456789abcdef.png";

#[tokio::test]
async fn image_path_round_trips_through_create_and_get() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Clustering").await;

    let (status, card) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "short_answer",
            "prompt_md": "Name the linkage shown in the dendrogram.",
            "image_path": UPLOADED,
            "accepted": [{ "text": "single linkage", "is_primary": true }]
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(card["image_path"], UPLOADED);

    let (_, fetched) = app.get(&format!("/api/cards/{}", card["id"].as_i64().unwrap())).await;
    assert_eq!(fetched["image_path"], UPLOADED, "the stored path survives a re-read");
}

#[tokio::test]
async fn a_patch_omitting_image_path_clears_it() {
    // Cards PATCH is a full replace, not the decks absent-vs-null dance: the
    // editor always holds the whole card, so an omitted optional means null.
    let app = common::spawn_app().await;
    let d = deck(&app, "Clustering").await;

    let (_, card) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "flashcard", "prompt_md": "Define linkage.",
            "answer_md": "How inter-cluster distance is measured.",
            "image_path": UPLOADED
        }))
        .await;
    let id = card["id"].as_i64().unwrap();
    assert_eq!(card["image_path"], UPLOADED);

    let (status, patched) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "flashcard", "prompt_md": "Define linkage.",
            "answer_md": "How inter-cluster distance is measured."
        }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(patched["image_path"].is_null(), "an omitted image_path must clear it");
}

#[tokio::test]
async fn a_patch_can_set_an_image_on_a_card_that_had_none() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Clustering").await;
    let (_, card) = app.post("/api/cards", mc(d)).await;
    let id = card["id"].as_i64().unwrap();
    assert!(card["image_path"].is_null());

    let (status, patched) = app
        .patch(&format!("/api/cards/{id}"), json!({
            "kind": "mc_single",
            "prompt_md": "Which linkage merges the two closest points?",
            "image_path": "images/fedcba9876543210.webp",
            "choices": [
                { "text_md": "Single",   "is_correct": true  },
                { "text_md": "Complete", "is_correct": false }
            ]
        }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patched["image_path"], "images/fedcba9876543210.webp");
}

#[tokio::test]
async fn rejects_an_image_path_this_server_did_not_issue() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Clustering").await;

    // Every one of these is a path `POST /api/images` cannot produce. The
    // value ends up in the browser as a URL, so the shape is the guard.
    let rejected = [
        ("../../etc/passwd", "escapes the directory"),
        ("images/../test.db", "traverses out of images/"),
        ("images/short.png", "stem is not 16 characters"),
        ("images/0123456789abcdeg.png", "g is not hex"),
        ("images/0123456789ABCDEF.png", "uppercase hex is not what we emit"),
        ("images/0123456789abcdef.gif", "gif is not an accepted type"),
        ("images/0123456789abcdef", "no extension"),
        ("http://example.com/x.png", "not a local path at all"),
        ("uploads/0123456789abcdef.png", "wrong directory"),
    ];

    for (path, why) in rejected {
        let (status, body) = app
            .post("/api/cards", json!({
                "deck_id": d, "kind": "flashcard", "prompt_md": "Q", "answer_md": "A",
                "image_path": path
            }))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path} ({why}) was accepted");
        assert!(
            body["fields"].as_array().unwrap().iter()
                .any(|f| f["field"] == "image_path"),
            "{path} ({why}) was rejected but not against image_path: {body}",
        );
    }

    assert_eq!(app.count("SELECT COUNT(*) FROM cards").await, 0, "none were written");
}

#[tokio::test]
async fn an_empty_image_path_is_treated_as_absent() {
    let app = common::spawn_app().await;
    let d = deck(&app, "Clustering").await;

    let (status, card) = app
        .post("/api/cards", json!({
            "deck_id": d, "kind": "flashcard", "prompt_md": "Q", "answer_md": "A",
            "image_path": ""
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "an empty string is 'no image', not an error");
    assert!(card["image_path"].is_null());
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test --test cards`
Expected: FAIL — `image_path` is an unknown field (`CardInput` is `deny_unknown_fields`), so
the create calls return 422 "This field is not recognized".

- [ ] **Step 3: Accept the field — `backend/src/routes/cards.rs`**

In `CardInput`, after `explanation_md`:

```rust
    #[serde(default)]
    pub image_path: Option<String>,
```

and in `ValidCard`, after `explanation_md`:

```rust
    pub image_path: Option<String>,
```

- [ ] **Step 4: Add the shape guard**

Put this free function just above `validate`:

```rust
/// True only for a path `POST /api/images` could have produced:
/// `images/<16 lowercase hex>.<png|jpg|webp>`.
///
/// The server assigns every legitimate value of this field, so anything
/// outside that shape did not come from an upload. It is worth a guard rather
/// than a shrug because the string is handed straight back to the browser as
/// a URL. Hand-rolled instead of pulling in `regex`: this is the only pattern
/// the codebase matches, and the crate would be the larger change. The
/// extension list must stay in step with `images::ImageType::extension`.
fn is_uploaded_image_path(p: &str) -> bool {
    let Some(rest) = p.strip_prefix("images/") else { return false };
    let Some((stem, ext)) = rest.rsplit_once('.') else { return false };

    stem.len() == 16
        && stem.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        && ["png", "jpg", "webp"].contains(&ext)
}
```

Then, inside `validate`, immediately after the `explanation_md` binding:

```rust
    // An empty string means "no image" — the editor clears the field rather
    // than deleting the key — so it is filtered out before the shape check.
    let image_path = input.image_path.as_deref().map(str::trim)
        .filter(|s| !s.is_empty()).map(str::to_string);
    if image_path.as_deref().is_some_and(|p| !is_uploaded_image_path(p)) {
        push("image_path", "That is not an uploaded image");
    }
```

and add it to the returned `ValidCard`:

```rust
    Ok(ValidCard {
        kind: input.kind, prompt_md, image_path, answer_md, explanation_md, choices, accepted,
    })
```

- [ ] **Step 5: Write it to the database**

In `create`, extend the INSERT (note the extra `?` and the extra bind, in matching order):

```rust
    let id = sqlx::query_scalar!(
        r#"INSERT INTO cards (deck_id, kind, prompt_md, image_path, answer_md, explanation_md)
           VALUES (?, ?, ?, ?, ?, ?) RETURNING id AS "id!: i64""#,
        deck_id, valid.kind, valid.prompt_md, valid.image_path, valid.answer_md,
        valid.explanation_md
    )
```

In `patch`, extend the UPDATE:

```rust
    sqlx::query!(
        r#"UPDATE cards
              SET kind = ?, prompt_md = ?, image_path = ?, answer_md = ?, explanation_md = ?,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = ?"#,
        valid.kind, valid.prompt_md, valid.image_path, valid.answer_md,
        valid.explanation_md, id
    )
```

Keep the placeholders as plain repeated `?`. Numbered `?1`/`?2` break the macro's
occurrence-counting binding, and the resulting compile error on this machine's nightly can be
mistaken for the self-recovering ICE.

- [ ] **Step 6: Regenerate the sqlx cache**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
DATABASE_URL="sqlite://data/quizapp.db?mode=rwc" cargo sqlx prepare --workspace
```

- [ ] **Step 7: Run the tests and watch them pass**

Run: `cargo test --test cards`
Expected: all pass, including the five new ones.

- [ ] **Step 8: Prove the tests can fail (mutation pass)**

| Mutation | Expected | Result |
|---|---|---|
| Delete the `push("image_path", …)` guard | `rejects_an_image_path_this_server_did_not_issue` fails | |
| Change `stem.len() == 16` to `stem.len() >= 4` | same test fails on `images/short.png` | |
| Drop the `is_ascii_digit() \|\| matches!(b, b'a'..=b'f')` check | same test fails on the `…abcdeg` and uppercase cases | |
| Add `"gif"` to the extension list | same test fails on the `.gif` case | |
| Drop `.filter(\|s\| !s.is_empty())` | `an_empty_image_path_is_treated_as_absent` fails | |
| Remove `image_path` from the UPDATE in `patch` | `a_patch_omitting_image_path_clears_it` and `a_patch_can_set_an_image...` fail | |
| Remove `image_path` from the INSERT in `create` | `image_path_round_trips_through_create_and_get` fails | |

- [ ] **Step 9: Full gate and commit**

```bash
cargo test && cargo clippy --all-targets -- -D warnings && SQLX_OFFLINE=true cargo build
git add backend/src/routes/cards.rs backend/tests/cards.rs .sqlx
git commit -m "feat(api): accept and validate image_path on card create and patch"
```

---

## Task 5: the shared `<Markdown>` component

**Goal:** One markdown-plus-KaTeX renderer, with the CSS that makes bullets and headings
survive Tailwind's preflight, and KaTeX's fonts served locally.

**Files:**
- Modify: `frontend/package.json` (four dependencies)
- Create: `frontend/src/components/Markdown.tsx`
- Modify: `frontend/src/styles/globals.css`

**Acceptance Criteria:**
- [ ] `react-markdown`, `remark-math`, `rehype-katex` and `katex` are installed with pnpm
- [ ] KaTeX's stylesheet is imported from the npm package, not a CDN
- [ ] Raw HTML in card text is not rendered as HTML (no `rehype-raw`)
- [ ] Bullets, numbered lists and headings render — Tailwind's preflight strips them by default
- [ ] Display maths scrolls horizontally inside its own container rather than widening the page
- [ ] `pnpm build` succeeds and the KaTeX font files appear in `frontend/dist/assets`

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build && ls dist/assets | grep -i katex` → build succeeds and font files are listed

**Steps:**

- [ ] **Step 1: Install the dependencies**

```bash
cd frontend && pnpm add react-markdown remark-math rehype-katex katex
```

**pnpm, never npm** — `packageManager` is pinned. If a `package-lock.json` appears, delete it;
something went wrong.

- [ ] **Step 2: Write the component — `frontend/src/components/Markdown.tsx`**

```tsx
import ReactMarkdown from 'react-markdown'
import remarkMath from 'remark-math'
import rehypeKatex from 'rehype-katex'
import { cn } from '@/lib/utils'

// KaTeX ships its own fonts inside the npm package, so importing the
// stylesheet this way keeps maths rendering correctly offline and on a
// LAN-only phone. Do NOT swap this for a CDN <link>: globals.css already
// pulls Quicksand and Inter over the network and that is a known defect
// deferred to build step 8, not a pattern to copy.
import 'katex/dist/katex.min.css'

type Props = {
  /** Markdown with inline LaTeX, as written in Obsidian: `$X, Y, Z$`, `$10\ 000$`. */
  children: string
  className?: string
}

/**
 * The app's only markdown renderer.
 *
 * The card list, the card editor's preview and (in Part 3) the session runner
 * all render through here, so what a card looks like while it is written and
 * what it looks like while it is answered cannot drift apart. Part 2a
 * deliberately shipped raw text everywhere so this would be built exactly
 * once — if you need different behaviour somewhere, add a prop here rather
 * than a second renderer.
 *
 * Raw HTML stays disabled. That is react-markdown's default and there is no
 * `rehype-raw`; card text is markdown, not a template.
 *
 * GitHub-flavoured extras (tables, strikethrough, autolinks) need `remark-gfm`
 * and are deliberately not installed. Add it when a real card needs one.
 */
export function Markdown({ children, className }: Props) {
  return (
    <div className={cn('markdown', className)}>
      <ReactMarkdown remarkPlugins={[remarkMath]} rehypePlugins={[rehypeKatex]}>
        {children}
      </ReactMarkdown>
    </div>
  )
}
```

- [ ] **Step 3: Add the element styles — the end of `frontend/src/styles/globals.css`**

```css
/* Everything rendered by <Markdown>.
   Tailwind's preflight resets list markers, heading sizes and margins to
   nothing, which is right for app chrome and wrong for prose: without this
   block a card's bullets render as an unindented run of lines. Written out
   here rather than pulling in @tailwindcss/typography, whose defaults would
   fight the Bibble type scale for the handful of elements a card actually
   uses. Colours come from the theme tokens, so both palettes are covered. */
.markdown > :first-child { margin-top: 0; }
.markdown > :last-child { margin-bottom: 0; }
.markdown p { margin: 0.5em 0; }
.markdown ul { list-style: disc; padding-left: 1.25rem; margin: 0.5em 0; }
.markdown ol { list-style: decimal; padding-left: 1.5rem; margin: 0.5em 0; }
.markdown li { margin: 0.125em 0; }
.markdown li > ul, .markdown li > ol { margin: 0.125em 0; }
.markdown h1, .markdown h2, .markdown h3 { font-weight: 700; margin: 0.75em 0 0.25em; }
.markdown h1 { font-size: 1.25rem; }
.markdown h2 { font-size: 1.125rem; }
.markdown h3 { font-size: 1rem; }
.markdown strong { font-weight: 700; }
.markdown em { font-style: italic; }
.markdown a { text-decoration: underline; }
.markdown blockquote {
  border-left: 3px solid var(--border);
  padding-left: 0.75rem;
  color: var(--muted-foreground);
}
.markdown code {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 0.9em;
  background: var(--muted);
  padding: 0.1em 0.3em;
  border-radius: 0.25rem;
}
.markdown pre {
  background: var(--muted);
  padding: 0.75rem;
  border-radius: 0.5rem;
  overflow-x: auto;
}
.markdown pre code { background: none; padding: 0; }

/* KaTeX inherits `color`, so it follows both Bibble palettes without help.
   Display maths, though, is a wide inline-block: in a narrow card row it must
   scroll inside itself rather than push the page sideways. */
.markdown .katex-display { overflow-x: auto; overflow-y: hidden; padding: 0.25rem 0; }
```

- [ ] **Step 4: Verify the build and the local fonts**

```bash
cd frontend && pnpm exec tsc --noEmit && pnpm build
ls dist/assets | grep -i katex
```

Expected: the build succeeds and several `KaTeX_*.woff2` files are listed. **If nothing is
listed, the fonts are not being bundled** — check that the `katex/dist/katex.min.css` import
is present and was not replaced with a CDN link.

Then confirm nothing reaches the network for maths:

```bash
grep -rn "cdn\|unpkg\|jsdelivr" src/components/Markdown.tsx src/styles/globals.css
```

Expected: no match other than the pre-existing Google Fonts `@import` line in `globals.css`,
which is the known deferred defect.

- [ ] **Step 5: Commit**

```bash
cd .. && git add frontend/package.json frontend/pnpm-lock.yaml \
  frontend/src/components/Markdown.tsx frontend/src/styles/globals.css
git commit -m "feat(ui): add the shared Markdown component with local KaTeX"
```

---

## Task 6: `<CardImage>` and a card list that renders markdown

**Goal:** The deck's card list shows each prompt as rendered markdown across as many lines as
it needs, with a thumbnail that opens the full image in a lightbox.

**Files:**
- Create: `frontend/src/components/CardImage.tsx`
- Modify: `frontend/src/pages/DeckPage.tsx`

**Acceptance Criteria:**
- [ ] `firstLine()` is gone from `DeckPage`; the full prompt renders through `<Markdown>`
- [ ] Rows are unclamped and grow to fit multi-line content, including bullets
- [ ] A card with an `image_path` shows a ~64px-tall thumbnail beside its prompt
- [ ] Clicking the thumbnail opens the image full-size in a dialog, closable with Escape
- [ ] The thumbnail is a real button with an accessible label, not a bare clickable `<img>`
- [ ] The kind badges and the edit/archive buttons stay top-aligned as rows grow

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build` → succeeds, plus the manual check in Step 5

**Steps:**

- [ ] **Step 1: Write the component — `frontend/src/components/CardImage.tsx`**

```tsx
import { useState } from 'react'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { cn } from '@/lib/utils'

type Props = {
  /** `image_path` exactly as stored on the card: `images/<hash>.<ext>`. */
  path: string
  /** Alt text. Pass the prompt, so a screen reader gets the question rather than "image". */
  alt: string
  className?: string
}

/**
 * A card's image: a small thumbnail that opens full-size in a dialog.
 *
 * The thumbnail is the same file scaled by CSS. There is no resizing pipeline
 * and, for hand-cropped diagrams pulled out of lecture slides, there does not
 * need to be — the whole file is tens of kilobytes.
 *
 * `image_path` is stored relative to the data directory and the server serves
 * that directory at `/images`, so the URL is the path with a leading slash.
 * That is the only place the two halves meet; keep it here rather than
 * building URLs at each call site.
 */
export function CardImage({ path, alt, className }: Props) {
  const [open, setOpen] = useState(false)
  const src = `/${path}`

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label={`Enlarge image: ${alt}`}
        title="Enlarge image"
        className={cn(
          'shrink-0 overflow-hidden rounded-md border bg-muted/30',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
          className,
        )}
      >
        <img src={src} alt={alt} className="h-16 w-auto max-w-32 object-contain" />
      </button>

      <Dialog open={open} onOpenChange={setOpen}>
        {/* aria-describedby is cleared because there is no description to
            point at — Radix warns about a missing one otherwise. */}
        <DialogContent className="sm:max-w-3xl" aria-describedby={undefined}>
          <DialogTitle className="sr-only">{alt}</DialogTitle>
          <img src={src} alt={alt} className="max-h-[75vh] w-full object-contain" />
        </DialogContent>
      </Dialog>
    </>
  )
}
```

- [ ] **Step 2: Render markdown in the list — `frontend/src/pages/DeckPage.tsx`**

Remove the `firstLine` helper entirely (lines 11–13) and add the two imports:

```tsx
import { Markdown } from '@/components/Markdown'
import { CardImage } from '@/components/CardImage'
```

Then replace the `<li>` body. The old version was one flex row of centred items with
`truncate`; the new one is top-aligned, because a row is now as tall as its content:

```tsx
          <li
            key={c.id}
            className={`flex items-start justify-between gap-3 rounded-lg border p-3 ${
              c.archived ? 'opacity-60' : ''
            }`}
          >
            <div className="flex min-w-0 flex-1 items-start gap-3">
              <div className="flex shrink-0 flex-wrap items-center gap-2">
                <Badge variant="outline">{KIND_LABEL[c.kind]}</Badge>
                {c.archived && <Badge variant="secondary">Archived</Badge>}
              </div>
              {c.image_path !== null && (
                <CardImage path={c.image_path} alt={c.prompt_md} />
              )}
              {/* Unclamped on purpose: a truncated single line cannot render
                  markdown without a half-open `$…$` or a stray list marker
                  looking broken. The cost is a long page for a 100+ card deck,
                  which is a deliberate, recorded trade-off. */}
              <Markdown className="min-w-0 flex-1">{c.prompt_md}</Markdown>
            </div>
            <div className="flex shrink-0 items-center gap-1">
```

Leave the action-button block that follows exactly as it is.

- [ ] **Step 3: Typecheck and build**

```bash
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

Expected: clean. A "`firstLine` is declared but never used" error means Step 2's deletion was
missed.

- [ ] **Step 4: Prove the rendering path is real**

There is no frontend test framework — a deliberate spec decision — so this is a manual check
with recorded output. With the API running (`cargo run`) and `pnpm dev` up, create a card
whose prompt is real Obsidian input:

```
Given $X, Y, Z$ independent with $10\ 000$ samples:

- Gini is impurity-based
- Entropy is $-\sum p_i \log_2 p_i$
```

Then confirm and record: the maths renders as maths (not `$…$`), the bullets have markers and
indentation, and the row is as tall as the content. **If the bullets have no markers, Task 5
Step 3's CSS is missing** — that is exactly the failure that block exists to prevent.

- [ ] **Step 5: Manual check, recorded**

| Check | Result |
|---|---|
| Prompt renders as markdown, maths as maths | |
| Bullets show markers and indent | |
| Row grows to fit; nothing is truncated | |
| Thumbnail appears for a card with an image | |
| Clicking the thumbnail opens the lightbox; Escape closes it | |
| Badges and action buttons stay top-aligned on a tall row | |

If the Chrome extension is not connected, say so and move these to the outstanding list in
Task 9 rather than reporting them as passed.

- [ ] **Step 6: Commit**

```bash
cd .. && git add frontend/src/components/CardImage.tsx frontend/src/pages/DeckPage.tsx
git commit -m "feat(ui): render card prompts as markdown, with an image thumbnail and lightbox"
```

---

## Task 7: the client seam — `uploadImage`, `image_path`, and the dev proxy

**Goal:** The frontend can upload an image, gets upload failures as ordinary field errors, and
reaches `/images` in development.

**Files:**
- Modify: `frontend/src/lib/api.ts`
- Modify: `frontend/vite.config.ts`

**Acceptance Criteria:**
- [ ] `api.uploadImage(file)` posts multipart to `/api/images` and returns `{ path }`
- [ ] An upload failure throws the same `ApiError` as every other call, so `byField().file` renders inline
- [ ] The envelope-parsing logic is written once, not duplicated between `request` and `uploadImage`
- [ ] `CardInput` carries `image_path?: string | null`
- [ ] Vite proxies `/images` to the API in development, so thumbnails load at `localhost:5273`

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build` → succeeds

**Steps:**

- [ ] **Step 1: Extract the failure path — `frontend/src/lib/api.ts`**

Replace the `if (!res.ok) { … }` block inside `request` with a call to a shared helper, and
add the helper above `request`:

```ts
/**
 * Turns any non-2xx response into an ApiError carrying the envelope's
 * `fields`, so the caller can render them inline. Shared by `request` and
 * `uploadImage`: an upload failure must reach the editor in exactly the same
 * shape as a rejected save, because it lands in the same error slot.
 */
async function fail(res: Response): Promise<never> {
  const payload = await res.json().catch(() => null)
  throw new ApiError(
    res.status,
    payload?.message ?? `Request failed (${res.status})`,
    payload?.fields ?? [],
  )
}
```

and inside `request`:

```ts
  if (!res.ok) await fail(res)
```

- [ ] **Step 2: Add the upload call**

Below the `cardQueryString` helper:

```ts
export type UploadedImage = { path: string }

/**
 * Cannot go through `request()`: the body is FormData, and the browser must
 * set `content-type` itself so it can include the multipart boundary. Setting
 * it by hand produces a boundary-less header the server cannot parse.
 *
 * The returned `path` is relative — `images/<hash>.<ext>` — and is what gets
 * stored on the card. Prefix it with `/` for a URL; `<CardImage>` does that.
 */
async function uploadImage(file: File, signal?: AbortSignal): Promise<UploadedImage> {
  const form = new FormData()
  form.append('file', file)
  const res = await fetch('/api/images', { method: 'POST', body: form, signal })
  if (!res.ok) await fail(res)
  return (await res.json()) as UploadedImage
}
```

- [ ] **Step 3: Put `image_path` on the input type and export the call**

In `CardInput`, after `prompt_md`:

```ts
  /**
   * `images/<hash>.<ext>` from `uploadImage`, or null for no image. Send it
   * explicitly on every save: cards PATCH is a full replace, so an absent key
   * means null on the server.
   */
  image_path?: string | null
```

and in the `api` object, after `unarchiveCard`:

```ts
  uploadImage,
```

- [ ] **Step 4: Proxy `/images` in development — `frontend/vite.config.ts`**

```ts
  server: {
    port: 5273,
    proxy: {
      '/api': { target: 'http://127.0.0.1:3000', changeOrigin: true },
      // Uploaded images are served by the API, not by Vite. Without this the
      // dev server answers /images/... with index.html and every thumbnail is
      // a broken image.
      '/images': { target: 'http://127.0.0.1:3000', changeOrigin: true },
    },
  },
```

Port 5273, not 5173 — 5173 is permanently taken on this machine.

- [ ] **Step 5: Typecheck, build, commit**

```bash
cd frontend && pnpm exec tsc --noEmit && pnpm build && cd ..
git add frontend/src/lib/api.ts frontend/vite.config.ts
git commit -m "feat(ui): api.uploadImage, image_path on CardInput, and an /images dev proxy"
```

---

## Task 8: the editor — image control and the Edit/Preview toggle

**Goal:** An author can attach an image while writing the card, and flip the whole form
between source and rendered output without leaving the keyboard.

**Files:**
- Create: `frontend/src/components/card-editor/CardPreview.tsx`
- Modify: `frontend/src/pages/CardEditorPage.tsx`

**Acceptance Criteria:**
- [ ] Choosing a file uploads it immediately and shows the thumbnail plus a Remove button
- [ ] An upload failure renders inline beside the picker and leaves every other typed field untouched
- [ ] The same file can be chosen again after a failure (the input value is reset)
- [ ] `image_path` is sent explicitly on every save, `null` when there is none
- [ ] Save-and-next clears the image along with the rest of the form
- [ ] Edit/Preview toggles the whole form, via buttons and `⌘/Ctrl+P`, without triggering the browser's print dialog
- [ ] Preview renders prompt, image, choices with their correct marks, accepted answers with the primary marked, the flashcard answer, and the explanation
- [ ] Returning to Edit puts focus back on the prompt
- [ ] The action bar stays visible in both views

**Verify:** `cd frontend && pnpm exec tsc --noEmit && pnpm build` → succeeds, plus the manual checks in Step 6

**Steps:**

- [ ] **Step 1: Write the preview — `frontend/src/components/card-editor/CardPreview.tsx`**

```tsx
import { Check, Star } from 'lucide-react'
import type { AcceptedInput, CardKind, ChoiceInput } from '@/lib/api'
import { Markdown } from '@/components/Markdown'
import { CardImage } from '@/components/CardImage'

type Props = {
  kind: CardKind
  promptMd: string
  imagePath: string | null
  choices: ChoiceInput[]
  accepted: AcceptedInput[]
  answerMd: string
  explanationMd: string
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-2">
      <h2 className="text-sm font-medium text-muted-foreground">{title}</h2>
      {children}
    </section>
  )
}

/**
 * The card as it will read once saved. Rendered from the form's live state
 * rather than from a fetch, so it shows unsaved edits — that is the whole
 * point of checking a formula before committing to it.
 *
 * Everything here goes through <Markdown>, the app's single renderer, so the
 * preview cannot disagree with the card list or (in Part 3) the session
 * runner about how a card looks.
 */
export function CardPreview({
  kind, promptMd, imagePath, choices, accepted, answerMd, explanationMd,
}: Props) {
  return (
    <div className="space-y-5 rounded-lg border p-4">
      <Section title="Prompt">
        {imagePath !== null && <CardImage path={imagePath} alt="Card image" />}
        {promptMd.trim() === ''
          ? <p className="text-sm italic text-muted-foreground">No prompt yet.</p>
          : <Markdown>{promptMd}</Markdown>}
      </Section>

      {kind === 'mc_single' && (
        <Section title="Choices">
          <ul className="space-y-1">
            {choices.map((c, i) => (
              <li key={i} className="flex items-start gap-2">
                {c.is_correct ? (
                  <Check className="mt-1 size-4 shrink-0 text-primary" aria-label="Correct" />
                ) : (
                  <span className="mt-1 size-4 shrink-0" aria-hidden="true" />
                )}
                <Markdown className="min-w-0 flex-1">{c.text_md}</Markdown>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {kind === 'short_answer' && (
        <Section title="Accepted answers">
          <ul className="space-y-1">
            {accepted.map((a, i) => (
              <li key={i} className="flex items-start gap-2">
                {a.is_primary ? (
                  <Star className="mt-1 size-4 shrink-0 text-primary" aria-label="Primary" />
                ) : (
                  <span className="mt-1 size-4 shrink-0" aria-hidden="true" />
                )}
                {/* Accepted answers are compared as plain text, but they are
                    shown back to the student as the expected answer, so they
                    render like everything else. */}
                <Markdown className="min-w-0 flex-1">{a.text}</Markdown>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {kind === 'flashcard' && (
        <Section title="Answer">
          <Markdown>{answerMd}</Markdown>
        </Section>
      )}

      {explanationMd.trim() !== '' && (
        <Section title="Explanation">
          <Markdown>{explanationMd}</Markdown>
        </Section>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Add state and imports — `frontend/src/pages/CardEditorPage.tsx`**

Extend the React import and add the new component imports:

```tsx
import { useEffect, useRef, useState, type ChangeEvent, type KeyboardEvent } from 'react'
```

```tsx
import { CardImage } from '@/components/CardImage'
import { CardPreview } from '@/components/card-editor/CardPreview'
import { Input } from '@/components/ui/input'
```

Then, alongside the other `useState` calls in `CardEditorPageInner`:

```tsx
  const [imagePath, setImagePath] = useState<string | null>(null)
  const [imageBusy, setImageBusy] = useState(false)
  const [view, setView] = useState<'edit' | 'preview'>('edit')
```

- [ ] **Step 3: Load, send and clear the image**

In the `useEffect` that loads an existing card, after `setPromptMd(card.prompt_md)`:

```tsx
        setImagePath(card.image_path)
```

In `buildInput`, after the `prompt_md` line — sent unconditionally, including `null`, because
cards PATCH is a full replace and an absent key means "no image":

```tsx
    input.image_path = imagePath
```

In `saveAndNext`, alongside the other resets:

```tsx
    setImagePath(null)
```

- [ ] **Step 4: Upload on selection**

Add above `save()`:

```tsx
  /** Drops one key from the error map without disturbing the others. */
  function clearError(key: string) {
    setErrors((prev) => Object.fromEntries(
      Object.entries(prev).filter(([k]) => k !== key),
    ))
  }

  async function pickImage(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0]
    // Reset the input before anything else: without this, choosing the same
    // file again after a failure fires no change event and the picker looks
    // dead.
    e.target.value = ''
    if (!file) return

    setImageBusy(true)
    clearError('file')
    try {
      const { path } = await api.uploadImage(file)
      setImagePath(path)
    } catch (err) {
      // A rejected upload must leave typed content alone, exactly like a
      // rejected save — so this merges one field error in rather than
      // replacing the error map.
      if (err instanceof ApiError) {
        const byField = err.byField()
        if (Object.keys(byField).length === 0) toast.error(err.message)
        else setErrors((prev) => ({ ...prev, ...byField }))
      } else {
        toast.error('Could not upload the image')
      }
    } finally {
      setImageBusy(false)
    }
  }
```

- [ ] **Step 5: Toggle, keyboard, and the two views**

Add `'image_path'` and `'file'` to the claimed error keys so they are not also rendered by the
unclaimed-errors fallback:

```tsx
  const claimedErrorKeys = new Set(['kind', 'prompt_md', 'explanation_md', 'deck_id',
                                    'image_path', 'file'])
```

Add the shortcut to `handleContainerKeyDown`, before the `Escape` branch:

```tsx
    } else if (mod && e.key.toLowerCase() === 'p') {
      // preventDefault matters: this is the browser's print shortcut.
      e.preventDefault()
      setView((v) => (v === 'edit' ? 'preview' : 'edit'))
```

Return focus to the prompt whenever the form comes back:

```tsx
  // Coming back from Preview must land the cursor where typing continues,
  // not wherever the toggle button happened to leave it.
  useEffect(() => {
    if (view === 'edit' && loaded) promptRef.current?.focus()
  }, [view, loaded])
```

Replace the `<h1>` line with a header row carrying the toggle:

```tsx
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-display text-2xl font-bold">
          {mode === 'create' ? 'New card' : 'Edit card'}
        </h1>
        <div className="flex items-center gap-1 rounded-lg border p-1">
          <Button
            size="sm" variant={view === 'edit' ? 'secondary' : 'ghost'}
            aria-pressed={view === 'edit'} onClick={() => setView('edit')}
          >
            Edit
          </Button>
          <Button
            size="sm" variant={view === 'preview' ? 'secondary' : 'ghost'}
            aria-pressed={view === 'preview'} onClick={() => setView('preview')}
          >
            Preview
          </Button>
        </div>
      </div>
```

Wrap everything from the `errors.deck_id` line down to the explanation block in
`{view === 'edit' ? ( … ) : ( … )}`, leaving the action bar outside so it shows in both views:

```tsx
      {view === 'edit' ? (
        <>
          {/* … the existing deck_id error, Kind, Prompt, the image control from
              Step 5b, the per-kind blocks, unclaimedErrors and Explanation,
              all unchanged … */}
        </>
      ) : (
        <CardPreview
          kind={kind}
          promptMd={promptMd}
          imagePath={imagePath}
          choices={choices}
          accepted={accepted}
          answerMd={answerMd}
          explanationMd={explanationMd}
        />
      )}
```

- [ ] **Step 5b: The image control**

Insert this immediately after the Prompt block, inside the `view === 'edit'` branch:

```tsx
      <div className="space-y-2">
        <Label htmlFor="card-image">Image (optional)</Label>
        {imagePath === null ? (
          <Input
            id="card-image"
            type="file"
            accept="image/png,image/jpeg,image/webp"
            disabled={imageBusy || busy}
            onChange={(e) => void pickImage(e)}
            aria-invalid={!!errors.file}
          />
        ) : (
          <div className="flex items-center gap-3">
            <CardImage path={imagePath} alt="Card image" />
            <Button
              type="button" variant="secondary" size="sm" disabled={busy}
              onClick={() => { setImagePath(null); clearError('image_path') }}
            >
              Remove
            </Button>
          </div>
        )}
        {imageBusy && <p className="text-sm text-muted-foreground">Uploading…</p>}
        {(errors.file ?? errors.image_path) && (
          <p className="text-sm text-destructive">{errors.file ?? errors.image_path}</p>
        )}
      </div>
```

Finally, extend the shortcut hint at the bottom of the action bar so the new binding is
discoverable — append ` · ⌘/Ctrl+P preview` to both strings.

- [ ] **Step 6: Typecheck, build, and check by hand**

```bash
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

Then, with `cargo run` and `pnpm dev` up:

| Check | Result |
|---|---|
| Picking a PNG shows a thumbnail; saving stores it (reload the card to confirm) | |
| Picking a `.txt` renamed `.png` shows "not a PNG, JPEG or WebP" beside the picker | |
| …and the prompt/choices typed before that failure are still there | |
| …and picking a valid file straight afterwards works (the input reset) | |
| Remove, then save, clears the image on an existing card | |
| ⌘/Ctrl+P toggles the preview and does NOT open the print dialog | |
| Preview shows maths, the correct-choice mark, and the primary accepted answer | |
| Returning to Edit puts the cursor in the prompt | |
| Save-and-next clears the image with the rest of the form | |

If the Chrome extension is not connected, record that and move these to Task 9's outstanding
list rather than reporting them as passed.

- [ ] **Step 7: Commit**

```bash
cd .. && git add frontend/src/components/card-editor/CardPreview.tsx \
                 frontend/src/pages/CardEditorPage.tsx
git commit -m "feat(ui): editor image upload and an Edit/Preview toggle"
```

---

## Task 9: update the record

**Goal:** `HANDOVER.md` describes the app as it now is, and the Part 2b handoff document is
retired rather than left to contradict it.

**Files:**
- Modify: `docs/HANDOVER.md`
- Delete: `docs/PART-2B-HANDOFF.md`

**Acceptance Criteria:**
- [ ] "Where things stand" lists the upload endpoint, the `/images` route and the `<Markdown>` component
- [ ] "Next up" points at the spec's step 3, practice mode, not at Part 2b
- [ ] The three open questions in `PART-2B-HANDOFF.md` are recorded as answered, and the file is deleted
- [ ] Outstanding gains the Part 2b browser-only checks that were not verified
- [ ] The new conventions are written down: one rendering path, content-addressed filenames, the `image_path` shape guard
- [ ] Nothing in `HANDOVER.md` still describes card text as raw markdown

**Verify:** `grep -rn "PART-2B-HANDOFF" docs/ && grep -n "raw markdown" docs/HANDOVER.md` → no matches (grep exits 1)

**Steps:**

- [ ] **Step 1: Update the header and "Where things stand"**

Change the **Last updated** line to the current date and the branch this work is on. In the
bullet list, add:

```markdown
- `POST /api/images`: multipart upload, magic-byte type check (PNG/JPEG/WebP), 5 MiB cap,
  content-addressed filenames (`images/<16 hex>.<ext>`) written to `data/images/` and served
  read-only at `/images`. Standalone rather than card-scoped, deliberately — see the Part 2b
  spec §1. Orphan files from an abandoned upload are accepted and nothing sweeps them.
- `image_path` on card create and PATCH, validated against the shape the upload endpoint
  issues, under the existing cards full-replace rule
- One `<Markdown>` component (`react-markdown` + `remark-math` + `rehype-katex`, KaTeX fonts
  bundled locally) rendering the card list, the editor preview and, later, the session runner
- The deck's card list renders full multi-line markdown per row, with an image thumbnail that
  opens a lightbox
- The card editor uploads an image while you write, and toggles the whole form between Edit
  and Preview with `⌘/Ctrl+P`
```

Remove the sentence saying card text renders as raw markdown with no KaTeX, and update the
backend test count to whatever `cargo test` actually reports.

- [ ] **Step 2: Rewrite "Next up"**

Part 2b is done, so this becomes the spec's step 3:

```markdown
**Part 3: practice mode.** The session runner, grading against `accepted.normalised`, the
"I was right" override, and `reviews` rows. This is the first feature that reads cards rather
than writing them, and the first consumer of `POST /api/sessions`.

Two things already in place that Part 3 must use rather than reinvent: the `<Markdown>`
component (the session runner is its third consumer — do not add a fourth rendering path),
and `normalise()`, which computes the same key grading will look up.

After that: Bibble theme pass → mock test → stats → SM-2 → embed the bundle and LAN binding.
```

- [ ] **Step 3: Record the answered questions and delete the handoff**

`docs/PART-2B-HANDOFF.md` exists only to carry three open questions. All three are answered
and recorded in the Part 2b spec, so leaving the file in place gives a future reader a
document that presents settled decisions as open ones. Delete it — git history keeps it — and
add a line under "Where the record lives":

```markdown
- **Part 2b's three open design questions** were answered in the design session of
  2026-08-27 and are recorded in [`mitis/specs/2026-08-27-part2b-images-markdown-design.md`](mitis/specs/2026-08-27-part2b-images-markdown-design.md):
  standalone upload endpoint, whole-form Edit/Preview toggle, unclamped markdown rows with a
  thumbnail lightbox. `PART-2B-HANDOFF.md`, which posed them, has been deleted.
```

- [ ] **Step 4: Add the new conventions**

Under "Conventions and traps":

```markdown
**One rendering path.** `<Markdown>` in `frontend/src/components/Markdown.tsx` is the only
markdown renderer in the app, and it is why Part 2a shipped raw text everywhere. The card
list, the editor preview and Part 3's session runner all go through it. If you need different
behaviour, add a prop — a second renderer is the exact outcome the 2a/2b split existed to
prevent.

**KaTeX's fonts come from the npm package**, imported as `katex/dist/katex.min.css`. Do not
switch to a CDN. The Google Fonts `@import` in `globals.css` is already a known defect
deferred to build step 8; a second network dependency makes it worse.

**Tailwind's preflight strips list markers and heading sizes.** The `.markdown` block at the
end of `globals.css` restores them. Delete it and every bullet in every card silently becomes
an unindented line.

**Uploaded filenames are content-addressed** — the first 8 bytes of the SHA-256 as hex, plus
an extension from the *sniffed* type, never from the uploaded filename. Re-uploading the same
image therefore reuses one file. The extension list in `images::ImageType::extension` and the
one in `cards::is_uploaded_image_path` must stay in step; the second rejects any path the
first could not have produced.

**Upload failures use the same envelope as everything else**, with `fields[0].field == "file"`,
which is why the upload route raises axum's `DefaultBodyLimit` above the 5 MiB check it does
itself — axum's own 413 is raw `text/plain` and would be the one failure in the app the
frontend cannot parse.
```

- [ ] **Step 5: Move the unverified browser checks to Outstanding**

Add whatever Tasks 6 and 8 could not verify, and keep the Part 1/2a entries that are still
open:

```markdown
- Whether 100+ unclamped rows, each rendering KaTeX, stay responsive and scannable — this is
  what COS781 will actually be. If not, the kind filter and prompt search deferred out of
  Part 2a Task 4 are the fix.
- The Edit/Preview toggle inside the keyboard loop, and where focus lands coming back
- The image thumbnail and its lightbox at 375px
- KaTeX legibility against both Bibble palettes
```

- [ ] **Step 6: Verify and commit**

```bash
grep -rn "PART-2B-HANDOFF" docs/ ; grep -n "raw markdown" docs/HANDOVER.md
```

Expected: no output from either (both greps exit 1).

```bash
git rm docs/PART-2B-HANDOFF.md
git add docs/HANDOVER.md
git commit -m "docs: bring the handover up to date for Part 2b"
```

---

## Verification

Run the full gate from the repo root, with `export PATH="$HOME/.cargo/bin:$PATH"` already done:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
SQLX_OFFLINE=true cargo build
cd frontend && pnpm exec tsc --noEmit && pnpm build
```

Then the end-to-end path, which no single task covers on its own:

1. `cargo run` and `cd frontend && pnpm dev`
2. New card in a deck → attach a diagram → write a `short_answer` prompt with
   `$X, Y, Z$` and `$10\ 000$` in it → ⌘/Ctrl+P and confirm the maths renders
3. Save → the deck list shows the rendered prompt and the thumbnail
4. Click the thumbnail → the lightbox opens at full size
5. Edit the card → Remove → save → the image is gone and the card is otherwise unchanged

**Run one implementer at a time.** Part 2a dispatched two concurrently because their file
lists were disjoint; git's index is not per-file, and one agent's `git add`/`commit` swept the
other's staged work into a commit labelled as something else. Read-only reviewers can run
alongside an implementer. Two writers cannot.
