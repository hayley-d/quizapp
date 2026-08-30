pub mod assets;
pub mod configuration;
pub mod database;
pub mod error;
pub mod extract;
pub mod grading;
pub mod images;
pub mod mock;
pub mod normalise;
pub mod practice;
pub mod routes;
pub mod scheduler;
pub mod state;
pub mod stats;

use std::convert::Infallible;

use axum::body::Body;
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum::Router;
use tower::ServiceBuilder;
use tower_http::compression::predicate::{DefaultPredicate, NotForContentType};
use tower_http::compression::{CompressionLayer, CompressionLevel, Predicate};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::error::AppError;

pub fn app(state: state::AppState) -> Router {
    let missing_image = tower::service_fn(|_request: Request<Body>| async {
        Ok::<Response, Infallible>(AppError::NotFound("image").into_response())
    });
    let images = ServeDir::new(state.images_directory.clone()).not_found_service(missing_image);

    // The fallback and both nests must be registered before .layer(), because a layer
    // applies only to what was registered before it. Moving .layer() above .fallback()
    // silently leaves every frontend response untraced and uncompressed.
    Router::new()
        .nest("/api", routes::api_router())
        .nest_service("/images", images)
        .fallback(assets::serve_frontend)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(
                    CompressionLayer::new()
                        .quality(CompressionLevel::Fastest)
                        .compress_when(compressible()),
                ),
        )
        .with_state(state)
}

// woff2 is already a compressed container, so recompressing it spends CPU for almost
// no bytes. DefaultPredicate already excludes images and server-sent events.
fn compressible() -> impl Predicate {
    DefaultPredicate::new()
        .and(NotForContentType::const_new("font/woff2"))
        .and(NotForContentType::const_new("font/woff"))
}
