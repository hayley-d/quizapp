use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{AppError, AppResult};
use crate::extract::AppJson;
use crate::state::AppState;

#[derive(Serialize)]
pub struct DeckResponse {
    pub id: i64,
    pub module_id: Option<i64>,
    pub module_name: Option<String>,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub card_count: i64,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub module_id: Option<String>,
    pub search: Option<String>,
    pub sort: Option<String>,
}

fn escape_like_metacharacters(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateDeck {
    pub name: String,
    #[serde(default)]
    pub module_id: Option<i64>,
    #[serde(default)]
    pub description: Option<String>,
}

fn distinguish_absent_from_null<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchDeck {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "distinguish_absent_from_null")]
    pub module_id: Option<Option<i64>>,
    #[serde(default)]
    pub description: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/decks", get(list).post(create))
        .route("/decks/{id}", get(get_one).patch(patch))
}

async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<DeckResponse>> {
    Ok(Json(fetch_one(&state.pool, id).await?))
}

async fn fetch_one(pool: &sqlx::SqlitePool, id: i64) -> AppResult<DeckResponse> {
    sqlx::query_as!(
        DeckResponse,
        r#"SELECT d.id AS "id!: i64",
                  d.module_id AS "module_id?: i64",
                  m.name      AS "module_name?: String",
                  d.name, d.description, d.created_at,
                  (SELECT COUNT(*) FROM cards c WHERE c.deck_id = d.id AND c.archived = 0)
                      AS "card_count!: i64"
           FROM decks d
           LEFT JOIN modules m ON m.id = d.module_id
           WHERE d.id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("deck"))
}

async fn list(
    State(state): State<AppState>,
    Query(list_query): Query<ListQuery>,
) -> AppResult<Json<Vec<DeckResponse>>> {
    let search_pattern = list_query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|search_text| !search_text.is_empty())
        .map(escape_like_metacharacters);

    let sort = list_query.sort.as_deref().unwrap_or("newest").to_string();
    if sort != "newest" && sort != "oldest" {
        return Err(AppError::validation([(
            "sort",
            "sort must be \"newest\" or \"oldest\"",
        )]));
    }

    let (module_filter_mode, module_id) = match list_query.module_id.as_deref() {
        None | Some("") | Some("all") => ("all".to_string(), None),
        Some("none") => ("none".to_string(), None),
        Some(raw) => {
            let parsed_module_id: i64 = raw.parse().map_err(|_| {
                AppError::validation([(
                    "module_id",
                    "module_id must be a number, \"none\" or \"all\"",
                )])
            })?;
            ("id".to_string(), Some(parsed_module_id))
        }
    };

    let rows = sqlx::query_as!(
        DeckResponse,
        r#"SELECT d.id AS "id!: i64",
                  d.module_id AS "module_id?: i64",
                  m.name      AS "module_name?: String",
                  d.name, d.description, d.created_at,
                  (SELECT COUNT(*) FROM cards c
                    WHERE c.deck_id = d.id AND c.archived = 0) AS "card_count!: i64"
           FROM decks d
           LEFT JOIN modules m ON m.id = d.module_id
           WHERE (? IS NULL OR d.name LIKE '%' || ? || '%' ESCAPE '\')
             AND (? = 'all'
                  OR (? = 'none' AND d.module_id IS NULL)
                  OR d.module_id = ?)
           ORDER BY CASE WHEN ? = 'oldest' THEN d.created_at END ASC,
                    CASE WHEN ? = 'newest' THEN d.created_at END DESC,
                    CASE WHEN ? = 'oldest' THEN d.id END ASC,
                    CASE WHEN ? = 'newest' THEN d.id END DESC"#,
        search_pattern,
        search_pattern,
        module_filter_mode,
        module_filter_mode,
        module_id,
        sort,
        sort,
        sort,
        sort
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

async fn create(
    State(state): State<AppState>,
    AppJson(body): AppJson<CreateDeck>,
) -> AppResult<(StatusCode, Json<DeckResponse>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::validation([("name", "Name must not be empty")]));
    }
    let description = body.description.unwrap_or_default().trim().to_string();

    let id = sqlx::query_scalar!(
        "INSERT INTO decks (module_id, name, description) VALUES (?, ?, ?) RETURNING id",
        body.module_id,
        name,
        description
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|error| {
        AppError::from(error)
            .tag_foreign_key_violation("module_id", "That module does not exist")
    })?;

    Ok((StatusCode::CREATED, Json(fetch_one(&state.pool, id).await?)))
}

async fn patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    AppJson(body): AppJson<PatchDeck>,
) -> AppResult<Json<DeckResponse>> {
    let current = fetch_one(&state.pool, id).await?;

    let name = match body.name {
        Some(submitted_name) => {
            let trimmed_name = submitted_name.trim().to_string();
            if trimmed_name.is_empty() {
                return Err(AppError::validation([("name", "Name must not be empty")]));
            }
            trimmed_name
        }
        None => current.name,
    };
    let module_id = match body.module_id {
        Some(submitted_module_id) => submitted_module_id,
        None => current.module_id,
    };
    let description = match body.description {
        Some(submitted_description) => submitted_description.trim().to_string(),
        None => current.description,
    };

    sqlx::query!(
        "UPDATE decks SET module_id = ?, name = ?, description = ? WHERE id = ?",
        module_id,
        name,
        description,
        id
    )
    .execute(&state.pool)
    .await
    .map_err(|error| {
        AppError::from(error)
            .tag_foreign_key_violation("module_id", "That module does not exist")
    })?;

    Ok(Json(fetch_one(&state.pool, id).await?))
}
