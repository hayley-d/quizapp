pub mod cards;
pub mod decks;
pub mod health;
pub mod images;
pub mod modules;
pub mod sessions;
pub mod transfer;

use axum::response::{IntoResponse, Response};
use axum::Router;

use crate::error::AppError;
use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(modules::router())
        .merge(decks::router())
        .merge(cards::router())
        .merge(images::router())
        .merge(sessions::router())
        .merge(transfer::router())
        .fallback(unknown_endpoint)
}

async fn unknown_endpoint() -> Response {
    AppError::NotFound("endpoint").into_response()
}
