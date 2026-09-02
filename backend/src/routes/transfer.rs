use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::Serialize;
use sqlx::{Sqlite, Transaction};

use crate::error::{AppError, AppResult, FieldError};
use crate::extract::AppJson;
use crate::images::{sniff, stored_name, ImageType, MAX_IMAGE_BYTES};
use crate::routes::cards::{
    validate, write_children, AcceptedInput, CardInput, ChoiceInput, ValidCard,
};
use crate::state::AppState;
use crate::transfer::{
    attachment_filename, TransferAccepted, TransferCard, TransferChoice, TransferDeck,
    TransferFile, MAX_TRANSFER_BYTES, TRANSFER_FORMAT, TRANSFER_FORMAT_VERSION,
};

#[derive(Serialize)]
pub struct ImportedDeckResponse {
    pub id: i64,
    pub name: String,
    pub original_name: String,
    pub card_count: i64,
}

#[derive(Serialize)]
pub struct ImportResponse {
    pub decks: Vec<ImportedDeckResponse>,
    pub image_count: i64,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/decks/{id}/export", get(export_deck))
        .route("/modules/{id}/export", get(export_module))
        .route(
            "/import",
            post(import).layer(DefaultBodyLimit::max(MAX_TRANSFER_BYTES)),
        )
}

struct DeckRow {
    id: i64,
    module_name: Option<String>,
    name: String,
    description: String,
}

struct CardRow {
    id: i64,
    kind: String,
    prompt_md: String,
    image_path: Option<String>,
    answer_md: Option<String>,
    explanation_md: Option<String>,
    archived: bool,
}

