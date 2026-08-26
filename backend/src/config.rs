#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    #[allow(dead_code)]
    pub data_dir: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            bind_addr: std::env::var("QUIZAPP_BIND")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/quizapp.db?mode=rwc".to_string()),
            data_dir: std::env::var("QUIZAPP_DATA_DIR")
                .unwrap_or_else(|_| "data".to_string()),
        }
    }
}
