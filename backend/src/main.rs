use tracing_subscriber::EnvFilter;

use quizapp::configuration::Configuration;
use quizapp::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")))
        .init();

    let configuration = Configuration::from_environment();
    std::fs::create_dir_all(&configuration.data_directory)?;
    let images_directory = configuration.images_directory();
    std::fs::create_dir_all(&images_directory)?;
    let pool = quizapp::database::connect(&configuration.database_url).await?;
    let app = quizapp::app(AppState { pool, images_directory });

    let listener = tokio::net::TcpListener::bind(&configuration.bind_address).await?;
    tracing::info!("listening on http://{}", configuration.bind_address);
    axum::serve(listener, app).await?;
    Ok(())
}
