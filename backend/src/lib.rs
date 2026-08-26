pub mod config;
pub mod db;
pub mod error;   // added in Task 3
pub mod extract;
pub mod routes;
pub mod state;

use axum::Router;
use tower_http::trace::TraceLayer;

pub fn app(state: state::AppState) -> Router {
    Router::new()
        .nest("/api", routes::api_router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
