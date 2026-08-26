//! Cards, and their kind-specific children.
//!
//! One `cards` table with a `kind` discriminator means the schema cannot
//! enforce per-kind invariants, so they are enforced here on every write —
//! see `validate`. Field names in the errors match the client's form
//! controls (`choices[1].text_md`) so the editor can render them inline.

use axum::extract::{Path, State};
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
pub struct ChoiceDto {
    pub id: i64,
    pub text_md: String,
    pub is_correct: bool,
    pub position: i64,
}

#[derive(Serialize)]
pub struct AcceptedDto {
    pub id: i64,
    pub text: String,
    pub normalised: String,
    pub is_primary: bool,
}

/// List row. Deliberately without children: the list never renders them and
/// loading them for a 200-card deck would be pure waste.
#[derive(Serialize)]
pub struct CardSummaryDto {
    pub id: i64,
    pub deck_id: i64,
    pub kind: String,
    pub prompt_md: String,
    pub image_path: Option<String>,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Authoring view. Returns `is_correct` — the spec's answer-key leakage rule
/// governs the session endpoints, which do not exist yet and will have their
/// own DTOs.
#[derive(Serialize)]
pub struct CardDto {
    #[serde(flatten)]
    pub card: CardSummaryDto,
    pub choices: Vec<ChoiceDto>,
    pub accepted: Vec<AcceptedDto>,
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

/// The editable content of a card. `POST` wraps this with a `deck_id`;
/// `PATCH` (Task 5) uses it as-is and replaces the card wholesale.
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

/// A card that has passed validation: trimmed, kind-consistent, ready to write.
pub struct ValidCard {
    pub kind: String,
    pub prompt_md: String,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub choices: Vec<ChoiceInput>,
    pub accepted: Vec<AcceptedInput>,
}

/// Enforces the per-kind invariants the schema cannot express (spec: "the
/// trade-off is that the schema cannot enforce per-kind invariants, so these
/// are validated in Rust on write"). Collects every problem rather than
/// stopping at the first, so the editor can highlight all of them at once.
pub fn validate(input: CardInput) -> AppResult<ValidCard> {
    let mut errors: Vec<FieldError> = Vec::new();
    let mut push = |field: &str, message: &str| {
        errors.push(FieldError { field: field.into(), message: message.into() })
    };

    if !KINDS.contains(&input.kind.as_str()) {
        // Nothing else can be judged without a kind, so fail immediately.
        return Err(AppError::validation([(
            "kind",
            "kind must be mc_single, short_answer or flashcard",
        )]));
    }

    let prompt_md = input.prompt_md.trim().to_string();
    if prompt_md.is_empty() {
        push("prompt_md", "A prompt is required");
    }

    let answer_md = input.answer_md.as_deref().map(str::trim).filter(|s| !s.is_empty())
        .map(str::to_string);
    let explanation_md = input.explanation_md.as_deref().map(str::trim)
        .filter(|s| !s.is_empty()).map(str::to_string);

    let choices: Vec<ChoiceInput> = input.choices.into_iter()
        .map(|c| ChoiceInput { text_md: c.text_md.trim().to_string(), ..c })
        .collect();
    let accepted: Vec<AcceptedInput> = input.accepted.into_iter()
        .map(|a| AcceptedInput { text: a.text.trim().to_string(), ..a })
        .collect();

    match input.kind.as_str() {
        "mc_single" => {
            if choices.len() < 2 {
                push("choices", "A multiple-choice card needs at least two options");
            }
            match choices.iter().filter(|c| c.is_correct).count() {
                1 => {}
                0 => push("choices", "Mark one option as correct"),
                _ => push("choices", "Only one option may be correct"),
            }
            for (i, c) in choices.iter().enumerate() {
                if c.text_md.is_empty() {
                    push(&format!("choices[{i}].text_md"), "An option cannot be blank");
                }
            }
            if !accepted.is_empty() {
                push("accepted", "Accepted answers belong to short-answer cards");
            }
            if answer_md.is_some() {
                push("answer_md", "An answer belongs to a flashcard");
            }
        }
        "short_answer" => {
            if accepted.is_empty() {
                push("accepted", "Add at least one accepted answer");
            }
            match accepted.iter().filter(|a| a.is_primary).count() {
                1 => {}
                0 => push("accepted", "Mark one answer as the primary wording"),
                _ => push("accepted", "Only one answer may be the primary wording"),
            }
            for (i, a) in accepted.iter().enumerate() {
                if a.text.is_empty() {
                    push(&format!("accepted[{i}].text"), "An answer cannot be blank");
                }
            }
            if !choices.is_empty() {
                push("choices", "Options belong to multiple-choice cards");
            }
            if answer_md.is_some() {
                push("answer_md", "An answer belongs to a flashcard");
            }
        }
        "flashcard" => {
            if answer_md.is_none() {
                push("answer_md", "A flashcard needs an answer");
            }
            if !choices.is_empty() {
                push("choices", "Options belong to multiple-choice cards");
            }
            if !accepted.is_empty() {
                push("accepted", "Accepted answers belong to short-answer cards");
            }
        }
        _ => unreachable!("kind was checked above"),
    }

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    Ok(ValidCard {
        kind: input.kind, prompt_md, answer_md, explanation_md, choices, accepted,
    })
}

async fn fetch_summary(pool: &sqlx::SqlitePool, id: i64) -> AppResult<CardSummaryDto> {
    sqlx::query_as!(
        CardSummaryDto,
        r#"SELECT id AS "id!: i64", deck_id AS "deck_id!: i64", kind,
                  prompt_md, image_path, answer_md, explanation_md,
                  archived AS "archived!: bool", created_at, updated_at
           FROM cards WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("card"))
}

async fn fetch_full(pool: &sqlx::SqlitePool, id: i64) -> AppResult<CardDto> {
    let card = fetch_summary(pool, id).await?;

    let choices = sqlx::query_as!(
        ChoiceDto,
        r#"SELECT id AS "id!: i64", text_md, is_correct AS "is_correct!: bool",
                  position AS "position!: i64"
           FROM choices WHERE card_id = ? ORDER BY position"#,
        id
    )
    .fetch_all(pool)
    .await?;

    let accepted = sqlx::query_as!(
        AcceptedDto,
        r#"SELECT id AS "id!: i64", text, normalised, is_primary AS "is_primary!: bool"
           FROM accepted WHERE card_id = ? ORDER BY is_primary DESC, id"#,
        id
    )
    .fetch_all(pool)
    .await?;

    Ok(CardDto { card, choices, accepted })
}

async fn get_one(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<CardDto>> {
    Ok(Json(fetch_full(&st.pool, id).await?))
}

async fn create(
    State(st): State<AppState>,
    AppJson(body): AppJson<CreateCard>,
) -> AppResult<(StatusCode, Json<CardDto>)> {
    let valid = validate(body.card)?;
    let deck_id = body.deck_id;

    let mut tx = st.pool.begin().await?;

    // Card, children and schedule row go in together or not at all: a card
    // without its choices is unanswerable, and one without a schedule row
    // would need a migration when SM-2 lands.
    let id = sqlx::query_scalar!(
        r#"INSERT INTO cards (deck_id, kind, prompt_md, answer_md, explanation_md)
           VALUES (?, ?, ?, ?, ?) RETURNING id AS "id!: i64""#,
        deck_id, valid.kind, valid.prompt_md, valid.answer_md, valid.explanation_md
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::from(e).fk_as("deck_id", "That deck does not exist"))?;

    write_children(&mut tx, id, &valid).await?;

    sqlx::query!(
        r#"INSERT INTO schedule (card_id, due_at)
           VALUES (?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))"#,
        id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(fetch_full(&st.pool, id).await?)))
}

/// Inserts the kind-appropriate children. Task 5 reuses this after deleting
/// the old ones, which is why it takes a transaction rather than a pool.
async fn write_children(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    card_id: i64,
    valid: &ValidCard,
) -> AppResult<()> {
    for (i, c) in valid.choices.iter().enumerate() {
        let position = i as i64;
        sqlx::query!(
            "INSERT INTO choices (card_id, text_md, is_correct, position)
             VALUES (?, ?, ?, ?)",
            card_id, c.text_md, c.is_correct, position
        )
        .execute(&mut **tx)
        .await?;
    }
    for a in &valid.accepted {
        // The comparison key is computed once here, on write, so grading is an
        // indexed lookup rather than a scan that re-normalises every row.
        let key = normalise(&a.text);
        sqlx::query!(
            "INSERT INTO accepted (card_id, text, normalised, is_primary)
             VALUES (?, ?, ?, ?)",
            card_id, a.text, key, a.is_primary
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cards", axum::routing::post(create))
        .route("/cards/{id}", get(get_one))
}
