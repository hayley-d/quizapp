use serde::Serialize;

use crate::error::AppResult;
use crate::practice::{
    fold_candidate_rows, weighted_miss_rate, CandidateRow, RECENT_REVIEW_LIMIT,
};

#[derive(Serialize)]
pub struct DeckStatsSummary {
    pub card_count: i64,
    pub unseen_count: i64,
    pub mock_accuracy: Option<f64>,
    pub mock_review_count: i64,
    pub practice_accuracy: Option<f64>,
    pub practice_review_count: i64,
    pub sm2_accuracy: Option<f64>,
    pub sm2_review_count: i64,
    pub due_count: i64,
    pub next_due_at: Option<String>,
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
        cards: load_card_stats(pool, deck_id).await?,
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
        ),
        due AS (
            SELECT cards.id AS card_id, schedule.due_at AS due_at
            FROM cards
            LEFT JOIN schedule ON schedule.card_id = cards.id
            WHERE cards.deck_id = ? AND cards.archived = 0
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
          (SELECT COUNT(*) FROM deck_reviews WHERE mode = 'sm2')
              AS "sm2_review_count!: i64",
          (SELECT COALESCE(SUM(correct), 0) FROM deck_reviews WHERE mode = 'sm2')
              AS "sm2_correct_count!: i64",
          (SELECT COUNT(*) FROM due
            WHERE due_at IS NULL
               OR due_at <= strftime('%Y-%m-%dT%H:%M:%SZ','now'))
              AS "due_count!: i64",
          (SELECT MIN(due_at) FROM due)
              AS "next_due_at?: String",
          (SELECT MAX(answered_at) FROM deck_reviews)
              AS "last_answered_at?: String"
        "#,
        deck_id,
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
        sm2_accuracy: accuracy_of(row.sm2_correct_count, row.sm2_review_count),
        sm2_review_count: row.sm2_review_count,
        due_count: row.due_count,
        next_due_at: row.next_due_at,
        last_answered_at: row.last_answered_at,
    })
}

async fn load_card_stats(pool: &sqlx::SqlitePool, deck_id: i64) -> AppResult<Vec<CardStats>> {
    let rows = sqlx::query_as!(
        CandidateRow,
        r#"
        WITH pool AS (
            SELECT id AS card_id FROM cards WHERE deck_id = ? AND archived = 0
        ),
        recent AS (
            SELECT reviews.card_id AS card_id,
                   reviews.correct AS correct,
                   ROW_NUMBER() OVER (
                       PARTITION BY reviews.card_id
                       ORDER BY reviews.answered_at DESC, reviews.id DESC
                   ) AS recency_rank,
                   CAST(strftime('%s','now') AS INTEGER)
                     - CAST(strftime('%s', reviews.answered_at) AS INTEGER) AS age_seconds
            FROM reviews
            JOIN pool ON pool.card_id = reviews.card_id
        ),
        counts AS (
            SELECT reviews.card_id AS card_id, COUNT(*) AS review_count
            FROM reviews
            JOIN pool ON pool.card_id = reviews.card_id
            GROUP BY reviews.card_id
        )
        SELECT pool.card_id                     AS "card_id!: i64",
               COALESCE(counts.review_count, 0) AS "review_count!: i64",
               recent.correct                   AS "correct?: bool",
               recent.recency_rank              AS "recency_rank?: i64",
               recent.age_seconds               AS "age_seconds?: i64"
        FROM pool
        LEFT JOIN counts ON counts.card_id = pool.card_id
        LEFT JOIN recent ON recent.card_id = pool.card_id AND recent.recency_rank <= ?
        ORDER BY pool.card_id, recent.recency_rank
        "#,
        deck_id,
        RECENT_REVIEW_LIMIT,
    )
    .fetch_all(pool)
    .await?;

    Ok(fold_candidate_rows(rows)
        .into_iter()
        .filter(|candidate| candidate.review_count > 0)
        .map(|candidate| CardStats {
            card_id: candidate.card_id,
            attempt_count: candidate.review_count,
            miss_rate: weighted_miss_rate(&candidate.recent_review_outcomes),
        })
        .collect())
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
