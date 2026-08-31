use std::collections::HashMap;

pub const BASE_WEIGHT: f64 = 1.0;
pub const MISS_RATE_WEIGHT: f64 = 60.0;
pub const STALENESS_WEIGHT: f64 = 20.0;
pub const MAXIMUM_REVIEWED_WEIGHT: f64 = BASE_WEIGHT + MISS_RATE_WEIGHT + STALENESS_WEIGHT;
pub const NEVER_SEEN_HEADROOM: f64 = 1.0;
pub const NEVER_SEEN_WEIGHT: f64 = MAXIMUM_REVIEWED_WEIGHT + NEVER_SEEN_HEADROOM;
pub const RECENT_REVIEW_LIMIT: i64 = 10;
pub const RECENCY_DECAY: f64 = 0.7;
pub const STALENESS_HALF_LIFE_SECONDS: f64 = 172_800.0;
pub const NO_REPEAT_WINDOW: usize = 8;

const _: () = assert!(NEVER_SEEN_WEIGHT > MAXIMUM_REVIEWED_WEIGHT);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutcome {
    Correct,
    Incorrect,
}

#[derive(Debug, Clone)]
pub struct CandidateCard {
    pub card_id: i64,
    pub review_count: i64,
    pub recent_review_outcomes: Vec<ReviewOutcome>,
    pub seconds_since_last_review: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CandidateRow {
    pub card_id: i64,
    pub review_count: i64,
    pub correct: Option<bool>,
    pub recency_rank: Option<i64>,
    pub age_seconds: Option<i64>,
}

pub fn weighted_miss_rate(outcomes: &[ReviewOutcome]) -> f64 {
    if outcomes.is_empty() {
        return 0.0;
    }
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (position, outcome) in outcomes.iter().enumerate() {
        let coefficient = RECENCY_DECAY.powi(position as i32);
        denominator += coefficient;
        if *outcome == ReviewOutcome::Incorrect {
            numerator += coefficient;
        }
    }
    numerator / denominator
}

pub fn staleness_fraction(seconds_since_last_review: Option<i64>) -> f64 {
    match seconds_since_last_review {
        None => 0.0,
        Some(seconds) => {
            let elapsed = seconds.max(0) as f64;
            1.0 - 0.5_f64.powf(elapsed / STALENESS_HALF_LIFE_SECONDS)
        }
    }
}

pub fn weight_for(candidate: &CandidateCard) -> f64 {
    if candidate.review_count == 0 {
        return NEVER_SEEN_WEIGHT;
    }
    BASE_WEIGHT
        + MISS_RATE_WEIGHT * weighted_miss_rate(&candidate.recent_review_outcomes)
        + STALENESS_WEIGHT * staleness_fraction(candidate.seconds_since_last_review)
}

pub fn fold_candidate_rows(rows: Vec<CandidateRow>) -> Vec<CandidateCard> {
    let mut ordered_card_ids: Vec<i64> = Vec::new();
    let mut review_counts: HashMap<i64, i64> = HashMap::new();
    let mut ranked_reviews: HashMap<i64, Vec<(i64, bool, i64)>> = HashMap::new();

    for row in rows {
        if review_counts.insert(row.card_id, row.review_count).is_none() {
            ordered_card_ids.push(row.card_id);
        }
        if let (Some(rank), Some(correct), Some(age)) =
            (row.recency_rank, row.correct, row.age_seconds)
        {
            ranked_reviews
                .entry(row.card_id)
                .or_default()
                .push((rank, correct, age));
        }
    }

    ordered_card_ids
        .into_iter()
        .map(|card_id| {
            let mut reviews = ranked_reviews.remove(&card_id).unwrap_or_default();
            reviews.sort_by_key(|(rank, _, _)| *rank);
            CandidateCard {
                card_id,
                review_count: review_counts.get(&card_id).copied().unwrap_or(0),
                seconds_since_last_review: reviews.first().map(|(_, _, age)| *age),
                recent_review_outcomes: reviews
                    .iter()
                    .map(|(_, correct, _)| {
                        if *correct {
                            ReviewOutcome::Correct
                        } else {
                            ReviewOutcome::Incorrect
                        }
                    })
                    .collect(),
            }
        })
        .collect()
}

pub fn effective_no_repeat_window(eligible_card_count: usize) -> usize {
    NO_REPEAT_WINDOW.min(eligible_card_count.saturating_sub(1))
}

pub fn excluded_card_ids(
    recent_review_card_ids: &[i64],
    eligible_card_count: usize,
) -> Vec<i64> {
    let window = effective_no_repeat_window(eligible_card_count);
    let mut excluded: Vec<i64> = Vec::new();
    for card_id in recent_review_card_ids.iter().take(window) {
        if !excluded.contains(card_id) {
            excluded.push(*card_id);
        }
    }
    excluded
}

pub fn select_card(
    candidates: &[CandidateCard],
    recent_review_card_ids: &[i64],
    unresolved_miss_card_ids: &[i64],
    roll: f64,
) -> Option<i64> {
    if candidates.is_empty() {
        return None;
    }
    let excluded = excluded_card_ids(recent_review_card_ids, candidates.len());
    let included: Vec<&CandidateCard> = candidates
        .iter()
        .filter(|candidate| !excluded.contains(&candidate.card_id))
        .collect();

    let unresolved: Vec<&CandidateCard> = included
        .iter()
        .copied()
        .filter(|candidate| unresolved_miss_card_ids.contains(&candidate.card_id))
        .collect();
    let selectable = if unresolved.is_empty() { &included } else { &unresolved };

    let total: f64 = selectable.iter().map(|candidate| weight_for(candidate)).sum();
    let target = roll * total;
    let mut cumulative = 0.0;
    for candidate in selectable {
        cumulative += weight_for(candidate);
        if target < cumulative {
            return Some(candidate.card_id);
        }
    }
    selectable.last().map(|candidate| candidate.card_id)
}

#[cfg(test)]
mod tests {
    use super::{
        effective_no_repeat_window, excluded_card_ids, fold_candidate_rows, select_card,
        staleness_fraction, weight_for, weighted_miss_rate, CandidateCard, CandidateRow,
        ReviewOutcome, BASE_WEIGHT, MAXIMUM_REVIEWED_WEIGHT, NEVER_SEEN_WEIGHT, NO_REPEAT_WINDOW,
        STALENESS_HALF_LIFE_SECONDS,
    };

