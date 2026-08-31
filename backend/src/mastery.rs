use serde::Serialize;

use crate::error::AppResult;
use crate::practice::{weighted_miss_rate, ReviewOutcome, RECENT_REVIEW_LIMIT};

pub const SHAKY_MISS_RATE_CEILING: f64 = 0.5;
pub const SOLID_MISS_RATE_CEILING: f64 = 0.25;
pub const MASTERED_MISS_RATE_CEILING: f64 = 0.1;
pub const SOLID_CONSECUTIVE_CORRECT: usize = 2;
pub const MASTERED_CONSECUTIVE_CORRECT: usize = 3;
pub const MASTERED_STREAK_SPAN_SECONDS: i64 = 43_200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MasteryLevel {
    Unseen,
    Shaky,
    Learning,
    Solid,
    Mastered,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct MasteryCounts {
    pub unseen: i64,
    pub shaky: i64,
    pub learning: i64,
    pub solid: i64,
    pub mastered: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MasteryReview {
    pub outcome: ReviewOutcome,
    pub answered_at_seconds: i64,
}

impl MasteryLevel {
    pub fn rank(self) -> u8 {
        match self {
            MasteryLevel::Unseen => 0,
            MasteryLevel::Shaky => 1,
            MasteryLevel::Learning => 2,
            MasteryLevel::Solid => 3,
            MasteryLevel::Mastered => 4,
        }
    }
}

impl MasteryCounts {
    pub fn add(&mut self, level: MasteryLevel) {
        match level {
            MasteryLevel::Unseen => self.unseen += 1,
            MasteryLevel::Shaky => self.shaky += 1,
            MasteryLevel::Learning => self.learning += 1,
            MasteryLevel::Solid => self.solid += 1,
            MasteryLevel::Mastered => self.mastered += 1,
        }
    }

    pub fn total(&self) -> i64 {
        self.unseen + self.shaky + self.learning + self.solid + self.mastered
    }
}

pub fn consecutive_correct_count(reviews_newest_first: &[MasteryReview]) -> usize {
    reviews_newest_first
        .iter()
        .take_while(|review| review.outcome == ReviewOutcome::Correct)
        .count()
}

pub fn correct_streak_span_seconds(reviews_newest_first: &[MasteryReview]) -> i64 {
    let streak: Vec<&MasteryReview> = reviews_newest_first
        .iter()
        .take_while(|review| review.outcome == ReviewOutcome::Correct)
        .collect();
    match (streak.first(), streak.last()) {
        (Some(newest), Some(oldest)) => newest.answered_at_seconds - oldest.answered_at_seconds,
        _ => 0,
    }
}

pub fn level_for(reviews_newest_first: &[MasteryReview]) -> MasteryLevel {
    if reviews_newest_first.is_empty() {
        return MasteryLevel::Unseen;
    }

    let outcomes: Vec<ReviewOutcome> = reviews_newest_first
        .iter()
        .map(|review| review.outcome)
        .collect();
    let miss_rate = weighted_miss_rate(&outcomes);
    if miss_rate > SHAKY_MISS_RATE_CEILING {
        return MasteryLevel::Shaky;
    }

    let consecutive_correct = consecutive_correct_count(reviews_newest_first);
    if consecutive_correct >= MASTERED_CONSECUTIVE_CORRECT
        && miss_rate <= MASTERED_MISS_RATE_CEILING
        && correct_streak_span_seconds(reviews_newest_first) >= MASTERED_STREAK_SPAN_SECONDS
    {
        return MasteryLevel::Mastered;
    }

    if consecutive_correct >= SOLID_CONSECUTIVE_CORRECT && miss_rate <= SOLID_MISS_RATE_CEILING {
        return MasteryLevel::Solid;
    }

    MasteryLevel::Learning
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MovementDirection {
    Up,
    Down,
    Unchanged,
}

pub fn movement_direction(before: MasteryLevel, after: MasteryLevel) -> MovementDirection {
    if before == MasteryLevel::Unseen && after == MasteryLevel::Shaky {
        return MovementDirection::Unchanged;
    }
    match after.rank().cmp(&before.rank()) {
        std::cmp::Ordering::Greater => MovementDirection::Up,
        std::cmp::Ordering::Less => MovementDirection::Down,
        std::cmp::Ordering::Equal => MovementDirection::Unchanged,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionMasteryMovement {
    pub card_id: i64,
    pub prompt_md: String,
    pub level_before: MasteryLevel,
    pub level_after: MasteryLevel,
    pub direction: MovementDirection,
}

struct SessionReviewRow {
    phase: String,
    card_id: i64,
    correct: bool,
    answered_at_seconds: i64,
}

pub async fn session_movements(
    pool: &sqlx::SqlitePool,
    session_id: i64,
    started_at: &str,
) -> AppResult<Vec<SessionMasteryMovement>> {
    let rows = sqlx::query_as!(
        SessionReviewRow,
        r#"
        WITH session_cards AS (
            SELECT DISTINCT reviews.card_id AS card_id
            FROM reviews
            JOIN cards ON cards.id = reviews.card_id
            WHERE reviews.session_id = ? AND cards.archived = 0
        ),
        ranked AS (
            SELECT 'before' AS phase,
                   reviews.card_id AS card_id,
                   reviews.correct AS correct,
                   CAST(strftime('%s', reviews.answered_at) AS INTEGER) AS answered_at_seconds,
                   ROW_NUMBER() OVER (
                       PARTITION BY reviews.card_id
                       ORDER BY reviews.answered_at DESC, reviews.id DESC
                   ) AS recency_rank
            FROM reviews
            JOIN session_cards ON session_cards.card_id = reviews.card_id
            WHERE reviews.answered_at < ?
            UNION ALL
            SELECT 'after' AS phase,
                   reviews.card_id AS card_id,
                   reviews.correct AS correct,
                   CAST(strftime('%s', reviews.answered_at) AS INTEGER) AS answered_at_seconds,
                   ROW_NUMBER() OVER (
                       PARTITION BY reviews.card_id
                       ORDER BY reviews.answered_at DESC, reviews.id DESC
                   ) AS recency_rank
            FROM reviews
            JOIN session_cards ON session_cards.card_id = reviews.card_id
            WHERE reviews.answered_at < ? OR reviews.session_id = ?
        )
        SELECT phase               AS "phase!: String",
               card_id             AS "card_id!: i64",
               correct             AS "correct!: bool",
               answered_at_seconds AS "answered_at_seconds!: i64"
        FROM ranked
        WHERE recency_rank <= ?
        ORDER BY card_id, phase, recency_rank
        "#,
        session_id,
        started_at,
        started_at,
        session_id,
        RECENT_REVIEW_LIMIT,
    )
    .fetch_all(pool)
    .await?;

    let prompts = sqlx::query!(
        r#"
        SELECT cards.id AS "card_id!: i64", cards.prompt_md AS "prompt_md!: String"
        FROM cards
        WHERE cards.archived = 0
          AND cards.id IN (SELECT DISTINCT card_id FROM reviews WHERE session_id = ?)
        "#,
        session_id,
    )
    .fetch_all(pool)
    .await?;

    let mut movements: Vec<SessionMasteryMovement> = Vec::new();
    for prompt in prompts {
        let level_before = level_from_phase(&rows, prompt.card_id, "before");
        let level_after = level_from_phase(&rows, prompt.card_id, "after");
        movements.push(SessionMasteryMovement {
            card_id: prompt.card_id,
            prompt_md: prompt.prompt_md,
            level_before,
            level_after,
            direction: movement_direction(level_before, level_after),
        });
    }

    movements.sort_by_key(|movement| {
        let direction_order = match movement.direction {
            MovementDirection::Up => 0,
            MovementDirection::Down => 1,
            MovementDirection::Unchanged => 2,
        };
        (direction_order, std::cmp::Reverse(movement.level_after.rank()), movement.card_id)
    });
    Ok(movements)
}

fn level_from_phase(rows: &[SessionReviewRow], card_id: i64, phase: &str) -> MasteryLevel {
    let reviews: Vec<MasteryReview> = rows
        .iter()
        .filter(|row| row.card_id == card_id && row.phase == phase)
        .map(|row| MasteryReview {
            outcome: if row.correct { ReviewOutcome::Correct } else { ReviewOutcome::Incorrect },
            answered_at_seconds: row.answered_at_seconds,
        })
        .collect();
    level_for(&reviews)
}

pub fn count_moved_up(movements: &[SessionMasteryMovement]) -> i64 {
    movements
        .iter()
        .filter(|movement| movement.direction == MovementDirection::Up)
        .count() as i64
}

pub fn count_moved_down(movements: &[SessionMasteryMovement]) -> i64 {
    movements
        .iter()
        .filter(|movement| movement.direction == MovementDirection::Down)
        .count() as i64
}

#[cfg(test)]
mod tests {
    use super::{
        consecutive_correct_count, correct_streak_span_seconds, level_for, movement_direction,
        MasteryCounts, MasteryLevel, MasteryReview, MovementDirection, ReviewOutcome,
        MASTERED_STREAK_SPAN_SECONDS,
    };

    const ONE_HOUR: i64 = 3_600;

    fn reviews(pattern: &[(bool, i64)]) -> Vec<MasteryReview> {
        pattern
            .iter()
            .map(|(was_correct, answered_at_seconds)| MasteryReview {
                outcome: if *was_correct {
                    ReviewOutcome::Correct
                } else {
                    ReviewOutcome::Incorrect
                },
                answered_at_seconds: *answered_at_seconds,
            })
            .collect()
    }

    fn correct_at(seconds: &[i64]) -> Vec<MasteryReview> {
        reviews(&seconds.iter().map(|second| (true, *second)).collect::<Vec<_>>())
    }

    #[test]
    fn a_card_with_no_reviews_is_unseen() {
        assert_eq!(level_for(&[]), MasteryLevel::Unseen);
    }

    #[test]
    fn a_single_correct_answer_is_learning_not_solid() {
        assert_eq!(
            level_for(&correct_at(&[0])),
            MasteryLevel::Learning,
            "one right answer is not yet evidence of anything",
        );
    }

    #[test]
    fn a_single_wrong_answer_is_shaky() {
        assert_eq!(level_for(&reviews(&[(false, 0)])), MasteryLevel::Shaky);
    }

    #[test]
    fn two_consecutive_correct_answers_reach_solid() {
        assert_eq!(level_for(&correct_at(&[ONE_HOUR, 0])), MasteryLevel::Solid);
    }

    #[test]
    fn a_long_streak_inside_one_sitting_does_not_reach_mastered() {
        assert_eq!(
            level_for(&correct_at(&[2 * ONE_HOUR, ONE_HOUR, 0])),
            MasteryLevel::Solid,
            "mastery needs the streak to be spaced out, not crammed into one sitting",
        );
    }

    #[test]
    fn a_streak_spanning_the_required_gap_reaches_mastered() {
        assert_eq!(
            level_for(&correct_at(&[MASTERED_STREAK_SPAN_SECONDS, ONE_HOUR, 0])),
            MasteryLevel::Mastered,
        );
    }

    #[test]
    fn the_spacing_requirement_is_inclusive_at_its_boundary() {
        let just_short = correct_at(&[MASTERED_STREAK_SPAN_SECONDS - 1, ONE_HOUR, 0]);
        let exactly_enough = correct_at(&[MASTERED_STREAK_SPAN_SECONDS, ONE_HOUR, 0]);
        assert_eq!(level_for(&just_short), MasteryLevel::Solid);
        assert_eq!(level_for(&exactly_enough), MasteryLevel::Mastered);
    }

    #[test]
    fn one_recent_miss_drops_a_perfect_card_off_solid() {
        let level = level_for(&reviews(&[
            (false, 3 * ONE_HOUR),
            (true, 2 * ONE_HOUR),
            (true, ONE_HOUR),
            (true, 0),
        ]));
        assert_eq!(
            level,
            MasteryLevel::Learning,
            "the streak is broken, so the card is back to learning even though most answers were right",
        );
    }

    #[test]
    fn an_old_miss_behind_a_long_correct_streak_still_permits_mastered() {
        let mut pattern: Vec<(bool, i64)> = (0..9)
            .map(|position| (true, -position * MASTERED_STREAK_SPAN_SECONDS))
            .collect();
        pattern.push((false, -9 * MASTERED_STREAK_SPAN_SECONDS));
        assert_eq!(
            level_for(&reviews(&pattern)),
            MasteryLevel::Mastered,
            "recency decay must let an old failure fade rather than pin the card down forever",
        );
    }

    #[test]
    fn two_recent_misses_are_shaky_however_good_the_older_history() {
        let level = level_for(&reviews(&[
            (false, 4 * ONE_HOUR),
            (false, 3 * ONE_HOUR),
            (true, 2 * ONE_HOUR),
            (true, ONE_HOUR),
            (true, 0),
        ]));
        assert_eq!(level, MasteryLevel::Shaky);
    }

    #[test]
    fn the_streak_helpers_stop_at_the_first_incorrect_answer() {
        let history = reviews(&[
            (true, 3 * ONE_HOUR),
            (true, 2 * ONE_HOUR),
            (false, ONE_HOUR),
            (true, 0),
        ]);
        assert_eq!(consecutive_correct_count(&history), 2);
        assert_eq!(
            correct_streak_span_seconds(&history),
            ONE_HOUR,
            "the fourth review is correct but sits behind a miss, so it is outside the streak",
        );
    }

    #[test]
    fn the_streak_helpers_are_empty_when_the_newest_answer_was_wrong() {
        let history = reviews(&[(false, ONE_HOUR), (true, 0)]);
        assert_eq!(consecutive_correct_count(&history), 0);
        assert_eq!(correct_streak_span_seconds(&history), 0);
    }

    #[test]
    fn a_single_correct_answer_spans_no_time_at_all() {
        assert_eq!(correct_streak_span_seconds(&correct_at(&[ONE_HOUR])), 0);
    }

    #[test]
    fn a_correct_answer_never_lowers_a_level_and_a_wrong_one_never_raises_it() {
        for history_length in 0..12u32 {
            for pattern_bits in 0..(1u32 << history_length) {
                let history = reviews(
                    &(0..history_length)
                        .map(|position| {
                            (
                                pattern_bits & (1 << position) != 0,
                                -MASTERED_STREAK_SPAN_SECONDS * i64::from(position + 1),
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                let before = level_for(&history);

                let mut after_correct = vec![MasteryReview {
                    outcome: ReviewOutcome::Correct,
                    answered_at_seconds: MASTERED_STREAK_SPAN_SECONDS,
                }];
                after_correct.extend(history.iter().copied());
                assert!(
                    level_for(&after_correct).rank() >= before.rank(),
                    "a correct answer lowered {before:?} for {history:?}",
                );

                let mut after_incorrect = vec![MasteryReview {
                    outcome: ReviewOutcome::Incorrect,
                    answered_at_seconds: MASTERED_STREAK_SPAN_SECONDS,
                }];
                after_incorrect.extend(history.iter().copied());
                assert!(
                    level_for(&after_incorrect).rank() <= before.rank().max(MasteryLevel::Shaky.rank()),
                    "a wrong answer raised {before:?} for {history:?}",
                );
            }
        }
    }

    #[test]
    fn first_seeing_a_card_and_getting_it_wrong_is_not_movement() {
        assert_eq!(
            movement_direction(MasteryLevel::Unseen, MasteryLevel::Shaky),
            MovementDirection::Unchanged,
            "an unseen card revealing itself as shaky has not moved, it has only been measured",
        );
        assert_eq!(
            movement_direction(MasteryLevel::Unseen, MasteryLevel::Learning),
            MovementDirection::Up,
        );
        assert_eq!(
            movement_direction(MasteryLevel::Learning, MasteryLevel::Shaky),
            MovementDirection::Down,
        );
        assert_eq!(
            movement_direction(MasteryLevel::Solid, MasteryLevel::Solid),
            MovementDirection::Unchanged,
        );
    }

    #[test]
    fn every_level_ranks_above_the_one_below_it() {
        let ladder = [
            MasteryLevel::Unseen,
            MasteryLevel::Shaky,
            MasteryLevel::Learning,
            MasteryLevel::Solid,
            MasteryLevel::Mastered,
        ];
        for rung in ladder.windows(2) {
            assert!(
                rung[0].rank() < rung[1].rank(),
                "{:?} must rank below {:?}",
                rung[0],
                rung[1],
            );
        }
    }

    #[test]
    fn counts_tally_every_level_and_total_them() {
        let mut counts = MasteryCounts::default();
        counts.add(MasteryLevel::Unseen);
        counts.add(MasteryLevel::Shaky);
        counts.add(MasteryLevel::Shaky);
        counts.add(MasteryLevel::Learning);
        counts.add(MasteryLevel::Solid);
        counts.add(MasteryLevel::Mastered);
        assert_eq!(
            counts,
            MasteryCounts { unseen: 1, shaky: 2, learning: 1, solid: 1, mastered: 1 },
        );
        assert_eq!(counts.total(), 6);
    }

    #[test]
    fn levels_serialise_as_lowercase_names() {
        assert_eq!(serde_json::to_string(&MasteryLevel::Unseen).unwrap(), "\"unseen\"");
        assert_eq!(serde_json::to_string(&MasteryLevel::Shaky).unwrap(), "\"shaky\"");
        assert_eq!(serde_json::to_string(&MasteryLevel::Learning).unwrap(), "\"learning\"");
        assert_eq!(serde_json::to_string(&MasteryLevel::Solid).unwrap(), "\"solid\"");
        assert_eq!(serde_json::to_string(&MasteryLevel::Mastered).unwrap(), "\"mastered\"");
    }
}
