use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::extract::AppJson;
use crate::state::AppState;

#[derive(Serialize)]
pub struct ModuleResponse {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub deck_count: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateModule {
    pub name: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/modules", get(list).post(create))
        .route("/modules/{id}", axum::routing::delete(delete_module))
}

async fn fetch_one(pool: &sqlx::SqlitePool, id: i64) -> AppResult<ModuleResponse> {
    sqlx::query_as!(
        ModuleResponse,
        r#"SELECT m.id AS "id!: i64",
                  m.name,
                  m.created_at,
                  (SELECT COUNT(*) FROM decks d WHERE d.module_id = m.id)
                      AS "deck_count!: i64"
           FROM modules m WHERE m.id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("module"))
}

async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<ModuleResponse>>> {
    let rows = sqlx::query_as!(
        ModuleResponse,
        r#"SELECT m.id AS "id!: i64",
                  m.name,
                  m.created_at,
                  (SELECT COUNT(*) FROM decks d WHERE d.module_id = m.id)
                      AS "deck_count!: i64"
           FROM modules m
           ORDER BY m.name COLLATE NOCASE"#
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn create(
    State(state): State<AppState>,
    AppJson(body): AppJson<CreateModule>,
) -> AppResult<(StatusCode, Json<ModuleResponse>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::validation([("name", "Name must not be empty")]));
    }

    let id = sqlx::query_scalar!("INSERT INTO modules (name) VALUES (?) RETURNING id", name)
        .fetch_one(&state.pool)
        .await?;

    Ok((StatusCode::CREATED, Json(fetch_one(&state.pool, id).await?)))
}

async fn delete_module(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    fetch_one(&state.pool, id).await?;

    sqlx::query!("DELETE FROM modules WHERE id = ?", id)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