    const ONE_DAY: i64 = 86_400;

    fn reviewed(card_id: i64, outcomes: &[ReviewOutcome], age: i64) -> CandidateCard {
        CandidateCard {
            card_id,
            review_count: outcomes.len() as i64,
            recent_review_outcomes: outcomes.to_vec(),
            seconds_since_last_review: Some(age),
        }
    }

    fn never_seen(card_id: i64) -> CandidateCard {
        CandidateCard {
            card_id,
            review_count: 0,
            recent_review_outcomes: Vec::new(),
            seconds_since_last_review: None,
        }
    }

    fn pool_of(size: usize) -> Vec<CandidateCard> {
        (0..size).map(|index| never_seen(index as i64 + 1)).collect()
    }

    #[test]
    fn a_never_seen_card_outweighs_the_worst_possible_reviewed_card() {
        let outcome_patterns: Vec<Vec<ReviewOutcome>> = vec![
            vec![ReviewOutcome::Incorrect; 10],
            vec![ReviewOutcome::Incorrect],
            vec![ReviewOutcome::Correct; 10],
            vec![ReviewOutcome::Incorrect, ReviewOutcome::Correct],
            vec![ReviewOutcome::Correct, ReviewOutcome::Incorrect],
        ];
        let ages = [0, 1, ONE_DAY, 2 * ONE_DAY, 3650 * ONE_DAY, i64::MAX];

        let unseen_weight = weight_for(&never_seen(99));

        for outcomes in &outcome_patterns {
            for age in ages {
                let candidate = reviewed(1, outcomes, age);
                assert!(
                    weight_for(&candidate) < unseen_weight,
                    "a reviewed card ({outcomes:?} at age {age}) weighed {} which is not below \
                     the never-seen weight {unseen_weight}",
                    weight_for(&candidate),
                );
            }
        }
    }

