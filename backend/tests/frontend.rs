mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn the_root_serves_the_embedded_index() {
    let app = common::spawn_app().await;

    let response = app.get_response("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8",
    );

    let (_, bytes) = app.get_raw("/").await;
    let html = String::from_utf8(bytes).unwrap();
    assert!(html.contains("<div id=\"root\">"), "not the app shell: {html:.200}");
}

#[tokio::test]
async fn a_client_side_route_serves_the_index_so_a_hard_refresh_works() {
    let app = common::spawn_app().await;

    for route in ["/decks", "/decks/3", "/cards/7/edit", "/session/2", "/mock/2"] {
        let response = app.get_response(route).await;
        assert_eq!(response.status(), StatusCode::OK, "route {route}");
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/html; charset=utf-8",
            "route {route}",
        );
    }
}

#[tokio::test]
async fn the_index_is_not_cached_because_it_names_the_hashed_bundles() {
    let app = common::spawn_app().await;

    let cache_control = app.header_value("/", "cache-control").await;
    assert_eq!(cache_control.as_deref(), Some("no-cache"));
}

#[tokio::test]
async fn hashed_assets_are_served_with_their_type_and_an_immutable_cache() {
    let app = common::spawn_app().await;

    let (_, bytes) = app.get_raw("/").await;
    let html = String::from_utf8(bytes).unwrap();
    let script_path = html
        .split("src=\"")
        .find_map(|fragment| {
            let candidate = fragment.split('"').next()?;
            candidate.starts_with("/assets/").then_some(candidate)
        })
        .expect("index.html references a hashed script under /assets/");

    let response = app.get_response(script_path).await;
    assert_eq!(response.status(), StatusCode::OK, "asset {script_path}");
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/javascript; charset=utf-8",
    );
    assert_eq!(
        response.headers().get("cache-control").unwrap(),
        "public, max-age=31536000, immutable",
    );
}

// The regression test for the ordering bug this whole arrangement risks: the SPA
// fallback must not swallow API paths, or a typo'd fetch gets HTML with a 200 and the
// client tries to parse it as JSON.
#[tokio::test]
async fn the_frontend_fallback_does_not_swallow_unknown_api_paths() {
    let app = common::spawn_app().await;

    let response = app.get_response("/api/nope").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.headers().get("content-type").unwrap(), "application/json");
}

#[tokio::test]
async fn a_missing_asset_is_a_404_rather_than_the_index() {
    let app = common::spawn_app().await;

    let response = app.get_response("/assets/does-not-exist-abc123.js").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_ne!(
        response.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8",
        "a missing bundle must not masquerade as the app shell",
    );
}

#[tokio::test]
async fn a_missing_image_is_a_404_rather_than_the_index() {
    let app = common::spawn_app().await;

    let response = app.get_response("/images/nothing-here.png").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_ne!(
        response.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8",
        "a broken thumbnail must not silently become a page of HTML",
    );
}

#[tokio::test]
async fn a_traversal_attempt_is_refused() {
    let app = common::spawn_app().await;

    let response = app.get_response("/assets/../../../etc/passwd").await;
    assert_ne!(response.status(), StatusCode::OK);
}