async fn export_deck(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Response> {
    let deck = sqlx::query_as!(
        DeckRow,
        r#"SELECT d.id AS "id!: i64", m.name AS "module_name?: String",
                  d.name, d.description
           FROM decks d
           LEFT JOIN modules m ON m.id = d.module_id
           WHERE d.id = ?"#,
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound("deck"))?;

    let filename = attachment_filename(&deck.name);
    let file = build_file(&state, vec![deck]).await?;

    attachment(&filename, &file)
}

async fn export_module(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Response> {
    let module_name = sqlx::query_scalar!("SELECT name FROM modules WHERE id = ?", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound("module"))?;

    let decks = sqlx::query_as!(
        DeckRow,
        r#"SELECT d.id AS "id!: i64", m.name AS "module_name?: String",
                  d.name, d.description
           FROM decks d
           LEFT JOIN modules m ON m.id = d.module_id
           WHERE d.module_id = ?
           ORDER BY d.created_at, d.id"#,
        id
    )
    .fetch_all(&state.pool)
    .await?;

    let filename = attachment_filename(&module_name);
    let file = build_file(&state, decks).await?;

    attachment(&filename, &file)
}

async fn build_file(state: &AppState, deck_rows: Vec<DeckRow>) -> AppResult<TransferFile> {
    let exported_at =
        sqlx::query_scalar!(r#"SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now') AS "now!: String""#)
            .fetch_one(&state.pool)
            .await?;

    let mut decks = Vec::with_capacity(deck_rows.len());

    for deck_row in deck_rows {
        let card_rows = sqlx::query_as!(
            CardRow,
            r#"SELECT id AS "id!: i64", kind, prompt_md, image_path,
                      answer_md, explanation_md, archived AS "archived!: bool"
               FROM cards
               WHERE deck_id = ?
               ORDER BY position, id"#,
            deck_row.id
        )
        .fetch_all(&state.pool)
        .await?;

        let mut cards = Vec::with_capacity(card_rows.len());

        for card_row in card_rows {
            let choices = sqlx::query!(
                r#"SELECT text_md, is_correct AS "is_correct!: bool"
                   FROM choices WHERE card_id = ? ORDER BY position, id"#,
                card_row.id
            )
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .map(|row| TransferChoice { text_md: row.text_md, is_correct: row.is_correct })
            .collect();

            let accepted = sqlx::query!(
                r#"SELECT text, is_primary AS "is_primary!: bool"
                   FROM accepted WHERE card_id = ? ORDER BY id"#,
                card_row.id
            )
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .map(|row| TransferAccepted { text: row.text, is_primary: row.is_primary })
            .collect();

            let image_base64 = match card_row.image_path.as_deref() {
                Some(path) => read_image_as_base64(state, path).await,
                None => None,
            };

            cards.push(TransferCard {
                kind: card_row.kind,
                prompt_md: card_row.prompt_md,
                answer_md: card_row.answer_md,
                explanation_md: card_row.explanation_md,
                archived: card_row.archived,
                image_base64,
                choices,
                accepted,
            });
        }

        decks.push(TransferDeck {
            module_name: deck_row.module_name,
            name: deck_row.name,
            description: deck_row.description,
            cards,
        });
    }

    Ok(TransferFile {
        format: TRANSFER_FORMAT.to_string(),
        format_version: TRANSFER_FORMAT_VERSION,
        exported_at: Some(exported_at),
        decks,
    })
}

async fn read_image_as_base64(state: &AppState, image_path: &str) -> Option<String> {
    let name = image_path.strip_prefix("images/")?;
    let source = state.images_directory.join(name);

    match tokio::fs::read(&source).await {
        Ok(bytes) => Some(BASE64.encode(bytes)),
        Err(error) => {
            tracing::warn!(
                error = ?error,
                path = ?source,
                "a card references an image that is not on disk; exporting the card without it",
            );
            None
        }
    }
}

fn attachment(filename: &str, file: &TransferFile) -> AppResult<Response> {
    let body = serde_json::to_vec_pretty(file).map_err(|error| {
        tracing::error!(error = ?error, "could not serialise the transfer file");
        AppError::Internal
    })?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response())
}

struct PreparedCard {
    valid: ValidCard,
    archived: bool,
    image: Option<PreparedImage>,
}

struct PreparedImage {
    name: String,
    bytes: Vec<u8>,
}

struct PreparedDeck {
    module_name: Option<String>,
    name: String,
    description: String,
    cards: Vec<PreparedCard>,
}

async fn import(
    State(state): State<AppState>,
    AppJson(file): AppJson<TransferFile>,
) -> AppResult<(StatusCode, Json<ImportResponse>)> {
    if file.format != TRANSFER_FORMAT {
        return Err(AppError::validation([(
            "format",
            "That file was not exported from quizapp",
        )]));
    }
    if file.format_version != TRANSFER_FORMAT_VERSION {
        return Err(AppError::validation([(
            "format_version",
            "That file was made by a different version of quizapp",
        )]));
    }
    if file.decks.is_empty() {
        return Err(AppError::validation([("decks", "That file holds no decks")]));
    }

    let prepared_decks = prepare_decks(file.decks)?;

    let image_count = write_images(&state, &prepared_decks).await?;

    let mut transaction = state.pool.begin().await?;
    let mut imported = Vec::with_capacity(prepared_decks.len());

    for prepared_deck in prepared_decks {
        imported.push(insert_deck(&mut transaction, prepared_deck).await?);
    }

    transaction.commit().await?;

    Ok((StatusCode::CREATED, Json(ImportResponse { decks: imported, image_count })))
}

fn prepare_decks(decks: Vec<TransferDeck>) -> AppResult<Vec<PreparedDeck>> {
    let mut errors: Vec<FieldError> = Vec::new();
    let mut prepared_decks = Vec::with_capacity(decks.len());

    for (deck_index, deck) in decks.into_iter().enumerate() {
        let name = deck.name.trim().to_string();
        if name.is_empty() {
            errors.push(FieldError {
                field: format!("decks[{deck_index}].name"),
                message: "A deck name is required".into(),
            });
        }

        let module_name = deck
            .module_name
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string);

        let mut cards = Vec::with_capacity(deck.cards.len());

        for (card_index, card) in deck.cards.into_iter().enumerate() {
            match prepare_card(card) {
                Ok(prepared_card) => cards.push(prepared_card),
                Err(card_errors) => errors.extend(card_errors.into_iter().map(|error| {
                    FieldError {
                        field: format!(
                            "decks[{deck_index}].cards[{card_index}].{}",
                            error.field
                        ),
                        message: error.message,
                    }
                })),
            }
        }

        prepared_decks.push(PreparedDeck {
            module_name,
            name,
            description: deck.description.trim().to_string(),
            cards,
        });
    }

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    Ok(prepared_decks)
}

fn prepare_card(card: TransferCard) -> Result<PreparedCard, Vec<FieldError>> {
    let archived = card.archived;

    let image = match card.image_base64.as_deref() {
        Some(encoded) => match decode_image(encoded) {
            Ok(prepared_image) => Some(prepared_image),
            Err(message) => {
                return Err(vec![FieldError { field: "image_base64".into(), message }]);
            }
        },
        None => None,
    };

    let input = CardInput {
        kind: card.kind,
        prompt_md: card.prompt_md,
        answer_md: card.answer_md,
        explanation_md: card.explanation_md,
        image_path: image.as_ref().map(|prepared| format!("images/{}", prepared.name)),
        choices: card
            .choices
            .into_iter()
            .map(|choice| ChoiceInput {
                text_md: choice.text_md,
                is_correct: choice.is_correct,
            })
            .collect(),
        accepted: card
            .accepted
            .into_iter()
            .map(|answer| AcceptedInput {
                text: answer.text,
                is_primary: answer.is_primary,
            })
            .collect(),
    };

    match validate(input) {
        Ok(valid) => Ok(PreparedCard { valid, archived, image }),
        Err(AppError::Validation(errors)) => Err(errors),
        Err(_) => Err(vec![FieldError {
            field: "kind".into(),
            message: "That card could not be read".into(),
        }]),
    }
}

fn decode_image(encoded: &str) -> Result<PreparedImage, String> {
    let compact: String = encoded.chars().filter(|character| !character.is_whitespace()).collect();

    let bytes = BASE64
        .decode(compact)
        .map_err(|_| "That image is not valid base64".to_string())?;

    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("That image is larger than 5 MB".to_string());
    }

    let image_type: ImageType =
        sniff(&bytes).ok_or_else(|| "That image is not a PNG, JPEG or WebP".to_string())?;

    Ok(PreparedImage { name: stored_name(&bytes, image_type), bytes })
}

