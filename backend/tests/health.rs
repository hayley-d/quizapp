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
async fn migration_creates_all_tables() {
    let app = common::spawn_app().await;
    // spawn_app ran migrations; assert every spec table is present.
    let _ = &app; // schema assertions below use a direct pool
}

#[tokio::test]
async fn schema_has_all_tables_and_constraints() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}/test.db?mode=rwc", dir.path().display());
    let pool = quizapp::db::connect(&url).await.unwrap();

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

    // foreign keys enforced
    let bad_fk = sqlx::query("INSERT INTO decks (module_id, name) VALUES (9999, 'x')")
        .execute(&pool)
        .await;
    assert!(bad_fk.is_err(), "foreign keys not enforced");

    // card kind CHECK
    sqlx::query("INSERT INTO modules (name) VALUES ('M')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO decks (module_id, name) VALUES (1, 'D')")
        .execute(&pool).await.unwrap();
    let bad_kind = sqlx::query(
        "INSERT INTO cards (deck_id, kind, prompt_md) VALUES (1, 'essay', 'p')")
        .execute(&pool).await;
    assert!(bad_kind.is_err(), "cards.kind CHECK not enforced");

    // duplicate deck name within the same module
    let dup = sqlx::query("INSERT INTO decks (module_id, name) VALUES (1, 'D')")
        .execute(&pool).await;
    assert!(dup.is_err(), "duplicate deck name allowed within a module");

    // duplicate deck name among module-less decks
    sqlx::query("INSERT INTO decks (module_id, name) VALUES (NULL, 'Loose')")
        .execute(&pool).await.unwrap();
    let dup_null = sqlx::query("INSERT INTO decks (module_id, name) VALUES (NULL, 'Loose')")
        .execute(&pool).await;
    assert!(dup_null.is_err(), "duplicate unparented deck name allowed");
}
