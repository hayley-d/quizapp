mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn health_returns_ok() {
    let app = common::spawn_app().await;
    let (status, body) = app.get("/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn schema_has_all_tables_and_constraints() {
    let temporary_directory = tempfile::tempdir().unwrap();
    let database_url = format!(
        "sqlite://{}/test.db?mode=rwc",
        temporary_directory.path().display(),
    );
    let pool = quizapp::database::connect(&database_url).await.unwrap();

    for table in ["modules", "decks", "cards", "choices", "accepted",
                  "sessions", "reviews", "schedule"] {
        let found: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
        )
        .bind(table)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(found.as_deref(), Some(table), "missing table {table}");
    }

    let foreign_key_violation =
        sqlx::query("INSERT INTO decks (module_id, name) VALUES (9999, 'x')")
            .execute(&pool)
            .await;
    assert!(foreign_key_violation.is_err(), "foreign keys not enforced");

    sqlx::query("INSERT INTO modules (name) VALUES ('M')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO decks (module_id, name) VALUES (1, 'D')")
        .execute(&pool).await.unwrap();
    let rejected_kind = sqlx::query(
        "INSERT INTO cards (deck_id, kind, prompt_md) VALUES (1, 'essay', 'p')")
        .execute(&pool).await;
    assert!(rejected_kind.is_err(), "cards.kind CHECK not enforced");

    let duplicate_within_module = sqlx::query(
        "INSERT INTO decks (module_id, name) VALUES (1, 'D')")
        .execute(&pool).await;
    assert!(
        duplicate_within_module.is_err(),
        "duplicate deck name allowed within a module",
    );

    sqlx::query("INSERT INTO decks (module_id, name) VALUES (NULL, 'Loose')")
        .execute(&pool).await.unwrap();
    let duplicate_unparented = sqlx::query(
        "INSERT INTO decks (module_id, name) VALUES (NULL, 'Loose')")
        .execute(&pool).await;
    assert!(duplicate_unparented.is_err(), "duplicate unparented deck name allowed");
}

#[tokio::test]
async fn unknown_api_path_returns_the_error_envelope() {
    let app = common::spawn_app().await;

    let (status, body) = app.get("/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
    assert_eq!(body["message"], "endpoint not found");
    assert_eq!(body["fields"], serde_json::json!([]));

    let content_type = app.header_value("/api/nope", "content-type").await;
    assert_eq!(content_type.as_deref(), Some("application/json"));
}

#[tokio::test]
async fn unknown_nested_api_path_returns_the_error_envelope() {
    let app = common::spawn_app().await;

    let (status, body) = app.get("/api/decks/1/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}
