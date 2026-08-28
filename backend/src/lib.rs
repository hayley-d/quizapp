pub mod configuration;
pub mod database;
pub mod error;
pub mod extract;
pub mod grading;
pub mod images;
pub mod normalise;
pub mod practice;
pub mod routes;
pub mod state;

use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub fn app(state: state::AppState) -> Router {
    let images = ServeDir::new(state.images_directory.clone());

    Router::new()
        .nest("/api", routes::api_router())
        .nest_service("/images", images)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
