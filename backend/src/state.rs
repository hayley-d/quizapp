use std::path::PathBuf;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    /// Where uploaded images live. `POST /api/images` writes here and the
    /// `/images` ServeDir reads from here; holding it in one place is what
    /// stops the writer and the reader drifting apart.
    pub images_dir: PathBuf,
}
