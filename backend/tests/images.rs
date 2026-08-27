mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn serves_a_file_from_the_images_directory() {
    let app = common::spawn_app().await;
    std::fs::write(app.images_dir.join("diagram.png"), b"pretend-image-bytes").unwrap();

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
    // test.db is the sibling of images/ inside the same tempdir, so if the
    // static route resolved `..` this would hand out the database file.
    let (status, body) = app.get_raw("/images/../test.db").await;
    assert_ne!(status, StatusCode::OK, "the database must not be reachable over HTTP");
    assert!(
        !body.starts_with(b"SQLite format 3"),
        "served the database file: the static route escaped its root",
    );
}