    #[test]
    fn the_reviewed_bound_tracks_the_terms_it_bounds() {
        let worst_case_reviewed = weight_for(&CandidateCard {
            card_id: 1,
            review_count: 1,
            recent_review_outcomes: vec![ReviewOutcome::Incorrect],
            seconds_since_last_review: Some(i64::MAX),
        });
        assert!(
            worst_case_reviewed <= MAXIMUM_REVIEWED_WEIGHT,
            "the worst reachable reviewed weight {worst_case_reviewed} exceeded its stated \
             bound {MAXIMUM_REVIEWED_WEIGHT}",
        );
        assert!(
            (worst_case_reviewed - MAXIMUM_REVIEWED_WEIGHT).abs() < 1e-9,
            "the bound must be tight, not merely an upper limit: worst case was \
             {worst_case_reviewed} against a bound of {MAXIMUM_REVIEWED_WEIGHT}",
        );
    }

    #[test]
    fn never_seen_is_decided_by_review_count_not_the_outcome_list() {
        let odd_shape = CandidateCard {
            card_id: 1,
            review_count: 50,
            recent_review_outcomes: Vec::new(),
            seconds_since_last_review: Some(ONE_DAY),
        };
        let weight = weight_for(&odd_shape);
        assert!(
            weight < NEVER_SEEN_WEIGHT,
            "a card with reviews must never be treated as never-seen",
        );
        assert!(weight.is_finite());
    }

    #[test]
    fn recent_misses_outweigh_older_misses() {
        let miss_newest = reviewed(
            1,
            &[ReviewOutcome::Incorrect, ReviewOutcome::Correct],
            ONE_DAY,
        );
        let miss_oldest = reviewed(
            2,
            &[ReviewOutcome::Correct, ReviewOutcome::Incorrect],
            ONE_DAY,
        );
        assert!(
            weight_for(&miss_newest) > weight_for(&miss_oldest),
            "a miss on the most recent attempt must weigh more than the same miss further back",
        );
    }

    #[test]
    fn a_full_miss_rate_beats_maximum_staleness() {
        let all_missed_fresh = reviewed(1, &[ReviewOutcome::Incorrect], 0);
        let all_correct_ancient = reviewed(2, &[ReviewOutcome::Correct], 3650 * ONE_DAY);
        assert!(
            weight_for(&all_missed_fresh) > weight_for(&all_correct_ancient),
            "miss rate must outrank staleness, per the spec's ordering",
        );
    }

    #[test]
    fn staleness_breaks_a_miss_rate_tie() {
        let fresher = reviewed(1, &[ReviewOutcome::Correct], ONE_DAY);
        let staler = reviewed(2, &[ReviewOutcome::Correct], 5 * ONE_DAY);
        assert!(
            weight_for(&staler) > weight_for(&fresher),
            "with identical history, the card unseen for longer must weigh more",
        );
    }

    #[test]
    fn staleness_reaches_one_half_at_the_half_life() {
        let at_half_life = staleness_fraction(Some(STALENESS_HALF_LIFE_SECONDS as i64));
        assert!(
            (at_half_life - 0.5).abs() < 1e-9,
            "expected 0.5 at the half life, got {at_half_life}",
        );

        let ages = [0, 3_600, ONE_DAY, 2 * ONE_DAY, 3650 * ONE_DAY];
        let mut previous = staleness_fraction(Some(-1));
        for age in ages {
            let current = staleness_fraction(Some(age));
            assert!(current >= previous, "staleness must be monotonic in age");
            assert!((0.0..=1.0).contains(&current), "staleness must stay in [0, 1]");
            previous = current;
        }
        assert!(staleness_fraction(Some(i64::MAX)) <= 1.0);
    }

    #[test]
    fn a_card_just_reviewed_gets_no_staleness_bonus() {
        assert_eq!(staleness_fraction(Some(0)), 0.0);
        assert_eq!(
            staleness_fraction(Some(-500)),
            0.0,
            "a clock-skewed future timestamp must not earn a bonus",
        );
        let just_reviewed = reviewed(1, &[ReviewOutcome::Correct], 0);
        assert_eq!(weight_for(&just_reviewed), BASE_WEIGHT);
    }

