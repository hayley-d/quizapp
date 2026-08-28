use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, FieldError};
use crate::extract::AppJson;
use crate::practice::{fold_candidate_rows, select_card, CandidateRow, NO_REPEAT_WINDOW,
    RECENT_REVIEW_LIMIT};
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


#[derive(Serialize)]
pub struct NextChoiceResponse {
    pub id: i64,
    pub text_md: String,
}

#[derive(Serialize)]
pub struct NextCardResponse {
    pub id: i64,
    pub kind: String,
    pub prompt_md: String,
    pub image_path: Option<String>,
    pub choices: Vec<NextChoiceResponse>,
}

#[derive(Serialize)]
pub struct NextResponse {
    pub card: NextCardResponse,
    pub pool_count: i64,
    pub answered_count: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevealRequest {
    pub card_id: i64,
}

#[derive(Serialize)]
pub struct RevealResponse {
    pub card_id: i64,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
}

pub struct ActiveSession {
    pub id: i64,
    pub deck_ids_json: String,
}

pub struct PoolCard {
    pub id: i64,
    pub kind: String,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
}

pub async fn load_active_session(
    pool: &sqlx::SqlitePool,
    session_id: i64,
) -> AppResult<ActiveSession> {
    let row = sqlx::query!(
        r#"SELECT id AS "id!: i64", deck_ids, ended_at FROM sessions WHERE id = ?"#,
        session_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("session"))?;

    if row.ended_at.is_some() {
        return Err(AppError::Conflict("This session has ended".to_string()));
    }
    Ok(ActiveSession { id: row.id, deck_ids_json: row.deck_ids })
}

pub async fn load_pool_card(
    pool: &sqlx::SqlitePool,
    deck_ids_json: &str,
    card_id: i64,
) -> AppResult<PoolCard> {
    sqlx::query_as!(
        PoolCard,
        r#"
        SELECT id AS "id!: i64", kind, answer_md, explanation_md
        FROM cards
        WHERE id = ?
          AND archived = 0
          AND deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
        "#,
        card_id,
        deck_ids_json,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::validation([("card_id", "That card is not in this session")]))
}

async fn load_candidates(
    pool: &sqlx::SqlitePool,
    deck_ids_json: &str,
) -> AppResult<Vec<CandidateRow>> {
    let rows = sqlx::query_as!(
        CandidateRow,
        r#"
        WITH pool AS (
            SELECT cards.id AS card_id
            FROM cards
            WHERE cards.archived = 0
              AND cards.deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
        ),
        recent AS (
            SELECT reviews.card_id AS card_id,
                   reviews.correct AS correct,
                   ROW_NUMBER() OVER (
                       PARTITION BY reviews.card_id
                       ORDER BY reviews.answered_at DESC, reviews.id DESC
                   ) AS recency_rank,
                   CAST(strftime('%s','now') AS INTEGER)
                     - CAST(strftime('%s', reviews.answered_at) AS INTEGER) AS age_seconds
            FROM reviews
            JOIN pool ON pool.card_id = reviews.card_id
        ),
        counts AS (
            SELECT reviews.card_id AS card_id, COUNT(*) AS review_count
            FROM reviews
            JOIN pool ON pool.card_id = reviews.card_id
            GROUP BY reviews.card_id
        )
        SELECT pool.card_id                     AS "card_id!: i64",
               COALESCE(counts.review_count, 0) AS "review_count!: i64",
               recent.correct                   AS "correct?: bool",
               recent.recency_rank              AS "recency_rank?: i64",
               recent.age_seconds               AS "age_seconds?: i64"
        FROM pool
        LEFT JOIN counts ON counts.card_id = pool.card_id
        LEFT JOIN recent ON recent.card_id = pool.card_id AND recent.recency_rank <= ?
        ORDER BY pool.card_id, recent.recency_rank
        "#,
        deck_ids_json,
        RECENT_REVIEW_LIMIT,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn load_recent_review_card_ids(
    pool: &sqlx::SqlitePool,
    session_id: i64,
) -> AppResult<Vec<i64>> {
    let window = NO_REPEAT_WINDOW as i64;
    let card_ids = sqlx::query_scalar!(
        r#"
        SELECT card_id AS "card_id!: i64"
        FROM reviews
        WHERE session_id = ?
        ORDER BY answered_at DESC, id DESC
        LIMIT ?
        "#,
        session_id,
        window,
    )
    .fetch_all(pool)
    .await?;
    Ok(card_ids)
}

async fn count_answered(pool: &sqlx::SqlitePool, session_id: i64) -> AppResult<i64> {
    let answered = sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "answered_count!: i64" FROM reviews WHERE session_id = ?"#,
        session_id,
    )
    .fetch_one(pool)
    .await?;
    Ok(answered)
}

async fn next(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
) -> AppResult<Json<NextResponse>> {
    let session = load_active_session(&state.pool, session_id).await?;

    let candidates = fold_candidate_rows(load_candidates(&state.pool, &session.deck_ids_json).await?);
    let recent_review_card_ids = load_recent_review_card_ids(&state.pool, session.id).await?;

    let card_id = select_card(&candidates, &recent_review_card_ids, rand::random::<f64>())
        .ok_or_else(|| {
            AppError::Conflict("This session has no cards left to practise".to_string())
        })?;

    let card = sqlx::query!(
        r#"SELECT id AS "id!: i64", kind, prompt_md, image_path FROM cards WHERE id = ?"#,
        card_id,
    )
    .fetch_one(&state.pool)
    .await?;

    let mut choices = sqlx::query_as!(
        NextChoiceResponse,
        r#"SELECT id AS "id!: i64", text_md FROM choices WHERE card_id = ? ORDER BY position"#,
        card_id,
    )
    .fetch_all(&state.pool)
    .await?;
    choices.shuffle(&mut rand::thread_rng());

    Ok(Json(NextResponse {
        card: NextCardResponse {
            id: card.id,
            kind: card.kind,
            prompt_md: card.prompt_md,
            image_path: card.image_path,
            choices,
        },
        pool_count: candidates.len() as i64,
        answered_count: count_answered(&state.pool, session.id).await?,
    }))
}

async fn reveal(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
    AppJson(body): AppJson<RevealRequest>,
) -> AppResult<Json<RevealResponse>> {
    let session = load_active_session(&state.pool, session_id).await?;
    let card = load_pool_card(&state.pool, &session.deck_ids_json, body.card_id).await?;

    if card.kind != "flashcard" {
        return Err(AppError::Conflict("Only a flashcard can be revealed".to_string()));
    }

    Ok(Json(RevealResponse {
        card_id: card.id,
        answer_md: card.answer_md,
        explanation_md: card.explanation_md,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(create))
        .route("/sessions/{id}/next", get(next))
        .route("/sessions/{id}/reveal", post(reveal))
}
