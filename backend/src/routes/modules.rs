use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct ModuleDto {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub deck_count: i64,
}

#[derive(Deserialize)]
pub struct CreateModule {
    pub name: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/modules", get(list).post(create))
}

async fn list(State(st): State<AppState>) -> AppResult<Json<Vec<ModuleDto>>> {
    let rows = sqlx::query_as!(
        ModuleDto,
        r#"SELECT m.id AS "id!: i64",
                  m.name,
                  m.created_at,
                  (SELECT COUNT(*) FROM decks d WHERE d.module_id = m.id)
                      AS "deck_count!: i64"
           FROM modules m
           ORDER BY m.name COLLATE NOCASE"#
    )
    .fetch_all(&st.pool)
    .await?;
    Ok(Json(rows))
}

async fn create(
    State(st): State<AppState>,
    Json(body): Json<CreateModule>,
) -> AppResult<(StatusCode, Json<ModuleDto>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::validation([("name", "Name must not be empty")]));
    }

    let id = sqlx::query_scalar!("INSERT INTO modules (name) VALUES (?) RETURNING id", name)
        .fetch_one(&st.pool)
        .await?;

    let created = sqlx::query_as!(
        ModuleDto,
        r#"SELECT m.id AS "id!: i64", m.name, m.created_at, 0 AS "deck_count!: i64"
           FROM modules m WHERE m.id = ?"#,
        id
    )
    .fetch_one(&st.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(created)))
}
