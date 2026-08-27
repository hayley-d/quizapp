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
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    UnsupportedMediaType(String),
    #[error("internal error")]
    Internal,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl AppError {
    pub fn validation<Fields, Field, Message>(fields: Fields) -> Self
    where
        Fields: IntoIterator<Item = (Field, Message)>,
        Field: Into<String>,
        Message: Into<String>,
    {
        AppError::Validation(
            fields
                .into_iter()
                .map(|(field, message)| FieldError {
                    field: field.into(),
                    message: message.into(),
                })
                .collect(),
        )
    }

    pub fn tag_foreign_key_violation(self, field: &str, message: &str) -> Self {
        let is_foreign_key_violation = matches!(&self, AppError::Database(error)
            if error.as_database_error()
                .is_some_and(|database_error| database_error.is_foreign_key_violation()));

        if is_foreign_key_violation {
            AppError::validation([(field, message)])
        } else {
            self
        }
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
            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorBody { error: "internal", message: "Something went wrong".into(),
                            fields: vec![] },
            ),
            AppError::Database(error) => {
                if let Some(database_error) = error.as_database_error() {
                    if database_error.is_unique_violation() {
                        return (
                            StatusCode::CONFLICT,
                            ErrorBody { error: "conflict",
                                        message: "That name is already in use".into(),
                                        fields: vec![] },
                        );
                    }
                    if database_error.is_foreign_key_violation() {
                        return (
                            StatusCode::UNPROCESSABLE_ENTITY,
                            ErrorBody {
                                error: "validation",
                                message: "A referenced record does not exist".into(),
                                fields: vec![],
                            },
                        );
                    }
                }
                tracing::error!(error = ?error, "database error");
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
        let error = AppError::validation([("name", "Name must not be empty")]);
        let (status, body) = error.parts();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body.error, "validation");
        assert_eq!(body.fields.len(), 1);
        assert_eq!(body.fields[0].field, "name");
    }

    #[test]
    fn internal_is_500_with_the_envelope_and_no_detail() {
        let (status, body) = AppError::Internal.parts();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "internal");
        assert!(body.fields.is_empty());
        assert_eq!(body.message, "Something went wrong", "must not leak the cause");
    }

    #[test]
    fn not_found_is_404_with_empty_fields() {
        let (status, body) = AppError::NotFound("deck").parts();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error, "not_found");
        assert!(body.fields.is_empty());
    }

    async fn in_memory_database() -> sqlx::sqlite::SqliteConnection {
        use sqlx::sqlite::SqliteConnectOptions;
        use sqlx::ConnectOptions;

        SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true)
            .connect()
            .await
            .unwrap()
    }

    fn provoked_foreign_key_error() -> AppError {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut connection = in_memory_database().await;
                sqlx::query("CREATE TABLE parent (id INTEGER PRIMARY KEY)")
                    .execute(&mut connection).await.unwrap();
                sqlx::query(
                    "CREATE TABLE child (parent_id INTEGER REFERENCES parent(id))")
                    .execute(&mut connection).await.unwrap();
                let error = sqlx::query("INSERT INTO child (parent_id) VALUES (99)")
                    .execute(&mut connection).await.unwrap_err();
                assert!(error.as_database_error().unwrap().is_foreign_key_violation());
                AppError::Database(error)
            })
    }

    fn provoked_unique_violation() -> AppError {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut connection = in_memory_database().await;
                sqlx::query("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT UNIQUE)")
                    .execute(&mut connection).await.unwrap();
                sqlx::query("INSERT INTO items (name) VALUES ('test')")
                    .execute(&mut connection).await.unwrap();
                let error = sqlx::query("INSERT INTO items (name) VALUES ('test')")
                    .execute(&mut connection).await.unwrap_err();
                assert!(error.as_database_error().unwrap().is_unique_violation());
                AppError::Database(error)
            })
    }

    #[test]
    fn tagging_leaves_non_foreign_key_errors_alone() {
        let error = AppError::validation([("name", "Name must not be empty")])
            .tag_foreign_key_violation("deck_id", "That deck does not exist");
        let (status, body) = error.parts();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body.fields[0].field, "name",
            "tagging must not rewrite Validation errors",
        );

        let (status, body) = provoked_unique_violation()
            .tag_foreign_key_violation("deck_id", "That deck does not exist")
            .parts();
        assert_eq!(
            status, StatusCode::CONFLICT,
            "tagging must not rewrite UNIQUE violations",
        );
        assert_eq!(body.error, "conflict");
    }

    #[test]
    fn untagged_foreign_key_violation_names_no_field() {
        let (status, body) = provoked_foreign_key_error().parts();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body.fields.is_empty());
    }

    #[test]
    fn tagging_applies_the_caller_s_field() {
        let (status, body) = provoked_foreign_key_error()
            .tag_foreign_key_violation("deck_id", "That deck does not exist")
            .parts();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body.fields[0].field, "deck_id");
        assert_eq!(body.fields[0].message, "That deck does not exist");
    }
}
