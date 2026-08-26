//! A JSON body extractor that always fails into the app's error envelope.
//!
//! `axum::Json`'s own rejections render as `text/plain` with serde's raw
//! message, which the frontend's `api.ts` cannot parse into `{error,
//! message, fields}`. `AppJson` re-implements body extraction on top of
//! `serde_path_to_error` so that:
//!   - a missing/incorrect `Content-Type` becomes a 415 envelope,
//!   - malformed JSON syntax (or a truncated body) becomes a 400 envelope,
//!   - a data error (missing field, wrong type) becomes a 422 validation
//!     envelope with `fields[0].field` set to the offending field, when the
//!     path-tracking deserializer can identify it.
//!
//! This lives apart from `error.rs` so the error *types* stay independent of
//! how a particular extractor produces them.

use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::header::CONTENT_TYPE;
use serde::de::DeserializeOwned;
use serde_json::error::Category;

use crate::error::AppError;

pub struct AppJson<T>(pub T);

impl<S, T> FromRequest<S> for AppJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let is_json = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
            .unwrap_or(false);
        if !is_json {
            return Err(AppError::UnsupportedMediaType(
                "Expected request with `Content-Type: application/json`".into(),
            ));
        }

        let bytes = Bytes::from_request(req, state).await.map_err(|e| {
            tracing::warn!(error = %e, "failed to read request body");
            AppError::BadRequest("Could not read the request body".into())
        })?;

        let de = &mut serde_json::Deserializer::from_slice(&bytes);
        serde_path_to_error::deserialize(de).map(AppJson).map_err(|err| {
            let path = err.path().to_string();
            let inner = err.into_inner();
            tracing::warn!(error = %inner, path = %path, "invalid request body");

            match inner.classify() {
                Category::Syntax | Category::Eof | Category::Io => {
                    AppError::BadRequest("The request body is not valid JSON".into())
                }
                Category::Data => {
                    let text = inner.to_string();
                    let field = field_from_path(&path, &inner);
                    let message = if text.starts_with("missing field") {
                        "This field is required"
                    } else if text.starts_with("unknown field") {
                        "This field is not recognized"
                    } else {
                        "This field has an unexpected type"
                    };
                    AppError::validation([(field, message)])
                }
            }
        })
    }
}

/// `serde_path_to_error` tracks the path *while descending into* a field, so
/// a wrong-typed value (e.g. a number where a string was expected) resolves
/// to a real path segment. A `missing field` error, though, is raised by the
/// struct visitor only after all fields have been read, off the tracked
/// path, so the path there is empty; recover the name from serde's message,
/// which is always `` missing field `<name>` ``.
fn field_from_path(path: &str, inner: &serde_json::Error) -> String {
    if !path.is_empty() && path != "." {
        return path.to_string();
    }
    let msg = inner.to_string();
    msg.split('`')
        .nth(1)
        .map(str::to_string)
        .unwrap_or_else(|| "body".to_string())
}
