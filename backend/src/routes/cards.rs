use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, FieldError};
use crate::extract::AppJson;
use crate::normalise::normalise;
use crate::state::AppState;

const KINDS: [&str; 3] = ["mc_single", "short_answer", "flashcard"];

#[derive(Serialize)]
pub struct ChoiceResponse {
    pub id: i64,
    pub text_md: String,
    pub is_correct: bool,
    pub position: i64,
}

#[derive(Serialize)]
pub struct AcceptedResponse {
    pub id: i64,
    pub text: String,
    pub normalised: String,
    pub is_primary: bool,
}

#[derive(Serialize)]
pub struct CardSummaryResponse {
    pub id: i64,
    pub deck_id: i64,
    pub kind: String,
    pub prompt_md: String,
    pub image_path: Option<String>,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub archived: bool,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct CardResponse {
    #[serde(flatten)]
    pub card: CardSummaryResponse,
    pub choices: Vec<ChoiceResponse>,
    pub accepted: Vec<AcceptedResponse>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceInput {
    pub text_md: String,
    #[serde(default)]
    pub is_correct: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedInput {
    pub text: String,
    #[serde(default)]
    pub is_primary: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardInput {
    pub kind: String,
    pub prompt_md: String,
    #[serde(default)]
    pub answer_md: Option<String>,
    #[serde(default)]
    pub explanation_md: Option<String>,
    #[serde(default)]
    pub image_path: Option<String>,
    #[serde(default)]
    pub choices: Vec<ChoiceInput>,
    #[serde(default)]
    pub accepted: Vec<AcceptedInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCard {
    pub deck_id: i64,
    #[serde(flatten)]
    pub card: CardInput,
}

pub struct ValidCard {
    pub kind: String,
    pub prompt_md: String,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub image_path: Option<String>,
    pub choices: Vec<ChoiceInput>,
    pub accepted: Vec<AcceptedInput>,
}

fn is_uploaded_image_path(path: &str) -> bool {
    let Some(remainder) = path.strip_prefix("images/") else { return false };
    let Some((stem, extension)) = remainder.rsplit_once('.') else { return false };

    stem.len() == 16
        && stem.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        && ["png", "jpg", "webp"].contains(&extension)
}

pub fn validate(input: CardInput) -> AppResult<ValidCard> {
    let mut errors: Vec<FieldError> = Vec::new();
    let mut push_error = |field: &str, message: &str| {
        errors.push(FieldError { field: field.into(), message: message.into() })
    };

    if !KINDS.contains(&input.kind.as_str()) {
        return Err(AppError::validation([(
            "kind",
            "kind must be mc_single, short_answer or flashcard",
        )]));
    }

    let prompt_md = input.prompt_md.trim().to_string();
    if prompt_md.is_empty() {
        push_error("prompt_md", "A prompt is required");
    }

    let answer_md = input.answer_md.as_deref().map(str::trim)
        .filter(|text| !text.is_empty()).map(str::to_string);
    let explanation_md = input.explanation_md.as_deref().map(str::trim)
        .filter(|text| !text.is_empty()).map(str::to_string);

    let image_path = input.image_path.as_deref().map(str::trim)
        .filter(|text| !text.is_empty()).map(str::to_string);
    if image_path.as_deref().is_some_and(|path| !is_uploaded_image_path(path)) {
        push_error("image_path", "That is not an uploaded image");
    }

    let choices: Vec<ChoiceInput> = input.choices.into_iter()
        .map(|choice| ChoiceInput { text_md: choice.text_md.trim().to_string(), ..choice })
        .collect();
    let accepted: Vec<AcceptedInput> = input.accepted.into_iter()
        .map(|answer| AcceptedInput { text: answer.text.trim().to_string(), ..answer })
        .collect();

    match input.kind.as_str() {
        "mc_single" => {
            if choices.len() < 2 {
                push_error("choices", "A multiple-choice card needs at least two options");
            }
            match choices.iter().filter(|choice| choice.is_correct).count() {
                1 => {}
                0 => push_error("choices", "Mark one option as correct"),
                _ => push_error("choices", "Only one option may be correct"),
            }
            for (choice_index, choice) in choices.iter().enumerate() {
                if choice.text_md.is_empty() {
                    push_error(
                        &format!("choices[{choice_index}].text_md"),
                        "An option cannot be blank",
                    );
                }
            }
            if !accepted.is_empty() {
                push_error("accepted", "Accepted answers belong to short-answer cards");
            }
            if answer_md.is_some() {
                push_error("answer_md", "An answer belongs to a flashcard");
            }
        }
        "short_answer" => {
            if accepted.is_empty() {
                push_error("accepted", "Add at least one accepted answer");
            }
            match accepted.iter().filter(|answer| answer.is_primary).count() {
                1 => {}
                0 => push_error("accepted", "Mark one answer as the primary wording"),
                _ => push_error("accepted", "Only one answer may be the primary wording"),
            }
            for (answer_index, answer) in accepted.iter().enumerate() {
                if answer.text.is_empty() {
                    push_error(
                        &format!("accepted[{answer_index}].text"),
                        "An answer cannot be blank",
                    );
                }
            }
            if !choices.is_empty() {
                push_error("choices", "Options belong to multiple-choice cards");
            }
            if answer_md.is_some() {
                push_error("answer_md", "An answer belongs to a flashcard");
            }
        }
        "flashcard" => {
            if answer_md.is_none() {
                push_error("answer_md", "A flashcard needs an answer");
            }
            if !choices.is_empty() {
                push_error("choices", "Options belong to multiple-choice cards");
            }
            if !accepted.is_empty() {
                push_error("accepted", "Accepted answers belong to short-answer cards");
            }
        }
        _ => unreachable!("kind was checked above"),
    }

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    Ok(ValidCard {
        kind: input.kind, prompt_md, image_path, answer_md, explanation_md, choices, accepted,
    })
}

async fn fetch_summary(pool: &sqlx::SqlitePool, id: i64) -> AppResult<CardSummaryResponse> {
    sqlx::query_as!(
        CardSummaryResponse,
        r#"SELECT id AS "id!: i64", deck_id AS "deck_id!: i64", kind,
                  prompt_md, image_path, answer_md, explanation_md,
                  archived AS "archived!: bool", position AS "position!: i64",
                  created_at, updated_at
           FROM cards WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("card"))
}

async fn fetch_full(pool: &sqlx::SqlitePool, id: i64) -> AppResult<CardResponse> {
    let card = fetch_summary(pool, id).await?;

    let choices = sqlx::query_as!(
        ChoiceResponse,
        r#"SELECT id AS "id!: i64", text_md, is_correct AS "is_correct!: bool",
                  position AS "position!: i64"
           FROM choices WHERE card_id = ? ORDER BY position"#,
        id
    )
    .fetch_all(pool)
    .await?;

    let accepted = sqlx::query_as!(
        AcceptedResponse,
        r#"SELECT id AS "id!: i64", text, normalised, is_primary AS "is_primary!: bool"
           FROM accepted WHERE card_id = ? ORDER BY is_primary DESC, id"#,
        id
    )
    .fetch_all(pool)
    .await?;

    Ok(CardResponse { card, choices, accepted })
}

async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<CardResponse>> {
    Ok(Json(fetch_full(&state.pool, id).await?))
}

async fn patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    AppJson(body): AppJson<CardInput>,
) -> AppResult<Json<CardResponse>> {
    fetch_summary(&state.pool, id).await?;
    let valid = validate(body)?;

    let mut transaction = state.pool.begin().await?;

    sqlx::query!(
        r#"UPDATE cards
              SET kind = ?, prompt_md = ?, image_path = ?, answer_md = ?, explanation_md = ?,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = ?"#,
        valid.kind, valid.prompt_md, valid.image_path, valid.answer_md, valid.explanation_md,
        id
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!("DELETE FROM choices WHERE card_id = ?", id)
        .execute(&mut *transaction).await?;
    sqlx::query!("DELETE FROM accepted WHERE card_id = ?", id)
        .execute(&mut *transaction).await?;
    write_children(&mut transaction, id, &valid).await?;

    transaction.commit().await?;

    Ok(Json(fetch_full(&state.pool, id).await?))
}

async fn archive(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<CardResponse>> {
    set_archived(&state, id, true).await
}

async fn unarchive(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<CardResponse>> {
    set_archived(&state, id, false).await
}

async fn set_archived(
    state: &AppState,
    id: i64,
    archived: bool,
) -> AppResult<Json<CardResponse>> {
    fetch_summary(&state.pool, id).await?;

    sqlx::query!(
        r#"UPDATE cards
              SET archived = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = ?"#,
        archived, id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(fetch_full(&state.pool, id).await?))
}

async fn delete_card(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    fetch_summary(&state.pool, id).await?;

    sqlx::query!("DELETE FROM cards WHERE id = ?", id)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn move_card(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    AppJson(body): AppJson<MoveCard>,
) -> AppResult<Json<CardResponse>> {
    let card = fetch_summary(&state.pool, id).await?;

    if body.before == Some(id) {
        return Err(AppError::validation([(
            "before",
            "A card cannot move before itself",
        )]));
    }

    let mut transaction = state.pool.begin().await?;

    let mut card_ids: Vec<i64> = sqlx::query_scalar!(
        r#"SELECT id AS "id!: i64" FROM cards
           WHERE deck_id = ? ORDER BY position ASC, id ASC"#,
        card.deck_id
    )
    .fetch_all(&mut *transaction)
    .await?;

    if let Some(before) = body.before {
        if !card_ids.contains(&before) {
            return Err(AppError::validation([(
                "before",
                "That card is not in this deck",
            )]));
        }
    }

    card_ids.retain(|&other_card_id| other_card_id != id);
    match body.before {
        Some(before) => {
            let Some(insertion_index) =
                card_ids.iter().position(|&other_card_id| other_card_id == before)
            else {
                return Err(AppError::validation([(
                    "before",
                    "That card is not in this deck",
                )]));
            };
            card_ids.insert(insertion_index, id);
        }
        None => card_ids.push(id),
    }

    for (index, card_id) in card_ids.iter().enumerate() {
        let position = index as i64;
        sqlx::query!("UPDATE cards SET position = ? WHERE id = ?", position, card_id)
            .execute(&mut *transaction)
            .await?;
    }

    transaction.commit().await?;

    Ok(Json(fetch_full(&state.pool, id).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveCard {
    pub before: Option<i64>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub deck_id: Option<String>,
    pub kind: Option<String>,
    pub archived: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Query(list_query): Query<ListQuery>,
) -> AppResult<Json<Vec<CardSummaryResponse>>> {
    let deck_id = match list_query.deck_id.as_deref() {
        None | Some("") => None,
        Some(raw) => Some(raw.parse::<i64>().map_err(|_| {
            AppError::validation([("deck_id", "deck_id must be a number")])
        })?),
    };

    let kind = list_query.kind.as_deref().unwrap_or("all").to_string();
    if kind != "all" && !KINDS.contains(&kind.as_str()) {
        return Err(AppError::validation([(
            "kind",
            "kind must be mc_single, short_answer, flashcard or \"all\"",
        )]));
    }

    let archived = list_query.archived.as_deref().unwrap_or("false").to_string();
    if !["false", "true", "all"].contains(&archived.as_str()) {
        return Err(AppError::validation([(
            "archived",
            "archived must be \"true\", \"false\" or \"all\"",
        )]));
    }

    let rows = sqlx::query_as!(
        CardSummaryResponse,
        r#"SELECT id AS "id!: i64", deck_id AS "deck_id!: i64", kind,
                  prompt_md, image_path, answer_md, explanation_md,
                  archived AS "archived!: bool", position AS "position!: i64",
                  created_at, updated_at
           FROM cards
           WHERE (? IS NULL OR deck_id = ?)
             AND (? = 'all' OR kind = ?)
             AND (? = 'all'
                  OR (? = 'true'  AND archived = 1)
                  OR (? = 'false' AND archived = 0))
           ORDER BY position ASC, id ASC"#,
        deck_id, deck_id, kind, kind, archived, archived, archived
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

async fn create(
    State(state): State<AppState>,
    AppJson(body): AppJson<CreateCard>,
) -> AppResult<(StatusCode, Json<CardResponse>)> {
    let valid = validate(body.card)?;
    let deck_id = body.deck_id;

    let mut transaction = state.pool.begin().await?;

    let position = sqlx::query_scalar!(
        r#"SELECT COALESCE(MAX(position), -1) + 1 AS "next!: i64"
           FROM cards WHERE deck_id = ?"#,
        deck_id
    )
    .fetch_one(&mut *transaction)
    .await?;

    let id = sqlx::query_scalar!(
        r#"INSERT INTO cards (deck_id, kind, prompt_md, image_path, answer_md,
                              explanation_md, position)
           VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id AS "id!: i64""#,
        deck_id, valid.kind, valid.prompt_md, valid.image_path, valid.answer_md,
        valid.explanation_md, position
    )
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| {
        AppError::from(error)
            .tag_foreign_key_violation("deck_id", "That deck does not exist")
    })?;

    write_children(&mut transaction, id, &valid).await?;

    sqlx::query!(
        r#"INSERT INTO schedule (card_id, due_at)
           VALUES (?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))"#,
        id
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok((StatusCode::CREATED, Json(fetch_full(&state.pool, id).await?)))
}

async fn write_children(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    card_id: i64,
    valid: &ValidCard,
) -> AppResult<()> {
    for (index, choice) in valid.choices.iter().enumerate() {
        let position = index as i64;
        sqlx::query!(
            "INSERT INTO choices (card_id, text_md, is_correct, position)
             VALUES (?, ?, ?, ?)",
            card_id, choice.text_md, choice.is_correct, position
        )
        .execute(&mut **transaction)
        .await?;
    }
    for answer in &valid.accepted {
        let comparison_key = normalise(&answer.text);
        sqlx::query!(
            "INSERT INTO accepted (card_id, text, normalised, is_primary)
             VALUES (?, ?, ?, ?)",
            card_id, answer.text, comparison_key, answer.is_primary
        )
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cards", get(list).post(create))
        .route("/cards/{id}", get(get_one).patch(patch).delete(delete_card))
        .route("/cards/{id}/archive", axum::routing::post(archive))
        .route("/cards/{id}/unarchive", axum::routing::post(unarchive))
        .route("/cards/{id}/move", axum::routing::post(move_card))
}
