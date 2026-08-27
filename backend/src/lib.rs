pub mod config;
pub mod db;
pub mod error;   // added in Task 3
pub mod extract;
pub mod images;
pub mod normalise;
pub mod routes;
pub mod state;

use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub fn app(state: state::AppState) -> Router {
    // Read-only, outside `/api`, and deliberately not behind the AppJson
    // extractor: these responses are image bytes, not the error envelope.
    // ServeDir rejects paths that escape its root, which is the only reason
    // it is safe to hand it a directory of client-supplied filenames.
    let images = ServeDir::new(state.images_dir.clone());

    Router::new()
        .nest("/api", routes::api_router())
        .nest_service("/images", images)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
