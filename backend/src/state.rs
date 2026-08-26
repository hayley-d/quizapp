use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    pub pool: SqlitePool,
}
