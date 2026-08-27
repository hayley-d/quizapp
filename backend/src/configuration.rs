use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Configuration {
    pub bind_address: String,
    pub database_url: String,
    pub data_directory: String,
}

impl Configuration {
    pub fn from_environment() -> Self {
        Self {
            bind_address: std::env::var("QUIZAPP_BIND")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/quizapp.db?mode=rwc".to_string()),
            data_directory: std::env::var("QUIZAPP_DATA_DIR")
                .unwrap_or_else(|_| "data".to_string()),
        }
    }

    pub fn images_directory(&self) -> PathBuf {
        Path::new(&self.data_directory).join("images")
    }
}
