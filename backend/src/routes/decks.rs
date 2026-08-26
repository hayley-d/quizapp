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
    /// Numeric module id, or the literal "none" for unparented decks only.
    pub module_id: Option<String>,
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
        .route("/decks/{id}", axum::routing::patch(patch))
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
    // Three literal queries rather than dynamic SQL: query_as! needs a literal
    // string, so every variant stays compile-time checked.
    let rows = match q.module_id.as_deref() {
        None => sqlx::query_as!(
            DeckDto,
            r#"SELECT d.id AS "id!: i64",
                      d.module_id AS "module_id?: i64",
                      m.name      AS "module_name?: String",
                      d.name, d.description, d.created_at,
                      (SELECT COUNT(*) FROM cards c
                        WHERE c.deck_id = d.id AND c.archived = 0) AS "card_count!: i64"
               FROM decks d
               LEFT JOIN modules m ON m.id = d.module_id
               ORDER BY m.name COLLATE NOCASE, d.name COLLATE NOCASE"#
        )
        .fetch_all(&st.pool)
        .await?,

        Some("none") => sqlx::query_as!(
            DeckDto,
            r#"SELECT d.id AS "id!: i64",
                      d.module_id AS "module_id?: i64",
                      m.name      AS "module_name?: String",
                      d.name, d.description, d.created_at,
                      (SELECT COUNT(*) FROM cards c
                        WHERE c.deck_id = d.id AND c.archived = 0) AS "card_count!: i64"
               FROM decks d
               LEFT JOIN modules m ON m.id = d.module_id
               WHERE d.module_id IS NULL
               ORDER BY d.name COLLATE NOCASE"#
        )
        .fetch_all(&st.pool)
        .await?,

        Some(raw) => {
            let mid: i64 = raw.parse().map_err(|_| {
                AppError::validation([("module_id", "module_id must be a number or \"none\"")])
            })?;
            sqlx::query_as!(
                DeckDto,
                r#"SELECT d.id AS "id!: i64",
                          d.module_id AS "module_id?: i64",
                          m.name      AS "module_name?: String",
                          d.name, d.description, d.created_at,
                          (SELECT COUNT(*) FROM cards c
                            WHERE c.deck_id = d.id AND c.archived = 0) AS "card_count!: i64"
                   FROM decks d
                   LEFT JOIN modules m ON m.id = d.module_id
                   WHERE d.module_id = ?
                   ORDER BY d.name COLLATE NOCASE"#,
                mid
            )
            .fetch_all(&st.pool)
            .await?
        }
    };
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
    .await?;

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
    .await?;

    Ok(Json(fetch_one(&st.pool, id).await?))
}
