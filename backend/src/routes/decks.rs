use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::{AppError, AppResult};
use crate::extract::AppJson;
use crate::state::AppState;

#[derive(Serialize)]
pub struct DeckDto {
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
    /// Numeric module id, the literal "none" for unparented decks, or "all"/absent.
    pub module_id: Option<String>,
    /// Case-insensitive substring match on the deck NAME only.
    pub q: Option<String>,
    /// "newest" (default) or "oldest", by created_at.
    pub sort: Option<String>,
}

/// Escapes LIKE metacharacters so a user searching for "100%" does not match everything.
/// Pairs with `ESCAPE '\'` in the SQL.
fn escape_like(raw: &str) -> String {
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

/// Distinguishes "key absent" (None) from "key present and null" (Some(None)).
fn some_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(d).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchDeck {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "some_option")]
    pub module_id: Option<Option<i64>>,
    #[serde(default)]
    pub description: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/decks", get(list).post(create))
        .route("/decks/{id}", get(get_one).patch(patch))
}

async fn get_one(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<DeckDto>> {
    Ok(Json(fetch_one(&st.pool, id).await?))
}

async fn fetch_one(pool: &sqlx::SqlitePool, id: i64) -> AppResult<DeckDto> {
    sqlx::query_as!(
        DeckDto,
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
    State(st): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<DeckDto>>> {
    // An empty q means "no filter", same as absent — an empty search box must not
    // filter everything out.
    let needle = q
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(escape_like);

    let sort = q.sort.as_deref().unwrap_or("newest").to_string();
    if sort != "newest" && sort != "oldest" {
        return Err(AppError::validation([(
            "sort",
            "sort must be \"newest\" or \"oldest\"",
        )]));
    }

    // mode selects the module branch; module_id is only consulted when mode is "id".
    let (mode, module_id) = match q.module_id.as_deref() {
        None | Some("") | Some("all") => ("all".to_string(), None),
        Some("none") => ("none".to_string(), None),
        Some(raw) => {
            let mid: i64 = raw.parse().map_err(|_| {
                AppError::validation([(
                    "module_id",
                    "module_id must be a number, \"none\" or \"all\"",
                )])
            })?;
            ("id".to_string(), Some(mid))
        }
    };

    // The id arms mirror the sort direction (ASC for oldest, DESC for newest) so that
    // oldest is the exact reverse of newest even when created_at ties (one-second
    // resolution makes ties the normal case, not an edge case). The `newest` arm is
    // proven by sort_newest_is_default_and_oldest_reverses_it. The `oldest` arm cannot
    // be proven the same way: on tied timestamps it coincides with SQLite's incidental
    // rowid scan order, so no black-box test can distinguish "the arm is doing the work"
    // from "the coincidence happens to agree with it". It is kept anyway as a
    // determinism guarantee against a future index or query-plan change that would
    // break that coincidence.
    let rows = sqlx::query_as!(
        DeckDto,
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
        needle,
        needle,
        mode,
        mode,
        module_id,
        sort,
        sort,
        sort,
        sort
    )
    .fetch_all(&st.pool)
    .await?;

    Ok(Json(rows))
}

async fn create(
    State(st): State<AppState>,
    AppJson(body): AppJson<CreateDeck>,
) -> AppResult<(StatusCode, Json<DeckDto>)> {
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
    .fetch_one(&st.pool)
    .await
    .map_err(|e| AppError::from(e).fk_as("module_id", "That module does not exist"))?;

    Ok((StatusCode::CREATED, Json(fetch_one(&st.pool, id).await?)))
}

async fn patch(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    AppJson(body): AppJson<PatchDeck>,
) -> AppResult<Json<DeckDto>> {
    // 404 before any write, so a bad id never touches the row.
    let current = fetch_one(&st.pool, id).await?;

    let name = match body.name {
        Some(n) => {
            let n = n.trim().to_string();
            if n.is_empty() {
                return Err(AppError::validation([("name", "Name must not be empty")]));
            }
            n
        }
        None => current.name,
    };
    let module_id = match body.module_id {
        Some(v) => v,              // present: Some(id) or None (unparent)
        None => current.module_id, // absent: leave alone
    };
    let description = match body.description {
        Some(d) => d.trim().to_string(),
        None => current.description,
    };

    sqlx::query!(
        "UPDATE decks SET module_id = ?, name = ?, description = ? WHERE id = ?",
        module_id,
        name,
        description,
        id
    )
    .execute(&st.pool)
    .await
    .map_err(|e| AppError::from(e).fk_as("module_id", "That module does not exist"))?;

    Ok(Json(fetch_one(&st.pool, id).await?))
}
