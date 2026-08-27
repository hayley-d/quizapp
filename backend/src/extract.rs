use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::header::CONTENT_TYPE;
use serde::de::DeserializeOwned;
use serde_json::error::Category;

use crate::error::AppError;

pub struct AppJson<T>(pub T);

impl<State, Body> FromRequest<State> for AppJson<Body>
where
    Body: DeserializeOwned,
    State: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(request: Request, state: &State) -> Result<Self, Self::Rejection> {
        let is_json = request
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|header_value| header_value.to_str().ok())
            .and_then(|header_value| header_value.split(';').next())
            .map(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
            .unwrap_or(false);
        if !is_json {
            return Err(AppError::UnsupportedMediaType(
                "Expected request with `Content-Type: application/json`".into(),
            ));
        }

        let bytes = Bytes::from_request(request, state).await.map_err(|error| {
            tracing::warn!(error = %error, "failed to read request body");
            AppError::BadRequest("Could not read the request body".into())
        })?;

        let deserializer = &mut serde_json::Deserializer::from_slice(&bytes);
        serde_path_to_error::deserialize(deserializer)
            .map(AppJson)
            .map_err(|path_error| {
                let path = path_error.path().to_string();
                let inner_error = path_error.into_inner();
                tracing::warn!(error = %inner_error, path = %path, "invalid request body");

                match inner_error.classify() {
                    Category::Syntax | Category::Eof | Category::Io => {
                        AppError::BadRequest("The request body is not valid JSON".into())
                    }
                    Category::Data => {
                        let description = inner_error.to_string();
                        let field = field_from_path(&path, &inner_error);
                        let message = if description.starts_with("missing field") {
                            "This field is required"
                        } else if description.starts_with("unknown field") {
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

fn field_from_path(path: &str, inner_error: &serde_json::Error) -> String {
    if !path.is_empty() && path != "." {
        return path.to_string();
    }
    let description = inner_error.to_string();
    description
        .split('`')
        .nth(1)
        .map(str::to_string)
        .unwrap_or_else(|| "body".to_string())
}
