use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, FieldError};
use crate::extract::AppJson;
use crate::normalise::normalise;
use crate::grading::{
    correctness_of_self_grade, grade_flashcard_typed, grade_multiple_choice, grade_text_answer,
    parse_self_grade, self_grade_as_text, GradableChoice,
};
use crate::mastery::{self, MasteryLevel, MovementDirection, SessionMasteryMovement};
use crate::mock::{first_unanswered, mock_order};
use crate::practice::{fold_candidate_rows, select_card, CandidateRow, NO_REPEAT_WINDOW,
    RECENT_REVIEW_LIMIT};
use crate::scheduler::{apply, initial_state, quality_for, replay, ScheduleState};
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
    #[serde(default)]
    pub mastery_goal: Option<i64>,
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub id: i64,
    pub mode: String,
    pub deck_ids: Vec<i64>,
    pub target_count: Option<i64>,
    pub mastery_goal: Option<i64>,
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
        let message = match body.mode.as_str() {
            "mock" => "A mock test is the whole deck, so its length is not yours to set",
            "sm2" => "A spaced repetition session is whatever is due, so its length is not yours to set",
            _ => "Practice sessions have no target count",
        };
        push_error("target_count", message);
    }

    match body.mastery_goal {
        Some(_) if body.mode == "mock" => push_error(
            "mastery_goal",
            "A mock test is a graded run, not a target to move cards up",
        ),
        Some(goal) if goal < 1 => {
            push_error("mastery_goal", "A goal must be at least one card")
        }
        _ => {}
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
        SELECT id AS "id!: i64", mode, deck_ids, target_count, mastery_goal, started_at, ended_at
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
        mastery_goal: row.mastery_goal,
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

    let target_count = match body.mode.as_str() {
        "mock" => Some(pool_count),
        "sm2" => {
            let due_count = count_due(&state.pool, &encoded).await?;
            if due_count == 0 {
                let field = if body.module_id.is_some() { "module_id" } else { "deck_ids" };
                let message = match next_due_at(&state.pool, &encoded).await? {
                    Some(next_due) => format!(
                        "Nothing is due yet — the next card is due {}",
                        next_due.get(..10).unwrap_or(&next_due),
                    ),
                    None => "Nothing is due yet".to_string(),
                };
                return Err(AppError::validation([(field, message)]));
            }
            Some(due_count)
        }
        _ => None,
    };

    if body.mastery_goal.is_some_and(|goal| goal > pool_count) {
        return Err(AppError::validation([(
            "mastery_goal",
            "That is more cards than this deck has",
        )]));
    }

    let session_id = sqlx::query_scalar!(
        r#"
        INSERT INTO sessions (mode, deck_ids, target_count, mastery_goal)
        VALUES (?, ?, ?, ?)
        RETURNING id AS "id!: i64"
        "#,
        body.mode,
        encoded,
        target_count,
        body.mastery_goal,
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
pub struct PracticeNextResponse {
    pub mode: &'static str,
    pub card: NextCardResponse,
    pub pool_count: i64,
    pub answered_count: i64,
    pub correct_count: i64,
    pub mastery_goal: Option<i64>,
    pub mastery_moved_up_count: i64,
}

#[derive(Serialize)]
pub struct MockNextResponse {
    pub mode: &'static str,
    pub card: NextCardResponse,
    pub target_count: Option<i64>,
    pub started_at: String,
    pub pool_count: i64,
    pub answered_count: i64,
}

#[derive(Serialize)]
pub struct Sm2NextResponse {
    pub mode: &'static str,
    pub card: NextCardResponse,
    pub target_count: Option<i64>,
    pub pool_count: i64,
    pub answered_count: i64,
    pub correct_count: i64,
    pub mastery_goal: Option<i64>,
    pub mastery_moved_up_count: i64,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum NextResponse {
    Practice(PracticeNextResponse),
    Mock(MockNextResponse),
    Sm2(Sm2NextResponse),
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

#[derive(Serialize)]
pub struct ResultQuestion {
    pub review_id: i64,
    pub card_id: i64,
    pub kind: String,
    pub prompt_md: String,
    pub image_path: Option<String>,
    pub given: Option<String>,
    pub self_grade: Option<String>,
    pub expected: Vec<String>,
    pub explanation_md: Option<String>,
    pub correct: bool,
    pub overridden: bool,
    pub can_override: bool,
    pub ms: Option<i64>,
    pub answered_at: String,
}

#[derive(Serialize)]
pub struct ResultsResponse {
    pub summary: SummaryResponse,
    pub questions: Vec<ResultQuestion>,
}

pub struct ResultReviewRow {
    pub review_id: i64,
    pub card_id: i64,
    pub kind: String,
    pub prompt_md: String,
    pub image_path: Option<String>,
    pub given: Option<String>,
    pub self_grade: Option<String>,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub correct: bool,
    pub overridden: bool,
    pub ms: Option<i64>,
    pub answered_at: String,
}

pub fn expected_for_kind(
    kind: &str,
    answer_md: &Option<String>,
    correct_choices: &[String],
    accepted: &[String],
) -> Vec<String> {
    match kind {
        "mc_single" => correct_choices.to_vec(),
        "text_answer" => accepted.to_vec(),
        _ => answer_md.clone().into_iter().collect(),
    }
}

pub fn can_override_result(kind: &str, correct: bool) -> bool {
    !correct && matches!(kind, "text_answer" | "flashcard")
}

pub fn assemble_results(
    rows: Vec<ResultReviewRow>,
    correct_choices_by_card: &std::collections::HashMap<i64, Vec<String>>,
    accepted_by_card: &std::collections::HashMap<i64, Vec<String>>,
) -> Vec<ResultQuestion> {
    let empty: Vec<String> = Vec::new();
    rows.into_iter()
        .map(|row| {
            let correct_choices =
                correct_choices_by_card.get(&row.card_id).unwrap_or(&empty).as_slice();
            let accepted = accepted_by_card.get(&row.card_id).unwrap_or(&empty).as_slice();
            let expected =
                expected_for_kind(&row.kind, &row.answer_md, correct_choices, accepted);
            ResultQuestion {
                review_id: row.review_id,
                card_id: row.card_id,
                can_override: can_override_result(&row.kind, row.correct),
                kind: row.kind,
                prompt_md: row.prompt_md,
                image_path: row.image_path,
                given: row.given,
                self_grade: row.self_grade,
                expected,
                explanation_md: row.explanation_md,
                correct: row.correct,
                overridden: row.overridden,
                ms: row.ms,
                answered_at: row.answered_at,
            }
        })
        .collect()
}

pub struct ActiveSession {
    pub id: i64,
    pub mode: String,
    pub target_count: Option<i64>,
    pub mastery_goal: Option<i64>,
    pub started_at: String,
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
        r#"
        SELECT id AS "id!: i64", mode, target_count, mastery_goal, started_at, deck_ids, ended_at
        FROM sessions WHERE id = ?
        "#,
        session_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("session"))?;

    if row.ended_at.is_some() {
        return Err(AppError::Conflict("This session has ended".to_string()));
    }
    Ok(ActiveSession {
        id: row.id,
        mode: row.mode,
        target_count: row.target_count,
        mastery_goal: row.mastery_goal,
        started_at: row.started_at,
        deck_ids_json: row.deck_ids,
    })
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

async fn load_unresolved_miss_card_ids(
    pool: &sqlx::SqlitePool,
    session_id: i64,
) -> AppResult<Vec<i64>> {
    let card_ids = sqlx::query_scalar!(
        r#"
        WITH latest AS (
            SELECT card_id,
                   correct,
                   ROW_NUMBER() OVER (
                       PARTITION BY card_id
                       ORDER BY answered_at DESC, id DESC
                   ) AS recency_rank
            FROM reviews
            WHERE session_id = ?
        )
        SELECT card_id AS "card_id!: i64"
        FROM latest
        WHERE recency_rank = 1 AND correct = 0
        "#,
        session_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(card_ids)
}

async fn count_progress(pool: &sqlx::SqlitePool, session_id: i64) -> AppResult<(i64, i64)> {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(*)                  AS "answered_count!: i64",
               COALESCE(SUM(correct), 0) AS "correct_count!: i64"
        FROM reviews WHERE session_id = ?
        "#,
        session_id,
    )
    .fetch_one(pool)
    .await?;
    Ok((row.answered_count, row.correct_count))
}

async fn load_unanswered_mock_card_ids(
    pool: &sqlx::SqlitePool,
    deck_ids_json: &str,
    session_id: i64,
) -> AppResult<Vec<i64>> {
    let rows = sqlx::query_scalar!(
        r#"
        SELECT cards.id AS "card_id!: i64"
        FROM cards
        WHERE cards.archived = 0
          AND cards.deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
          AND NOT EXISTS (
                SELECT 1 FROM reviews
                WHERE reviews.card_id = cards.id AND reviews.session_id = ?
              )
        ORDER BY cards.id
        "#,
        deck_ids_json,
        session_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn load_next_due_card_id(
    pool: &sqlx::SqlitePool,
    deck_ids_json: &str,
    session_id: i64,
) -> AppResult<Option<i64>> {
    let card_id = sqlx::query_scalar!(
        r#"
        SELECT cards.id AS "card_id!: i64"
        FROM cards
        LEFT JOIN schedule ON schedule.card_id = cards.id
        WHERE cards.archived = 0
          AND cards.deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
          AND (schedule.due_at IS NULL
               OR schedule.due_at <= strftime('%Y-%m-%dT%H:%M:%SZ','now'))
          AND cards.id NOT IN (SELECT card_id FROM reviews WHERE session_id = ?)
        ORDER BY COALESCE(schedule.due_at, '') ASC, cards.id ASC
        LIMIT 1
        "#,
        deck_ids_json,
        session_id,
    )
    .fetch_optional(pool)
    .await?;
    Ok(card_id)
}

async fn count_pool(pool: &sqlx::SqlitePool, deck_ids_json: &str) -> AppResult<i64> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "pool_count!: i64"
        FROM cards
        WHERE archived = 0 AND deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
        "#,
        deck_ids_json,
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

async fn count_due(pool: &sqlx::SqlitePool, deck_ids_json: &str) -> AppResult<i64> {
    let due_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "due_count!: i64"
        FROM cards
        LEFT JOIN schedule ON schedule.card_id = cards.id
        WHERE cards.archived = 0
          AND cards.deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
          AND (schedule.due_at IS NULL
               OR schedule.due_at <= strftime('%Y-%m-%dT%H:%M:%SZ','now'))
        "#,
        deck_ids_json,
    )
    .fetch_one(pool)
    .await?;
    Ok(due_count)
}

async fn next_due_at(
    pool: &sqlx::SqlitePool,
    deck_ids_json: &str,
) -> AppResult<Option<String>> {
    let next_due = sqlx::query_scalar!(
        r#"
        SELECT MIN(schedule.due_at) AS "next_due_at?: String"
        FROM cards
        JOIN schedule ON schedule.card_id = cards.id
        WHERE cards.archived = 0
          AND cards.deck_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?))
        "#,
        deck_ids_json,
    )
    .fetch_one(pool)
    .await?;
    Ok(next_due)
}

async fn load_served_card(
    pool: &sqlx::SqlitePool,
    card_id: i64,
    choice_order_seed: Option<u64>,
) -> AppResult<NextCardResponse> {
    let card = sqlx::query!(
        r#"SELECT id AS "id!: i64", kind, prompt_md, image_path FROM cards WHERE id = ?"#,
        card_id,
    )
    .fetch_one(pool)
    .await?;

    let mut choices = sqlx::query_as!(
        NextChoiceResponse,
        r#"SELECT id AS "id!: i64", text_md FROM choices WHERE card_id = ? ORDER BY position"#,
        card_id,
    )
    .fetch_all(pool)
    .await?;

    match choice_order_seed {
        Some(seed) => {
            let choice_ids: Vec<i64> = choices.iter().map(|choice| choice.id).collect();
            let ordered = mock_order(&choice_ids, seed);
            choices.sort_by_key(|choice| {
                ordered.iter().position(|id| *id == choice.id).unwrap_or(usize::MAX)
            });
        }
        None => choices.shuffle(&mut rand::thread_rng()),
    }

    Ok(NextCardResponse {
        id: card.id,
        kind: card.kind,
        prompt_md: card.prompt_md,
        image_path: card.image_path,
        choices,
    })
}

async fn next(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
) -> AppResult<Json<NextResponse>> {
    let session = load_active_session(&state.pool, session_id).await?;

    if session.mode == "mock" {
        let unanswered =
            load_unanswered_mock_card_ids(&state.pool, &session.deck_ids_json, session.id).await?;
        let seed = session.id as u64;
        let card_id = first_unanswered(&unanswered, seed).ok_or_else(|| {
            AppError::Conflict("Every card in this mock test has been answered".to_string())
        })?;

        let card =
            load_served_card(&state.pool, card_id, Some(seed ^ (card_id as u64))).await?;
        let pool_count = count_pool(&state.pool, &session.deck_ids_json).await?;
        let (answered_count, _) = count_progress(&state.pool, session.id).await?;

        return Ok(Json(NextResponse::Mock(MockNextResponse {
            mode: "mock",
            card,
            target_count: session.target_count,
            started_at: session.started_at,
            pool_count,
            answered_count,
        })));
    }

    if session.mode == "sm2" {
        let card_id = load_next_due_card_id(&state.pool, &session.deck_ids_json, session.id)
            .await?
            .ok_or_else(|| {
                AppError::Conflict("Everything due in this session has been answered".to_string())
            })?;

        let card = load_served_card(&state.pool, card_id, None).await?;
        let pool_count = count_due(&state.pool, &session.deck_ids_json).await?;
        let (answered_count, correct_count) = count_progress(&state.pool, session.id).await?;

        let movements =
            mastery::session_movements(&state.pool, session.id, &session.started_at).await?;

        return Ok(Json(NextResponse::Sm2(Sm2NextResponse {
            mode: "sm2",
            card,
            target_count: session.target_count,
            pool_count,
            answered_count,
            correct_count,
            mastery_goal: session.mastery_goal,
            mastery_moved_up_count: mastery::count_moved_up(&movements),
        })));
    }

    let candidates = fold_candidate_rows(load_candidates(&state.pool, &session.deck_ids_json).await?);
    let recent_review_card_ids = load_recent_review_card_ids(&state.pool, session.id).await?;
    let unresolved_miss_card_ids =
        load_unresolved_miss_card_ids(&state.pool, session.id).await?;

    let card_id = select_card(
        &candidates,
        &recent_review_card_ids,
        &unresolved_miss_card_ids,
        rand::random::<f64>(),
    )
        .ok_or_else(|| {
            AppError::Conflict("This session has no cards left to practise".to_string())
        })?;

    let card = load_served_card(&state.pool, card_id, None).await?;
    let (answered_count, correct_count) = count_progress(&state.pool, session.id).await?;

    let movements = mastery::session_movements(&state.pool, session.id, &session.started_at).await?;

    Ok(Json(NextResponse::Practice(PracticeNextResponse {
        mode: "practice",
        card,
        pool_count: candidates.len() as i64,
        answered_count,
        correct_count,
        mastery_goal: session.mastery_goal,
        mastery_moved_up_count: mastery::count_moved_up(&movements),
    })))
}

async fn reveal(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
    AppJson(body): AppJson<RevealRequest>,
) -> AppResult<Json<RevealResponse>> {
    let session = load_active_session(&state.pool, session_id).await?;

    if session.mode == "mock" {
        return Err(AppError::Conflict("A mock test does not reveal answers".to_string()));
    }

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitAnswer {
    pub card_id: i64,
    #[serde(default)]
    pub given: Option<String>,
    #[serde(default)]
    pub choice_id: Option<i64>,
    #[serde(default)]
    pub self_grade: Option<String>,
    #[serde(default)]
    pub ms: Option<i64>,
}

#[derive(Serialize)]
pub struct PracticeAnswerResponse {
    pub mode: &'static str,
    pub review_id: i64,
    pub correct: bool,
    pub expected: Vec<String>,
    pub explanation_md: Option<String>,
    pub can_override: bool,
    pub level_before: MasteryLevel,
    pub level_after: MasteryLevel,
    pub mastery_direction: MovementDirection,
    pub mastery_moved_up_count: i64,
}

#[derive(Serialize)]
pub struct MockAnswerResponse {
    pub mode: &'static str,
    pub answered_count: i64,
    pub pool_count: i64,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum AnswerResponse {
    Practice(PracticeAnswerResponse),
    Mock(MockAnswerResponse),
}

struct GradedAnswer {
    correct: bool,
    expected: Vec<String>,
    stored_given: Option<String>,
    stored_self_grade: Option<&'static str>,
}

fn reject_fields_for_other_kinds(
    errors: &mut Vec<FieldError>,
    body: &SubmitAnswer,
    allowed: &str,
    mode: &str,
) {
    if allowed != "given" && body.given.is_some() {
        let message = if mode == "mock" {
            "Only a text-answer or flashcard takes typed text"
        } else {
            "Only a text-answer card takes typed text"
        };
        errors.push(FieldError { field: "given".to_string(), message: message.to_string() });
    }
    if allowed != "choice_id" && body.choice_id.is_some() {
        errors.push(FieldError {
            field: "choice_id".to_string(),
            message: "Only a multiple-choice card has options".to_string(),
        });
    }
    if allowed != "self_grade" && body.self_grade.is_some() {
        let message = if mode == "mock" {
            "A mock test grades flashcards automatically"
        } else {
            "Only a flashcard is self-graded"
        };
        errors.push(FieldError { field: "self_grade".to_string(), message: message.to_string() });
    }
}

async fn grade_answer(
    pool: &sqlx::SqlitePool,
    card: &PoolCard,
    body: &SubmitAnswer,
    mode: &str,
) -> AppResult<GradedAnswer> {
    let mut errors: Vec<FieldError> = Vec::new();

    if body.ms.is_some_and(|milliseconds| milliseconds < 0) {
        errors.push(FieldError {
            field: "ms".to_string(),
            message: "ms must not be negative".to_string(),
        });
    }

    match card.kind.as_str() {
        "mc_single" => {
            reject_fields_for_other_kinds(&mut errors, body, "choice_id", mode);
            let Some(choice_id) = body.choice_id else {
                errors.push(FieldError {
                    field: "choice_id".to_string(),
                    message: "This field is required".to_string(),
                });
                return Err(AppError::Validation(errors));
            };
            if !errors.is_empty() {
                return Err(AppError::Validation(errors));
            }

            let rows = sqlx::query!(
                r#"
                SELECT id AS "choice_id!: i64", text_md, is_correct AS "is_correct!: bool"
                FROM choices WHERE card_id = ? ORDER BY position
                "#,
                card.id,
            )
            .fetch_all(pool)
            .await?;

            let gradable: Vec<GradableChoice> = rows
                .iter()
                .map(|row| GradableChoice {
                    choice_id: row.choice_id,
                    is_correct: row.is_correct,
                })
                .collect();

            let correct = grade_multiple_choice(&gradable, choice_id).ok_or_else(|| {
                AppError::validation([("choice_id", "That option is not on this card")])
            })?;

            let chosen_text = rows
                .iter()
                .find(|row| row.choice_id == choice_id)
                .map(|row| row.text_md.clone());
            let expected = rows
                .iter()
                .filter(|row| row.is_correct)
                .map(|row| row.text_md.clone())
                .collect();

            Ok(GradedAnswer {
                correct,
                expected,
                stored_given: chosen_text,
                stored_self_grade: None,
            })
        }
        "text_answer" => {
            reject_fields_for_other_kinds(&mut errors, body, "given", mode);
            let trimmed = body.given.as_deref().map(str::trim).unwrap_or_default();
            if body.given.is_none() {
                errors.push(FieldError {
                    field: "given".to_string(),
                    message: "This field is required".to_string(),
                });
            } else if trimmed.is_empty() {
                errors.push(FieldError {
                    field: "given".to_string(),
                    message: "Type an answer".to_string(),
                });
            }
            if !errors.is_empty() {
                return Err(AppError::Validation(errors));
            }

            let rows = sqlx::query!(
                r#"
                SELECT text, normalised, is_primary AS "is_primary!: bool"
                FROM accepted WHERE card_id = ? ORDER BY is_primary DESC, id
                "#,
                card.id,
            )
            .fetch_all(pool)
            .await?;

            let keys: Vec<String> = rows.iter().map(|row| row.normalised.clone()).collect();
            let correct = grade_text_answer(trimmed, &keys);
            let expected = rows.iter().map(|row| row.text.clone()).collect();

            Ok(GradedAnswer {
                correct,
                expected,
                stored_given: Some(trimmed.to_string()),
                stored_self_grade: None,
            })
        }
        "flashcard" if mode == "mock" => {
            reject_fields_for_other_kinds(&mut errors, body, "given", mode);

            let trimmed = match body.given.as_deref().map(str::trim) {
                None => {
                    errors.push(FieldError {
                        field: "given".to_string(),
                        message: "This field is required".to_string(),
                    });
                    None
                }
                Some("") => {
                    errors.push(FieldError {
                        field: "given".to_string(),
                        message: "Type an answer".to_string(),
                    });
                    None
                }
                Some(text) => Some(text),
            };
            if !errors.is_empty() {
                return Err(AppError::Validation(errors));
            }
            let trimmed = trimmed.ok_or(AppError::Internal)?;
            let answer_md = card.answer_md.as_deref().unwrap_or_default();

            Ok(GradedAnswer {
                correct: grade_flashcard_typed(trimmed, answer_md),
                expected: card.answer_md.clone().into_iter().collect(),
                stored_given: Some(trimmed.to_string()),
                stored_self_grade: None,
            })
        }
        "flashcard" => {
            reject_fields_for_other_kinds(&mut errors, body, "self_grade", mode);
            let parsed = match body.self_grade.as_deref() {
                None => {
                    errors.push(FieldError {
                        field: "self_grade".to_string(),
                        message: "This field is required".to_string(),
                    });
                    None
                }
                Some(raw) => {
                    let parsed = parse_self_grade(raw);
                    if parsed.is_none() {
                        errors.push(FieldError {
                            field: "self_grade".to_string(),
                            message: "self_grade must be again, hard, good or easy".to_string(),
                        });
                    }
                    parsed
                }
            };
            if !errors.is_empty() {
                return Err(AppError::Validation(errors));
            }
            let self_grade = parsed.ok_or(AppError::Internal)?;

            Ok(GradedAnswer {
                correct: correctness_of_self_grade(self_grade),
                expected: card.answer_md.clone().into_iter().collect(),
                stored_given: None,
                stored_self_grade: Some(self_grade_as_text(self_grade)),
            })
        }
        _ => Err(AppError::Internal),
    }
}

async fn answer(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
    AppJson(body): AppJson<SubmitAnswer>,
) -> AppResult<Json<AnswerResponse>> {
    let session = load_active_session(&state.pool, session_id).await?;
    let card = load_pool_card(&state.pool, &session.deck_ids_json, body.card_id).await?;

    if session.mode == "mock" {
        let already_answered = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "already_answered!: i64"
            FROM reviews WHERE session_id = ? AND card_id = ?
            "#,
            session.id,
            card.id,
        )
        .fetch_one(&state.pool)
        .await?;

        if already_answered > 0 {
            return Err(AppError::Conflict(
                "That card has already been answered in this mock test".to_string(),
            ));
        }
    }

    let graded = grade_answer(&state.pool, &card, &body, &session.mode).await?;

    let mut transaction = state.pool.begin().await?;

    let review = sqlx::query!(
        r#"
        INSERT INTO reviews (card_id, session_id, given, correct, self_grade, ms)
        VALUES (?, ?, ?, ?, ?, ?)
        RETURNING id AS "id!: i64", answered_at AS "answered_at!: String"
        "#,
        card.id,
        session.id,
        graded.stored_given,
        graded.correct,
        graded.stored_self_grade,
        body.ms,
    )
    .fetch_one(&mut *transaction)
    .await?;
    let review_id = review.id;

    if session.mode == "sm2" {
        let self_grade = graded.stored_self_grade.and_then(parse_self_grade);
        let quality = quality_for(graded.correct, self_grade);
        let current = load_schedule_state(&mut transaction, card.id).await?;
        let updated = apply(&current, quality);
        write_schedule(&mut transaction, card.id, &updated, &review.answered_at).await?;
    }

    transaction.commit().await?;

    if session.mode == "mock" {
        let (answered_count, _) = count_progress(&state.pool, session.id).await?;
        let pool_count = count_pool(&state.pool, &session.deck_ids_json).await?;
        return Ok(Json(AnswerResponse::Mock(MockAnswerResponse {
            mode: "mock",
            answered_count,
            pool_count,
        })));
    }

    let progress = card_mastery_progress(&state.pool, session.id, &session.started_at, card.id).await?;

    Ok(Json(AnswerResponse::Practice(PracticeAnswerResponse {
        mode: if session.mode == "sm2" { "sm2" } else { "practice" },
        review_id,
        correct: graded.correct,
        expected: graded.expected,
        explanation_md: card.explanation_md,
        can_override: card.kind == "text_answer" && !graded.correct,
        level_before: progress.level_before,
        level_after: progress.level_after,
        mastery_direction: progress.direction,
        mastery_moved_up_count: progress.moved_up_count,
    })))
}

struct CardMasteryProgress {
    level_before: MasteryLevel,
    level_after: MasteryLevel,
    direction: MovementDirection,
    moved_up_count: i64,
}

async fn card_mastery_progress(
    pool: &sqlx::SqlitePool,
    session_id: i64,
    started_at: &str,
    card_id: i64,
) -> AppResult<CardMasteryProgress> {
    let movements = mastery::session_movements(pool, session_id, started_at).await?;
    let moved_up_count = mastery::count_moved_up(&movements);
    let card = movements.iter().find(|movement| movement.card_id == card_id);
    Ok(CardMasteryProgress {
        level_before: card.map_or(MasteryLevel::Unseen, |movement| movement.level_before),
        level_after: card.map_or(MasteryLevel::Unseen, |movement| movement.level_after),
        direction: card.map_or(MovementDirection::Unchanged, |movement| movement.direction),
        moved_up_count,
    })
}

async fn load_schedule_state(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    card_id: i64,
) -> AppResult<ScheduleState> {
    let row = sqlx::query!(
        r#"
        SELECT interval_days AS "interval_days!: f64",
               ease          AS "ease!: f64",
               reps          AS "repetitions!: i64",
               lapses        AS "lapses!: i64"
        FROM schedule WHERE card_id = ?
        "#,
        card_id,
    )
    .fetch_optional(&mut **transaction)
    .await?;

    Ok(match row {
        Some(row) => ScheduleState {
            interval_days: row.interval_days,
            ease: row.ease,
            repetitions: row.repetitions,
            lapses: row.lapses,
        },
        None => initial_state(),
    })
}

async fn write_schedule(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    card_id: i64,
    state: &ScheduleState,
    answered_at: &str,
) -> AppResult<()> {
    let interval_days = state.interval_days;
    let whole_days = state.interval_days as i64;
    sqlx::query!(
        r#"
        INSERT INTO schedule (card_id, due_at, interval_days, ease, reps, lapses)
        VALUES (
            ?,
            date(?, '+' || CAST(? AS TEXT) || ' days') || 'T00:00:00Z',
            ?, ?, ?, ?
        )
        ON CONFLICT (card_id) DO UPDATE SET
            due_at        = excluded.due_at,
            interval_days = excluded.interval_days,
            ease          = excluded.ease,
            reps          = excluded.reps,
            lapses        = excluded.lapses
        "#,
        card_id,
        answered_at,
        whole_days,
        interval_days,
        state.ease,
        state.repetitions,
        state.lapses,
    )
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn recompute_schedule_from_history(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    card_id: i64,
) -> AppResult<()> {
    let rows = sqlx::query!(
        r#"
        SELECT reviews.correct    AS "correct!: bool",
               reviews.self_grade AS "self_grade?: String",
               reviews.answered_at AS "answered_at!: String"
        FROM reviews
        JOIN sessions ON sessions.id = reviews.session_id
        WHERE reviews.card_id = ? AND sessions.mode = 'sm2'
        ORDER BY reviews.answered_at ASC, reviews.id ASC
        "#,
        card_id,
    )
    .fetch_all(&mut **transaction)
    .await?;

    let Some(last_answered_at) = rows.last().map(|row| row.answered_at.clone()) else {
        return Ok(());
    };

    let qualities: Vec<u8> = rows
        .iter()
        .map(|row| {
            quality_for(row.correct, row.self_grade.as_deref().and_then(parse_self_grade))
        })
        .collect();

    let state = replay(&qualities);
    write_schedule(transaction, card_id, &state, &last_answered_at).await
}

#[derive(Serialize)]
pub struct OverrideResponse {
    pub review_id: i64,
    pub correct: bool,
    pub overridden: bool,
    pub accepted_added: bool,
    pub expected: Vec<String>,
    pub level_after: MasteryLevel,
    pub mastery_direction: MovementDirection,
    pub mastery_moved_up_count: i64,
}

#[derive(Serialize)]
pub struct SummaryResponse {
    pub id: i64,
    pub mode: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub answered_count: i64,
    pub correct_count: i64,
    pub overridden_count: i64,
    pub distinct_card_count: i64,
    pub accuracy: Option<f64>,
    pub total_ms: i64,
    pub mastery_goal: Option<i64>,
    pub mastery_moved_up_count: i64,
    pub mastery_moved_down_count: i64,
    pub mastery_movements: Vec<SessionMasteryMovement>,
}

async fn override_review(
    State(state): State<AppState>,
    Path(review_id): Path<i64>,
) -> AppResult<Json<OverrideResponse>> {
    let review = sqlx::query!(
        r#"
        SELECT reviews.id AS "id!: i64",
               reviews.card_id AS "card_id!: i64",
               reviews.given,
               reviews.correct AS "correct!: bool",
               cards.kind,
               cards.answer_md,
               sessions.id AS "session_id!: i64",
               sessions.mode,
               sessions.started_at,
               sessions.ended_at
        FROM reviews
        JOIN cards ON cards.id = reviews.card_id
        JOIN sessions ON sessions.id = reviews.session_id
        WHERE reviews.id = ?
        "#,
        review_id,
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound("review"))?;

    if review.mode == "mock" && review.ended_at.is_none() {
        return Err(AppError::Conflict(
            "Submit the mock test before overriding an answer".to_string(),
        ));
    }

    if review.kind == "mc_single" {
        return Err(AppError::Conflict(
            "A multiple-choice answer cannot be overridden".to_string(),
        ));
    }
    if review.kind == "flashcard" && review.mode != "mock" {
        return Err(AppError::Conflict(
            "Grade the flashcard again instead of overriding it".to_string(),
        ));
    }
    if review.correct {
        return Err(AppError::Conflict(
            "That answer was already marked correct".to_string(),
        ));
    }

    let given = review.given.unwrap_or_default();
    let comparison_key = normalise(&given);
    if comparison_key.is_empty() {
        return Err(AppError::Conflict("There is no answer to accept".to_string()));
    }

    if review.kind == "flashcard" {
        sqlx::query!(
            "UPDATE reviews SET correct = 1, overridden = 1 WHERE id = ?",
            review_id,
        )
        .execute(&state.pool)
        .await?;

        let progress = card_mastery_progress(
            &state.pool,
            review.session_id,
            &review.started_at,
            review.card_id,
        )
        .await?;

        return Ok(Json(OverrideResponse {
            review_id,
            correct: true,
            overridden: true,
            accepted_added: false,
            expected: review.answer_md.into_iter().collect(),
            level_after: progress.level_after,
            mastery_direction: progress.direction,
            mastery_moved_up_count: progress.moved_up_count,
        }));
    }

    let mut transaction = state.pool.begin().await?;
    let insertion = sqlx::query!(
        r#"
        INSERT INTO accepted (card_id, text, normalised, is_primary)
        SELECT ?, ?, ?, 0
        WHERE NOT EXISTS (
            SELECT 1 FROM accepted WHERE card_id = ? AND normalised = ?
        )
        "#,
        review.card_id,
        given,
        comparison_key,
        review.card_id,
        comparison_key,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        "UPDATE reviews SET correct = 1, overridden = 1 WHERE id = ?",
        review_id,
    )
    .execute(&mut *transaction)
    .await?;

    if review.mode == "sm2" {
        recompute_schedule_from_history(&mut transaction, review.card_id).await?;
    }

    transaction.commit().await?;

    let expected = sqlx::query_scalar!(
        "SELECT text FROM accepted WHERE card_id = ? ORDER BY is_primary DESC, id",
        review.card_id,
    )
    .fetch_all(&state.pool)
    .await?;

    let progress = card_mastery_progress(
        &state.pool,
        review.session_id,
        &review.started_at,
        review.card_id,
    )
    .await?;

    Ok(Json(OverrideResponse {
        review_id,
        correct: true,
        overridden: true,
        accepted_added: insertion.rows_affected() == 1,
        expected,
        level_after: progress.level_after,
        mastery_direction: progress.direction,
        mastery_moved_up_count: progress.moved_up_count,
    }))
}

fn accuracy_for(correct_count: i64, answered_count: i64) -> Option<f64> {
    if answered_count == 0 {
        return None;
    }
    Some(correct_count as f64 / answered_count as f64)
}

async fn finish(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
) -> AppResult<Json<SummaryResponse>> {
    sqlx::query_scalar!(
        r#"SELECT id AS "id!: i64" FROM sessions WHERE id = ?"#,
        session_id,
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound("session"))?;

    sqlx::query!(
        r#"
        UPDATE sessions
        SET ended_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
        WHERE id = ? AND ended_at IS NULL
        "#,
        session_id,
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(summarise(&state.pool, session_id).await?))
}

async fn summarise(pool: &sqlx::SqlitePool, session_id: i64) -> AppResult<SummaryResponse> {
    let session = sqlx::query!(
        r#"
        SELECT id AS "id!: i64", mode, mastery_goal, started_at, ended_at
        FROM sessions WHERE id = ?
        "#,
        session_id,
    )
    .fetch_one(pool)
    .await?;

    let movements = mastery::session_movements(pool, session_id, &session.started_at).await?;

    let statistics = sqlx::query!(
        r#"
        SELECT COUNT(*)                                                    AS "answered_count!: i64",
               COALESCE(SUM(correct), 0)                                   AS "correct_count!: i64",
               COALESCE(SUM(CASE WHEN overridden = 1 THEN 1 ELSE 0 END),0) AS "overridden_count!: i64",
               COUNT(DISTINCT card_id)                                     AS "distinct_card_count!: i64",
               COALESCE(SUM(ms), 0)                                        AS "total_ms!: i64"
        FROM reviews WHERE session_id = ?
        "#,
        session_id,
    )
    .fetch_one(pool)
    .await?;

    let accuracy = accuracy_for(statistics.correct_count, statistics.answered_count);

    Ok(SummaryResponse {
        id: session.id,
        mode: session.mode,
        started_at: session.started_at,
        ended_at: session.ended_at,
        answered_count: statistics.answered_count,
        correct_count: statistics.correct_count,
        overridden_count: statistics.overridden_count,
        distinct_card_count: statistics.distinct_card_count,
        accuracy,
        total_ms: statistics.total_ms,
        mastery_goal: session.mastery_goal,
        mastery_moved_up_count: mastery::count_moved_up(&movements),
        mastery_moved_down_count: mastery::count_moved_down(&movements),
        mastery_movements: movements,
    })
}


async fn results(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
) -> AppResult<Json<ResultsResponse>> {
    let session = sqlx::query!(
        r#"SELECT id AS "id!: i64", ended_at FROM sessions WHERE id = ?"#,
        session_id,
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound("session"))?;

    if session.ended_at.is_none() {
        return Err(AppError::Conflict(
            "This session has not been submitted yet".to_string(),
        ));
    }

    let rows = sqlx::query_as!(
        ResultReviewRow,
        r#"
        SELECT reviews.id         AS "review_id!: i64",
               reviews.card_id    AS "card_id!: i64",
               reviews.answered_at,
               reviews.given,
               reviews.self_grade,
               reviews.correct    AS "correct!: bool",
               reviews.overridden AS "overridden!: bool",
               reviews.ms,
               cards.kind,
               cards.prompt_md,
               cards.image_path,
               cards.answer_md,
               cards.explanation_md
        FROM reviews
        JOIN cards ON cards.id = reviews.card_id
        WHERE reviews.session_id = ?
        ORDER BY reviews.answered_at, reviews.id
        "#,
        session_id,
    )
    .fetch_all(&state.pool)
    .await?;

    let choice_rows = sqlx::query!(
        r#"
        SELECT choices.card_id AS "card_id!: i64", choices.text_md
        FROM choices
        WHERE choices.is_correct = 1
          AND choices.card_id IN (SELECT card_id FROM reviews WHERE session_id = ?)
        ORDER BY choices.card_id, choices.position
        "#,
        session_id,
    )
    .fetch_all(&state.pool)
    .await?;

    let accepted_rows = sqlx::query!(
        r#"
        SELECT accepted.card_id AS "card_id!: i64", accepted.text
        FROM accepted
        WHERE accepted.card_id IN (SELECT card_id FROM reviews WHERE session_id = ?)
        ORDER BY accepted.card_id, accepted.is_primary DESC, accepted.id
        "#,
        session_id,
    )
    .fetch_all(&state.pool)
    .await?;

    let mut correct_choices_by_card: std::collections::HashMap<i64, Vec<String>> =
        std::collections::HashMap::new();
    for row in choice_rows {
        correct_choices_by_card.entry(row.card_id).or_default().push(row.text_md);
    }

    let mut accepted_by_card: std::collections::HashMap<i64, Vec<String>> =
        std::collections::HashMap::new();
    for row in accepted_rows {
        accepted_by_card.entry(row.card_id).or_default().push(row.text);
    }

    let questions = assemble_results(rows, &correct_choices_by_card, &accepted_by_card);
    let summary = summarise(&state.pool, session_id).await?;

    Ok(Json(ResultsResponse { summary, questions }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(create))
        .route("/sessions/{id}/next", get(next))
        .route("/sessions/{id}/reveal", post(reveal))
        .route("/sessions/{id}/answer", post(answer))
        .route("/sessions/{id}/finish", post(finish))
        .route("/sessions/{id}/results", get(results))
        .route("/reviews/{id}/override", post(override_review))
}

#[cfg(test)]
mod tests {
    use super::accuracy_for;

    #[test]
    fn accuracy_is_none_rather_than_a_division_by_zero() {
        let accuracy = accuracy_for(0, 0);
        assert!(
            accuracy.is_none(),
            "an unanswered session must report no accuracy, not a NaN that serialises to null",
        );
    }

    #[test]
    fn accuracy_is_the_correct_share_of_the_answered() {
        assert_eq!(accuracy_for(1, 2), Some(0.5));
        assert_eq!(accuracy_for(3, 3), Some(1.0));
        assert_eq!(accuracy_for(0, 4), Some(0.0));
    }
}
