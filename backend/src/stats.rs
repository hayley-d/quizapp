use serde::Serialize;

use crate::error::AppResult;

#[derive(Serialize)]
pub struct DeckStatsSummary {
    pub card_count: i64,
    pub unseen_count: i64,
    pub mock_accuracy: Option<f64>,
    pub mock_review_count: i64,
    pub practice_accuracy: Option<f64>,
    pub practice_review_count: i64,
    pub last_answered_at: Option<String>,
}

#[derive(Serialize)]
pub struct CardStats {
    pub card_id: i64,
    pub attempt_count: i64,
    pub miss_rate: f64,
}

#[derive(Serialize)]
pub struct DeckStatsResponse {
    pub summary: DeckStatsSummary,
    pub cards: Vec<CardStats>,
}

pub fn accuracy_of(correct_count: i64, review_count: i64) -> Option<f64> {
    if review_count == 0 {
        None
    } else {
        Some(correct_count as f64 / review_count as f64)
    }
}

pub async fn deck_stats(pool: &sqlx::SqlitePool, deck_id: i64) -> AppResult<DeckStatsResponse> {
    Ok(DeckStatsResponse {
        summary: load_summary(pool, deck_id).await?,
        cards: Vec::new(),
    })
}

async fn load_summary(pool: &sqlx::SqlitePool, deck_id: i64) -> AppResult<DeckStatsSummary> {
    let row = sqlx::query!(
        r#"
        WITH pool AS (
            SELECT id AS card_id FROM cards WHERE deck_id = ? AND archived = 0
        ),
        deck_reviews AS (
            SELECT reviews.card_id     AS card_id,
                   reviews.correct     AS correct,
                   reviews.answered_at AS answered_at,
                   sessions.mode       AS mode
            FROM reviews
            JOIN pool     ON pool.card_id = reviews.card_id
            JOIN sessions ON sessions.id = reviews.session_id
        )
        SELECT
          (SELECT COUNT(*) FROM pool)
              AS "card_count!: i64",
          (SELECT COUNT(*) FROM pool
            WHERE card_id NOT IN (SELECT card_id FROM deck_reviews))
              AS "unseen_count!: i64",
          (SELECT COUNT(*) FROM deck_reviews WHERE mode = 'mock')
              AS "mock_review_count!: i64",
          (SELECT COALESCE(SUM(correct), 0) FROM deck_reviews WHERE mode = 'mock')
              AS "mock_correct_count!: i64",
          (SELECT COUNT(*) FROM deck_reviews WHERE mode = 'practice')
              AS "practice_review_count!: i64",
          (SELECT COALESCE(SUM(correct), 0) FROM deck_reviews WHERE mode = 'practice')
              AS "practice_correct_count!: i64",
          (SELECT MAX(answered_at) FROM deck_reviews)
              AS "last_answered_at?: String"
        "#,
        deck_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(DeckStatsSummary {
        card_count: row.card_count,
        unseen_count: row.unseen_count,
        mock_accuracy: accuracy_of(row.mock_correct_count, row.mock_review_count),
        mock_review_count: row.mock_review_count,
        practice_accuracy: accuracy_of(row.practice_correct_count, row.practice_review_count),
        practice_review_count: row.practice_review_count,
        last_answered_at: row.last_answered_at,
    })
}

#[cfg(test)]
mod tests {
    use super::accuracy_of;

    #[test]
    fn accuracy_of_no_reviews_is_none_not_a_nan() {
        let accuracy = accuracy_of(0, 0);
        assert_eq!(accuracy, None);
        assert!(!accuracy.is_some_and(f64::is_nan));
    }

    #[test]
    fn accuracy_of_counts_is_the_ratio() {
        assert_eq!(accuracy_of(3, 4), Some(0.75));
        assert_eq!(accuracy_of(0, 4), Some(0.0));
        assert_eq!(accuracy_of(4, 4), Some(1.0));
    }
}
