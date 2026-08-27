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
    pub path: String,
}

async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<UploadedImage>)> {
    let mut uploaded_bytes: Option<axum::body::Bytes> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        tracing::warn!(error = %error, "malformed multipart upload");
        AppError::BadRequest("The upload could not be read".into())
    })? {
        if field.name() == Some("file") {
            uploaded_bytes = Some(field.bytes().await.map_err(|error| {
                tracing::warn!(error = %error, "could not read the upload field");
                AppError::BadRequest("The upload could not be read".into())
            })?);
            break;
        }
    }

    let bytes = uploaded_bytes
        .ok_or_else(|| AppError::validation([("file", "Choose an image to upload")]))?;

    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(AppError::validation([("file", "That image is larger than 5 MB")]));
    }

    let image_type = sniff(&bytes).ok_or_else(|| {
        AppError::validation([("file", "That file is not a PNG, JPEG or WebP image")])
    })?;

    let name = stored_name(&bytes, image_type);
    let destination = state.images_directory.join(&name);

    if !destination.exists() {
        tokio::fs::write(&destination, &bytes).await.map_err(|error| {
            tracing::error!(
                error = ?error,
                path = ?destination,
                "could not write the uploaded image",
            );
            AppError::Internal
        })?;
    }

    Ok((StatusCode::CREATED, Json(UploadedImage { path: format!("images/{name}") })))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/images",
        post(upload)
            .layer(DefaultBodyLimit::max(MAX_IMAGE_BYTES + 1024 * 1024)),
    )
}