    #[test]
    fn every_weight_is_positive_and_finite() {
        let candidates = [
            never_seen(1),
            reviewed(2, &[], 0),
            reviewed(3, &[ReviewOutcome::Incorrect; 10], i64::MAX),
            reviewed(4, &[ReviewOutcome::Correct], -9_999),
            CandidateCard {
                card_id: 5,
                review_count: 3,
                recent_review_outcomes: Vec::new(),
                seconds_since_last_review: None,
            },
        ];
        for candidate in &candidates {
            let weight = weight_for(candidate);
            assert!(weight.is_finite(), "card {} weighed {weight}", candidate.card_id);
            assert!(weight > 0.0, "card {} weighed {weight}", candidate.card_id);
        }
        assert_eq!(weighted_miss_rate(&[]), 0.0, "an empty slice must not divide by zero");
    }

    #[test]
    fn folding_orders_outcomes_by_recency_rank() {
        let rows = vec![
            CandidateRow { card_id: 7, review_count: 3, correct: Some(true), recency_rank: Some(3), age_seconds: Some(300) },
            CandidateRow { card_id: 7, review_count: 3, correct: Some(false), recency_rank: Some(1), age_seconds: Some(100) },
            CandidateRow { card_id: 7, review_count: 3, correct: Some(true), recency_rank: Some(2), age_seconds: Some(200) },
        ];
        let folded = fold_candidate_rows(rows);
        assert_eq!(folded.len(), 1);
        assert_eq!(
            folded[0].recent_review_outcomes,
            vec![ReviewOutcome::Incorrect, ReviewOutcome::Correct, ReviewOutcome::Correct],
            "outcomes must be newest-first by recency_rank, not by row arrival order",
        );
    }

    #[test]
    fn folding_takes_the_age_of_the_most_recent_review_only() {
        let rows = vec![
            CandidateRow { card_id: 7, review_count: 3, correct: Some(true), recency_rank: Some(2), age_seconds: Some(1_000) },
            CandidateRow { card_id: 7, review_count: 3, correct: Some(true), recency_rank: Some(3), age_seconds: Some(9_999) },
            CandidateRow { card_id: 7, review_count: 3, correct: Some(true), recency_rank: Some(1), age_seconds: Some(10) },
        ];
        let folded = fold_candidate_rows(rows);
        assert_eq!(folded[0].seconds_since_last_review, Some(10));
    }

