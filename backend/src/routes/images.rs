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
