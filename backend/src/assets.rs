use axum::body::{Body, Bytes};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::{Embed, EmbeddedFile};

use crate::error::AppError;

#[derive(Embed)]
#[folder = "$OUT_DIR/frontend/"]
#[exclude = ".DS_Store"]
struct FrontendAssets;

const HASHED_ASSET_CACHE: &str = "public, max-age=31536000, immutable";
const UNHASHED_ASSET_CACHE: &str = "public, max-age=0, must-revalidate";
// index.html names the content-hashed bundles, so a cached copy of it outliving a
// rebuild is a white screen rather than a stale pixel.
const INDEX_CACHE: &str = "no-cache";

const HASHED_ASSET_PREFIX: &str = "assets/";

pub async fn serve_frontend(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if path.split('/').any(|segment| segment == ".." || segment == ".") {
        return AppError::BadRequest("bad asset path".to_string()).into_response();
    }

    if let Some(file) = FrontendAssets::get(path) {
        let cache = if path.starts_with(HASHED_ASSET_PREFIX) {
            HASHED_ASSET_CACHE
        } else {
            UNHASHED_ASSET_CACHE
        };
        return embedded_response(path, file, cache);
    }

    // A miss under assets/ is a missing build artefact, never a client-side route.
    // Answering it with index.html turns that into "Expected a JavaScript module but
    // the server responded with a MIME type of text/html", which hides the real cause.
    if path.starts_with(HASHED_ASSET_PREFIX) {
        return AppError::NotFound("asset").into_response();
    }

    match FrontendAssets::get("index.html") {
        Some(file) => embedded_response("index.html", file, INDEX_CACHE),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the frontend bundle was not embedded in this binary",
        )
            .into_response(),
    }
}

fn embedded_response(path: &str, file: EmbeddedFile, cache: &'static str) -> Response {
    let body = match file.data {
        std::borrow::Cow::Borrowed(bytes) => Body::from(Bytes::from_static(bytes)),
        std::borrow::Cow::Owned(bytes) => Body::from(bytes),
    };

    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type_for(path)),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache)),
        ],
        body,
    )
        .into_response()
}

fn content_type_for(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("map") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types_carry_a_charset_for_text_formats() {
        assert_eq!(content_type_for("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            content_type_for("assets/index-abc123.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type_for("assets/index-abc123.css"), "text/css; charset=utf-8");
    }

    #[test]
    fn fonts_are_not_served_as_octet_stream() {
        assert_eq!(content_type_for("assets/KaTeX_Main-Regular-abc.woff2"), "font/woff2");
    }

    #[test]
    fn an_unknown_extension_falls_back_to_octet_stream() {
        assert_eq!(content_type_for("assets/thing.bin"), "application/octet-stream");
        assert_eq!(content_type_for("noextension"), "application/octet-stream");
    }

    #[test]
    fn the_bundle_is_actually_embedded() {
        assert!(
            FrontendAssets::get("index.html").is_some(),
            "index.html is missing from the embedded bundle",
        );
    }
}
