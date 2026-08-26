use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: &'static str,
    pub message: String,
    pub fields: Vec<FieldError>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("validation failed")]
    Validation(Vec<FieldError>),
    #[error("{0}")]
    Conflict(String),
    /// Body present but not syntactically valid JSON (e.g. truncated/malformed input).
    #[error("{0}")]
    BadRequest(String),
    /// Missing or incorrect `Content-Type` on a request that requires a JSON body.
    #[error("{0}")]
    UnsupportedMediaType(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

impl AppError {
    pub fn validation<I, F, M>(fields: I) -> Self
    where
        I: IntoIterator<Item = (F, M)>,
        F: Into<String>,
        M: Into<String>,
    {
        AppError::Validation(
            fields
                .into_iter()
                .map(|(f, m)| FieldError { field: f.into(), message: m.into() })
                .collect(),
        )
    }

    pub fn parts(self) -> (StatusCode, ErrorBody) {
        match self {
            AppError::NotFound(what) => (
                StatusCode::NOT_FOUND,
                ErrorBody { error: "not_found", message: format!("{what} not found"),
                            fields: vec![] },
            ),
            AppError::Validation(fields) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorBody { error: "validation",
                            message: "The submitted values are invalid".into(), fields },
            ),
            AppError::Conflict(message) => (
                StatusCode::CONFLICT,
                ErrorBody { error: "conflict", message, fields: vec![] },
            ),
            AppError::BadRequest(message) => (
                StatusCode::BAD_REQUEST,
                ErrorBody { error: "bad_request", message, fields: vec![] },
            ),
            AppError::UnsupportedMediaType(message) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ErrorBody { error: "unsupported_media_type", message, fields: vec![] },
            ),
            AppError::Db(e) => {
                if let Some(dbe) = e.as_database_error() {
                    if dbe.is_unique_violation() {
                        return (
                            StatusCode::CONFLICT,
                            ErrorBody { error: "conflict",
                                        message: "That name is already in use".into(),
                                        fields: vec![] },
                        );
                    }
                    if dbe.is_foreign_key_violation() {
                        return (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            ErrorBody {
                                error: "validation",
                                message: "A referenced record does not exist".into(),
                                fields: vec![FieldError {
                                    field: "module_id".into(),
                                    message: "That module does not exist".into(),
                                }],
                            },
                        );
                    }
                }
                tracing::error!(error = ?e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorBody { error: "internal",
                                message: "Something went wrong".into(), fields: vec![] },
                )
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = self.parts();
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn validation_is_422_with_fields() {
        let err = AppError::validation([("name", "Name must not be empty")]);
        let (status, body) = err.parts();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body.error, "validation");
        assert_eq!(body.fields.len(), 1);
        assert_eq!(body.fields[0].field, "name");
    }

    #[test]
    fn not_found_is_404_with_empty_fields() {
        let (status, body) = AppError::NotFound("deck").parts();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error, "not_found");
        assert!(body.fields.is_empty());
    }
}
