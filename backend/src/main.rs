use tracing_subscriber::EnvFilter;

use quizapp::config::Config;
use quizapp::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")))
        .init();

    let config = Config::from_env();
    std::fs::create_dir_all(&config.data_dir)?;
    let images_dir = config.images_dir();
    std::fs::create_dir_all(&images_dir)?;
    let pool = quizapp::db::connect(&config.database_url).await?;
    let app = quizapp::app(AppState { pool, images_dir });

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("listening on http://{}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