async fn write_images(state: &AppState, decks: &[PreparedDeck]) -> AppResult<i64> {
    let mut written = 0;

    for deck in decks {
        for card in &deck.cards {
            let Some(image) = card.image.as_ref() else { continue };
            written += 1;

            let destination = state.images_directory.join(&image.name);
            if destination.exists() {
                continue;
            }

            tokio::fs::write(&destination, &image.bytes).await.map_err(|error| {
                tracing::error!(
                    error = ?error,
                    path = ?destination,
                    "could not write an imported image",
                );
                AppError::Internal
            })?;
        }
    }

    Ok(written)
}

async fn insert_deck(
    transaction: &mut Transaction<'_, Sqlite>,
    deck: PreparedDeck,
) -> AppResult<ImportedDeckResponse> {
    let module_id = match deck.module_name.as_deref() {
        Some(module_name) => Some(resolve_module(transaction, module_name).await?),
        None => None,
    };

    let name = free_deck_name(transaction, module_id, &deck.name).await?;

    let deck_id = sqlx::query_scalar!(
        r#"INSERT INTO decks (module_id, name, description)
           VALUES (?, ?, ?) RETURNING id AS "id!: i64""#,
        module_id,
        name,
        deck.description
    )
    .fetch_one(&mut **transaction)
    .await?;

    let card_count = deck.cards.len() as i64;

    for (card_index, card) in deck.cards.iter().enumerate() {
        let position = card_index as i64;

        let card_id = sqlx::query_scalar!(
            r#"INSERT INTO cards (deck_id, kind, prompt_md, image_path, answer_md,
                                  explanation_md, archived, position)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id AS "id!: i64""#,
            deck_id,
            card.valid.kind,
            card.valid.prompt_md,
            card.valid.image_path,
            card.valid.answer_md,
            card.valid.explanation_md,
            card.archived,
            position
        )
        .fetch_one(&mut **transaction)
        .await?;

        write_children(transaction, card_id, &card.valid).await?;

        sqlx::query!(
            r#"INSERT INTO schedule (card_id, due_at)
               VALUES (?, strftime('%Y-%m-%dT%H:%M:%SZ','now'))"#,
            card_id
        )
        .execute(&mut **transaction)
        .await?;
    }

    Ok(ImportedDeckResponse {
        id: deck_id,
        name,
        original_name: deck.name,
        card_count,
    })
}

async fn resolve_module(
    transaction: &mut Transaction<'_, Sqlite>,
    module_name: &str,
) -> AppResult<i64> {
    let existing = sqlx::query_scalar!(
        r#"SELECT id AS "id!: i64" FROM modules WHERE name = ?"#,
        module_name
    )
    .fetch_optional(&mut **transaction)
    .await?;

    if let Some(module_id) = existing {
        return Ok(module_id);
    }

    Ok(sqlx::query_scalar!(
        r#"INSERT INTO modules (name) VALUES (?) RETURNING id AS "id!: i64""#,
        module_name
    )
    .fetch_one(&mut **transaction)
    .await?)
}

async fn free_deck_name(
    transaction: &mut Transaction<'_, Sqlite>,
    module_id: Option<i64>,
    wanted: &str,
) -> AppResult<String> {
    let mut attempt = 1;

    loop {
        let candidate =
            if attempt == 1 { wanted.to_string() } else { format!("{wanted} ({attempt})") };

        let taken = sqlx::query_scalar!(
            r#"SELECT 1 AS "taken!: i64" FROM decks
               WHERE ifnull(module_id, -1) = ifnull(?, -1) AND name = ?"#,
            module_id,
            candidate
        )
        .fetch_optional(&mut **transaction)
        .await?
        .is_some();

        if !taken {
            return Ok(candidate);
        }

        attempt += 1;
    }
}
