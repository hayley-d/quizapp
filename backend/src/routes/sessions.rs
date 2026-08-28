use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, FieldError};
use crate::extract::AppJson;
use crate::state::AppState;

const MODES: [&str; 3] = ["practice", "mock", "sm2"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSession {
    pub mode: String,
    #[serde(default)]
    pub deck_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub module_id: Option<i64>,
    #[serde(default)]
    pub target_count: Option<i64>,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub id: i64,
    pub mode: String,
    pub deck_ids: Vec<i64>,
    pub target_count: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub pool_count: i64,
    pub answered_count: i64,
}

fn validate_submission(body: &CreateSession) -> AppResult<()> {
    let mut errors: Vec<FieldError> = Vec::new();
    let mut push_error = |field: &str, message: &str| {
        errors.push(FieldError { field: field.to_string(), message: message.to_string() });
    };

    if !MODES.contains(&body.mode.as_str()) {
        push_error("mode", "mode must be practice, mock or sm2");
    } else if body.mode != "practice" {
        push_error("mode", "Only practice mode is available yet");
    }

    match (&body.deck_ids, body.module_id) {
        (Some(_), Some(_)) => push_error("deck_ids", "Choose either decks or a module, not both"),
        (None, None) => push_error("deck_ids", "Choose at least one deck or a module"),
        (Some(deck_ids), None) if deck_ids.is_empty() => {
            push_error("deck_ids", "Choose at least one deck")
        }
        _ => {}
    }

    if body.target_count.is_some() {
        push_error("target_count", "Practice sessions have no target count");
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::Validation(errors))
    }
}

fn canonical_deck_ids(deck_ids: &[i64]) -> Vec<i64> {
    let mut canonical = deck_ids.to_vec();
    canonical.sort_unstable();
    canonical.dedup();
    canonical
}

fn encode_deck_ids(deck_ids: &[i64]) -> AppResult<String> {
    serde_json::to_string(deck_ids).map_err(|_| AppError::Internal)
}

async fn resolve_deck_ids(
    pool: &sqlx::SqlitePool,
    body: &CreateSession,
) -> AppResult<Vec<i64>> {
    if let Some(module_id) = body.module_id {
        let module_count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "module_count!: i64" FROM modules WHERE id = ?"#,
            module_id,
        )
        .fetch_one(pool)
        .await?;
        if module_count == 0 {
            return Err(AppError::validation([("module_id", "That module does not exist")]));
        }

        let deck_ids = sqlx::query_scalar!(
            r#"SELECT id AS "id!: i64" FROM decks WHERE module_id = ? ORDER BY id"#,
            module_id,
        )
        .fetch_all(pool)
        .await?;
        if deck_ids.is_empty() {
            return Err(AppError::validation([("module_id", "That module has no decks")]));
        }
        return Ok(deck_ids);
    }

    let requested = canonical_deck_ids(body.deck_ids.as_deref().unwrap_or_default());
    let encoded = encode_deck_ids(&requested)?;
    let found = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "found!: i64"
        FROM decks
        WHERE id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
        "#,
        encoded,
    )
    .fetch_one(pool)
    .await?;
    if found != requested.len() as i64 {
        return Err(AppError::validation([("deck_ids", "That deck does not exist")]));
    }
    Ok(requested)
}

pub async fn fetch_session(
    pool: &sqlx::SqlitePool,
    session_id: i64,
) -> AppResult<SessionResponse> {
    let row = sqlx::query!(
        r#"
        SELECT id AS "id!: i64", mode, deck_ids, target_count, started_at, ended_at
        FROM sessions WHERE id = ?
        "#,
        session_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("session"))?;

    let deck_ids: Vec<i64> = serde_json::from_str(&row.deck_ids).map_err(|_| AppError::Internal)?;
    let encoded = encode_deck_ids(&deck_ids)?;

    let pool_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "pool_count!: i64"
        FROM cards
        WHERE archived = 0 AND deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
        "#,
        encoded,
    )
    .fetch_one(pool)
    .await?;

    let answered_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "answered_count!: i64" FROM reviews WHERE session_id = ?"#,
        session_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(SessionResponse {
        id: row.id,
        mode: row.mode,
        deck_ids,
        target_count: row.target_count,
        started_at: row.started_at,
        ended_at: row.ended_at,
        pool_count,
        answered_count,
    })
}

async fn create(
    State(state): State<AppState>,
    AppJson(body): AppJson<CreateSession>,
) -> AppResult<(StatusCode, Json<SessionResponse>)> {
    validate_submission(&body)?;

    let deck_ids = resolve_deck_ids(&state.pool, &body).await?;
    let encoded = encode_deck_ids(&deck_ids)?;

    let pool_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "pool_count!: i64"
        FROM cards
        WHERE archived = 0 AND deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
        "#,
        encoded,
    )
    .fetch_one(&state.pool)
    .await?;

    if pool_count == 0 {
        let field = if body.module_id.is_some() { "module_id" } else { "deck_ids" };
        return Err(AppError::validation([(field, "Those decks have no cards to practise")]));
    }

    let session_id = sqlx::query_scalar!(
        r#"
        INSERT INTO sessions (mode, deck_ids, target_count)
        VALUES (?, ?, NULL)
        RETURNING id AS "id!: i64"
        "#,
        body.mode,
        encoded,
    )
    .fetch_one(&state.pool)
    .await?;

    let session = fetch_session(&state.pool, session_id).await?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/sessions", post(create))
}
