pub mod cards;
pub mod decks;
pub mod health;
pub mod images;
pub mod modules;

use axum::Router;
use crate::state::AppState;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(modules::router())
        .merge(decks::router())
        .merge(cards::router())
        .merge(images::router())
}
