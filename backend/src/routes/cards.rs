//! Cards, and their kind-specific children.
//!
//! One `cards` table with a `kind` discriminator means the schema cannot
//! enforce per-kind invariants, so they are enforced here on every write —
//! see `validate`. Field names in the errors match the client's form
//! controls (`choices[1].text_md`) so the editor can render them inline.

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
/// `patch` uses it as-is and replaces the card wholesale.
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

/// A card that has passed validation: trimmed, kind-consistent, ready to write.
pub struct ValidCard {
    pub kind: String,
    pub prompt_md: String,
    pub answer_md: Option<String>,
    pub explanation_md: Option<String>,
    pub image_path: Option<String>,
    pub choices: Vec<ChoiceInput>,
    pub accepted: Vec<AcceptedInput>,
}

/// True only for a path `POST /api/images` could have produced:
/// `images/<16 lowercase hex>.<png|jpg|webp>`.
///
/// The server assigns every legitimate value of this field, so anything
/// outside that shape did not come from an upload. It is worth a guard rather
/// than a shrug because the string is handed straight back to the browser as
/// a URL. Hand-rolled instead of pulling in `regex`: this is the only pattern
/// the codebase matches, and the crate would be the larger change. The
/// extension list must stay in step with `images::ImageType::extension`.
fn is_uploaded_image_path(p: &str) -> bool {
    let Some(rest) = p.strip_prefix("images/") else { return false };
    let Some((stem, ext)) = rest.rsplit_once('.') else { return false };

    stem.len() == 16
        && stem.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        && ["png", "jpg", "webp"].contains(&ext)
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

    // An empty string means "no image" — the editor clears the field rather
    // than deleting the key — so it is filtered out before the shape check.
    let image_path = input.image_path.as_deref().map(str::trim)
        .filter(|s| !s.is_empty()).map(str::to_string);
    if image_path.as_deref().is_some_and(|p| !is_uploaded_image_path(p)) {
        push("image_path", "That is not an uploaded image");
    }

    let choices: Vec<ChoiceInput> = input.choices.into_iter()
        .map(|c| ChoiceInput { text_md: c.text_md.trim().to_string(), ..c })
        .collect();
    let accepted: Vec<AcceptedInput> = input.accepted.into_iter()
        .map(|a| AcceptedInput { text: a.text.trim().to_string(), ..a })
        .collect();

    // Inside each arm below: cardinality errors (`choices`/`accepted` as a
    // whole) are pushed BEFORE the per-row errors (`choices[i].text_md` etc)
    // ON PURPOSE — several tests assert on `fields[0]`. Reordering the
    // `push` calls within an arm will silently break those tests.
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
        kind: input.kind, prompt_md, image_path, answer_md, explanation_md, choices, accepted,
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

/// Full replace of a card's editable content.
///
/// Not a field-by-field patch, and deliberately not the absent-vs-null dance
/// `PATCH /api/decks/:id` needs: the editor always holds the whole card and
/// always submits the whole card, so an omitted optional means null. It is a
/// PATCH by route because the spec's API table says so. Cards do not move
/// between decks in 2a.
async fn patch(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    AppJson(body): AppJson<CardInput>,
) -> AppResult<Json<CardDto>> {
    // 404 before validation and before any write, matching decks::patch.
    fetch_summary(&st.pool, id).await?;
    let valid = validate(body)?;

    let mut tx = st.pool.begin().await?;

    sqlx::query!(
        r#"UPDATE cards
              SET kind = ?, prompt_md = ?, image_path = ?, answer_md = ?, explanation_md = ?,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = ?"#,
        valid.kind, valid.prompt_md, valid.image_path, valid.answer_md, valid.explanation_md,
        id
    )
    .execute(&mut *tx)
    .await?;

    // Both child tables are cleared regardless of kind: a kind change must not
    // leave rows that would resurface if the kind changed back.
    sqlx::query!("DELETE FROM choices WHERE card_id = ?", id).execute(&mut *tx).await?;
    sqlx::query!("DELETE FROM accepted WHERE card_id = ?", id).execute(&mut *tx).await?;
    write_children(&mut tx, id, &valid).await?;

    tx.commit().await?;

    Ok(Json(fetch_full(&st.pool, id).await?))
}

async fn archive(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<CardDto>> {
    set_archived(&st, id, true).await
}

async fn unarchive(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<CardDto>> {
    set_archived(&st, id, false).await
}

/// Cards are archived, never deleted: a hard delete would orphan the card's
/// `reviews` rows and silently rewrite history. `reviews.card_id` has no
/// ON DELETE CASCADE for the same reason.
// NB the pre-check below is not observable from the HTTP surface: an UPDATE
// against a nonexistent id matches zero rows and has no side effect, and
// fetch_full() unconditionally re-runs fetch_summary() at the end anyway, so
// a black-box test cannot tell this function apart from one that skips the
// pre-check and lets the final fetch_summary() supply the 404. It is kept
// because it avoids a wasted UPDATE and matches the 404-before-write shape
// of patch() above, not because a test proves it.
async fn set_archived(st: &AppState, id: i64, archived: bool) -> AppResult<Json<CardDto>> {
    fetch_summary(&st.pool, id).await?; // 404 before the write

    sqlx::query!(
        r#"UPDATE cards
              SET archived = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
            WHERE id = ?"#,
        archived, id
    )
    .execute(&st.pool)
    .await?;

    Ok(Json(fetch_full(&st.pool, id).await?))
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub deck_id: Option<String>,
    /// "all" (default) or one of the three kinds.
    pub kind: Option<String>,
    /// "false" (default), "true", or "all".
    pub archived: Option<String>,
}

async fn list(
    State(st): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<CardSummaryDto>>> {
    let deck_id = match q.deck_id.as_deref() {
        None | Some("") => None,
        Some(raw) => Some(raw.parse::<i64>().map_err(|_| {
            AppError::validation([("deck_id", "deck_id must be a number")])
        })?),
    };

    let kind = q.kind.as_deref().unwrap_or("all").to_string();
    if kind != "all" && !KINDS.contains(&kind.as_str()) {
        return Err(AppError::validation([(
            "kind",
            "kind must be mc_single, short_answer, flashcard or \"all\"",
        )]));
    }

    let archived = q.archived.as_deref().unwrap_or("false").to_string();
    if !["false", "true", "all"].contains(&archived.as_str()) {
        return Err(AppError::validation([(
            "archived",
            "archived must be \"true\", \"false\" or \"all\"",
        )]));
    }

    // Oldest first: a deck reads in the order it was written. Timestamps have
    // one-second resolution, so a burst of save-and-next cards all share a
    // created_at and the `id ASC` tiebreak is needed to put them back in
    // authoring order. On this schema/engine no black-box test can actually
    // discriminate it: cards.id is the rowid, every INSERT assigns it in true
    // insertion order, and SQLite scans a rowid B-tree (or the trailing-rowid
    // idx_cards_deck_archived index) in ascending rowid order regardless, so
    // ties already coincide with `id ASC` without it being named. It is kept
    // anyway as a determinism guarantee against a future index or query-plan
    // change that would break that coincidence.
    let rows = sqlx::query_as!(
        CardSummaryDto,
        r#"SELECT id AS "id!: i64", deck_id AS "deck_id!: i64", kind,
                  prompt_md, image_path, answer_md, explanation_md,
                  archived AS "archived!: bool", created_at, updated_at
           FROM cards
           WHERE (? IS NULL OR deck_id = ?)
             AND (? = 'all' OR kind = ?)
             AND (? = 'all'
                  OR (? = 'true'  AND archived = 1)
                  OR (? = 'false' AND archived = 0))
           ORDER BY created_at ASC, id ASC"#,
        deck_id, deck_id, kind, kind, archived, archived, archived
    )
    .fetch_all(&st.pool)
    .await?;

    Ok(Json(rows))
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
    // would need a migration when SM-2 lands. Note: `validate` above already
    // rejects every malformed body before this transaction opens, so there is
    // currently no reachable path where a child or schedule insert fails
    // after the card row has been written — the only failure inside the
    // transaction today is the deck_id foreign key on the very first insert.
    // The transaction still guards future code paths (and keeps the three
    // writes atomic if that changes), but no test today exercises a rollback
    // of a partial write; see the comment on `a_rejected_create_writes_nothing`.
    let id = sqlx::query_scalar!(
        r#"INSERT INTO cards (deck_id, kind, prompt_md, image_path, answer_md, explanation_md)
           VALUES (?, ?, ?, ?, ?, ?) RETURNING id AS "id!: i64""#,
        deck_id, valid.kind, valid.prompt_md, valid.image_path, valid.answer_md,
        valid.explanation_md
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

/// Inserts the kind-appropriate children. Reused by `patch`, which deletes
/// the old ones first and calls this again — which is why this takes a
/// Transaction rather than a Pool.
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
        .route("/cards", get(list).post(create))
        .route("/cards/{id}", get(get_one).patch(patch))
        .route("/cards/{id}/archive", axum::routing::post(archive))
        .route("/cards/{id}/unarchive", axum::routing::post(unarchive))
}