    #[test]
    fn folding_produces_a_never_seen_candidate_from_a_row_with_no_reviews() {
        let rows = vec![CandidateRow {
            card_id: 4,
            review_count: 0,
            correct: None,
            recency_rank: None,
            age_seconds: None,
        }];
        let folded = fold_candidate_rows(rows);
        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].review_count, 0);
        assert!(folded[0].recent_review_outcomes.is_empty());
        assert_eq!(folded[0].seconds_since_last_review, None);
        assert_eq!(weight_for(&folded[0]), NEVER_SEEN_WEIGHT);
    }

    #[test]
    fn folding_preserves_the_query_order_of_cards() {
        let rows = vec![
            CandidateRow { card_id: 30, review_count: 0, correct: None, recency_rank: None, age_seconds: None },
            CandidateRow { card_id: 10, review_count: 0, correct: None, recency_rank: None, age_seconds: None },
            CandidateRow { card_id: 20, review_count: 0, correct: None, recency_rank: None, age_seconds: None },
        ];
        let folded = fold_candidate_rows(rows);
        let card_ids: Vec<i64> = folded.iter().map(|candidate| candidate.card_id).collect();
        assert_eq!(
            card_ids,
            vec![30, 10, 20],
            "the fold must preserve the query's ordering, not a hash map's",
        );
    }

    #[test]
    fn selection_is_deterministic_for_a_given_roll() {
        let candidates = vec![
            reviewed(1, &[ReviewOutcome::Incorrect], ONE_DAY),
            reviewed(2, &[ReviewOutcome::Correct], ONE_DAY),
            never_seen(3),
        ];
        let first = select_card(&candidates, &[], &[], 0.42);
        for _ in 0..100 {
            assert_eq!(select_card(&candidates, &[], &[], 0.42), first);
        }
    }

    #[test]
    fn a_roll_of_zero_selects_the_first_included_candidate() {
        let candidates = pool_of(4);
        assert_eq!(select_card(&candidates, &[], &[], 0.0), Some(1));
    }

    #[test]
    fn a_roll_just_below_one_selects_the_last_included_candidate() {
        let candidates = pool_of(4);
        assert_eq!(select_card(&candidates, &[], &[], 0.999_999), Some(4));
    }

    #[test]
    fn a_roll_of_exactly_one_still_returns_a_candidate() {
        let candidates = pool_of(4);
        assert_eq!(select_card(&candidates, &[], &[], 1.0), Some(4));
    }

    #[test]
    fn any_roll_selects_a_candidate_from_the_included_set() {
        let candidates = pool_of(4);
        let card_ids = [1, 2, 3, 4];
        for roll in [-5.0, 0.0, 0.25, 0.5, 1.0, 12.0, f64::NAN, f64::INFINITY] {
            let selected = select_card(&candidates, &[], &[], roll)
                .unwrap_or_else(|| panic!("roll {roll} selected nothing"));
            assert!(
                card_ids.contains(&selected),
                "roll {roll} selected {selected}, which is not in the pool",
            );
        }
    }

    #[test]
    fn selection_frequency_tracks_the_weights() {
        let heavy = never_seen(1);
        let light = reviewed(2, &[ReviewOutcome::Correct], 0);
        let candidates = vec![heavy, light];

        let expected_ratio = weight_for(&candidates[0]) / weight_for(&candidates[1]);

        let steps = 10_000;
        let mut heavy_hits = 0;
        let mut light_hits = 0;
        for step in 0..steps {
            let roll = step as f64 / steps as f64;
            match select_card(&candidates, &[], &[], roll) {
                Some(1) => heavy_hits += 1,
                Some(2) => light_hits += 1,
                other => panic!("unexpected selection {other:?}"),
            }
        }

        let observed_ratio = heavy_hits as f64 / light_hits as f64;
        let relative_error = (observed_ratio - expected_ratio).abs() / expected_ratio;
        assert!(
            relative_error < 0.02,
            "expected a hit ratio near {expected_ratio}, observed {observed_ratio}",
        );
    }

    #[test]
    fn the_window_is_eight_for_a_large_pool() {
        assert_eq!(effective_no_repeat_window(50), NO_REPEAT_WINDOW);
        assert_eq!(effective_no_repeat_window(9), NO_REPEAT_WINDOW);
    }

    #[test]
    fn the_window_shrinks_below_nine_cards() {
        assert_eq!(effective_no_repeat_window(0), 0);
        assert_eq!(effective_no_repeat_window(1), 0);
        assert_eq!(effective_no_repeat_window(3), 2);
        assert_eq!(effective_no_repeat_window(8), 7);
    }

    #[test]
    fn a_three_card_pool_excludes_the_previous_two_only() {
        assert_eq!(excluded_card_ids(&[3, 2, 1], 3), vec![3, 2]);
    }

    #[test]
    fn exclusion_deduplicates_and_ignores_ids_outside_the_pool() {
        assert_eq!(
            excluded_card_ids(&[5, 5, 5, 5], 3),
            vec![5],
            "a repeated id must consume one exclusion slot, not several",
        );
        let candidates = pool_of(3);
        assert!(
            select_card(&candidates, &[99, 98], &[], 0.0).is_some(),
            "ids that are not in the pool must not starve the selector",
        );
    }

    #[test]
    fn a_single_card_pool_repeats_that_card() {
        let candidates = pool_of(1);
        assert_eq!(select_card(&candidates, &[1], &[], 0.0), Some(1));
        assert_eq!(select_card(&candidates, &[1, 1, 1], &[], 0.5), Some(1));
    }

    #[test]
    fn an_empty_candidate_list_selects_nothing() {
        assert_eq!(select_card(&[], &[], &[], 0.0), None);
        assert_eq!(select_card(&[], &[1, 2, 3], &[], 0.5), None);
    }

    #[test]
    fn the_window_never_starves_the_selector() {
        for pool_size in 1..=12usize {
            let candidates = pool_of(pool_size);
            let card_ids: Vec<i64> = candidates.iter().map(|c| c.card_id).collect();

            for history_length in 0..=(2 * pool_size) {
                let history: Vec<i64> = (0..history_length)
                    .map(|index| card_ids[index % pool_size])
                    .rev()
                    .collect();

                for roll in [0.0, 0.5, 0.999_999] {
                    assert!(
                        select_card(&candidates, &history, &[], roll).is_some(),
                        "pool of {pool_size} starved with history {history:?} at roll {roll}",
                    );
                }
            }
        }
    }

    #[test]
    fn the_window_never_starves_on_any_short_history() {
        for pool_size in 1..=4usize {
            let candidates = pool_of(pool_size);
            for history_length in 0..=6u32 {
                let combinations = pool_size.pow(history_length);
                for combination in 0..combinations {
                    let mut remaining = combination;
                    let mut history: Vec<i64> = Vec::new();
                    for _ in 0..history_length {
                        history.push((remaining % pool_size) as i64 + 1);
                        remaining /= pool_size;
                    }
                    assert!(
                        select_card(&candidates, &history, &[], 0.5).is_some(),
                        "pool of {pool_size} starved with history {history:?}",
                    );
                }
            }
        }
    }

    #[test]
    fn an_unresolved_miss_outranks_even_a_never_seen_card() {
        let candidates = vec![
            never_seen(1),
            never_seen(2),
            reviewed(3, &[ReviewOutcome::Incorrect], 0),
        ];
        for step in 0..1_000 {
            let roll = step as f64 / 1_000.0;
            assert_eq!(
                select_card(&candidates, &[], &[3], roll),
                Some(3),
                "an unresolved miss must be served ahead of the heaviest ordinary card",
            );
        }
    }

    #[test]
    fn several_unresolved_misses_are_all_reachable() {
        let candidates = pool_of(4);
        let mut seen: Vec<i64> = Vec::new();
        for step in 0..1_000 {
            let roll = step as f64 / 1_000.0;
            let selected = select_card(&candidates, &[], &[2, 4], roll).unwrap();
            assert!(
                [2, 4].contains(&selected),
                "selected {selected}, which is not an unresolved miss",
            );
            if !seen.contains(&selected) {
                seen.push(selected);
            }
        }
        seen.sort_unstable();
        assert_eq!(seen, vec![2, 4], "every unresolved miss must be reachable");
    }

    #[test]
    fn an_unresolved_miss_still_waits_out_the_no_repeat_window() {
        let candidates = pool_of(4);
        for step in 0..1_000 {
            let roll = step as f64 / 1_000.0;
            let selected = select_card(&candidates, &[2], &[2], roll).unwrap();
            assert_ne!(
                selected, 2,
                "a miss inside the no-repeat window must not be served again immediately",
            );
        }
    }

    #[test]
    fn a_miss_returns_as_soon_as_it_leaves_the_no_repeat_window() {
        let candidates = pool_of(4);
        assert_eq!(
            effective_no_repeat_window(candidates.len()),
            3,
            "this test's history is built around a window of three",
        );
        for roll in [0.0, 0.5, 0.999_999] {
            assert_eq!(
                select_card(&candidates, &[3, 4, 1, 2], &[2], roll),
                Some(2),
                "once outside the window the miss must be served at the next opportunity",
            );
        }
    }

    #[test]
    fn unresolved_ids_outside_the_pool_do_not_starve_the_selector() {
        let candidates = pool_of(3);
        for roll in [0.0, 0.5, 0.999_999] {
            assert!(
                select_card(&candidates, &[], &[99], roll).is_some(),
                "an unresolved id that is not in the pool must fall back to ordinary selection",
            );
        }
    }

    #[test]
    fn a_resolved_miss_returns_to_ordinary_weighting() {
        let candidates = vec![
            never_seen(1),
            reviewed(2, &[ReviewOutcome::Correct, ReviewOutcome::Incorrect], 0),
        ];
        let forced = select_card(&candidates, &[], &[2], 0.5);
        assert_eq!(forced, Some(2));
        let unforced = select_card(&candidates, &[], &[], 0.5);
        assert_eq!(
            unforced,
            select_card(&candidates, &[], &[], 0.5),
            "with nothing unresolved, selection must behave exactly as before",
        );
        assert_ne!(
            unforced, forced,
            "the never-seen card must win at this roll once the miss is resolved",
        );
    }

    #[test]
    fn a_card_inside_the_window_is_never_selected() {
        let candidates = pool_of(10);
        let history = vec![10, 9, 8, 7, 6, 5, 4, 3];
        for step in 0..1_000 {
            let roll = step as f64 / 1_000.0;
            let selected = select_card(&candidates, &history, &[], roll).unwrap();
            assert!(
                !history.contains(&selected),
                "selected {selected}, which is inside the no-repeat window",
            );
        }
    }
}
