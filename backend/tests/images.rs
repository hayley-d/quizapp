mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn serves_a_file_from_the_images_directory() {
    let app = common::spawn_app().await;
    std::fs::write(app.images_directory.join("diagram.png"), b"pretend-image-bytes").unwrap();

    let (status, body) = app.get_raw("/images/diagram.png").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"pretend-image-bytes");
}

#[tokio::test]
async fn an_absent_image_is_404_not_500() {
    let app = common::spawn_app().await;
    let (status, _) = app.get_raw("/images/nothing-here.png").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_traversal_path_cannot_escape_the_images_directory() {
    let app = common::spawn_app().await;
    let (status, body) = app.get_raw("/images/../test.db").await;
    assert_ne!(status, StatusCode::OK, "the database must not be reachable over HTTP");
    assert!(
        !body.starts_with(b"SQLite format 3"),
        "served the database file: the static route escaped its root",
    );
}

const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];

fn image(signature: &[u8], filler: usize) -> Vec<u8> {
    let mut bytes = signature.to_vec();
    bytes.resize(signature.len() + filler, 0xAB);
    bytes
}

#[tokio::test]
async fn uploads_a_png_and_serves_it_back() {
    let app = common::spawn_app().await;
    let bytes = image(PNG_SIGNATURE, 64);

    let (status, body) = app.post_file("/api/images", "file", "diagram.png", &bytes).await;
    assert_eq!(status, StatusCode::CREATED);

    let path = body["path"].as_str().expect("response carries a path");
    let name = path.strip_prefix("images/").expect("path is under images/");
    assert!(name.ends_with(".png"), "got {path}");
    assert_eq!(name.len(), "0123456789abcdef.png".len(), "16 hex characters plus .png");
    assert!(app.images_directory.join(name).exists(), "the file was not written");

    let (status, served) = app.get_raw(&format!("/{path}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(served, bytes, "the bytes served back are the bytes uploaded");
}

#[tokio::test]
async fn the_extension_comes_from_the_bytes_not_the_filename() {
    let app = common::spawn_app().await;

    let (status, body) = app
        .post_file("/api/images", "file", "diagram.png", &image(JPEG_SIGNATURE, 32))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        body["path"].as_str().unwrap().ends_with(".jpg"),
        "a JPEG named .png must be stored as .jpg, got {}", body["path"],
    );
}

#[tokio::test]
async fn identical_bytes_reuse_one_file() {
    let app = common::spawn_app().await;
    let bytes = image(PNG_SIGNATURE, 100);

    let (_, first) = app.post_file("/api/images", "file", "a.png", &bytes).await;
    let (_, second) = app.post_file("/api/images", "file", "b.png", &bytes).await;

    assert_eq!(first["path"], second["path"], "content-addressed names must match");
    assert_eq!(app.image_count().await, 1, "the same bytes must not be stored twice");
}

#[tokio::test]
async fn different_images_get_different_paths() {
    let app = common::spawn_app().await;

    let (_, a) = app.post_file("/api/images", "file", "a.png", &image(PNG_SIGNATURE, 10)).await;
    let (_, b) = app.post_file("/api/images", "file", "b.png", &image(PNG_SIGNATURE, 11)).await;

    assert_ne!(a["path"], b["path"]);
    assert_eq!(app.image_count().await, 2);
}

#[tokio::test]
async fn rejects_a_file_that_is_not_an_image() {
    let app = common::spawn_app().await;

    let (status, body) = app
        .post_file("/api/images", "file", "notes.png", b"just some notes about k-means")
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"][0]["field"], "file", "the editor renders this beside the picker");
    assert_eq!(app.image_count().await, 0, "a rejected upload must write nothing");
}

#[tokio::test]
async fn rejects_an_oversize_image_through_the_envelope() {
    let app = common::spawn_app().await;
    let bytes = image(PNG_SIGNATURE, 5 * 1024 * 1024 + 1 - PNG_SIGNATURE.len());

    let (status, body) = app.post_file("/api/images", "file", "huge.png", &bytes).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "not axum's raw 413");
    assert_eq!(body["error"], "validation");
    assert_eq!(body["fields"][0]["field"], "file");
    assert_eq!(app.image_count().await, 0);
}

#[tokio::test]
async fn rejects_a_multipart_body_with_no_file_part() {
    let app = common::spawn_app().await;

    let (status, body) = app
        .post_file("/api/images", "notthefield", "diagram.png", &image(PNG_SIGNATURE, 16))
        .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["fields"][0]["field"], "file");
    assert_eq!(app.image_count().await, 0);
}

#[tokio::test]
async fn an_upload_touches_no_card() {
    let app = common::spawn_app().await;
    let (_, deck) = app.post("/api/decks", serde_json::json!({ "name": "Clustering" })).await;
    let deck_id = deck["id"].as_i64().unwrap();
    let (_, card) = app
        .post("/api/cards", serde_json::json!({
            "deck_id": deck_id, "kind": "flashcard",
            "prompt_md": "Define support.", "answer_md": "A fraction of transactions."
        }))
        .await;
    let before = card["updated_at"].as_str().unwrap().to_string();

    let (status, _) = app.post_file("/api/images", "file", "d.png", &image(PNG_SIGNATURE, 8)).await;
    assert_eq!(status, StatusCode::CREATED);

    let (_, after) = app.get(&format!("/api/cards/{}", card["id"].as_i64().unwrap())).await;
    assert_eq!(after["updated_at"], before, "an upload must not touch any card");
    assert!(after["image_path"].is_null(), "an upload must not attach itself to a card");
}
