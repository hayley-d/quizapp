use crate::grading::SelfGrade;

pub const INITIAL_EASE: f64 = 2.5;
pub const MINIMUM_EASE: f64 = 1.3;
pub const FIRST_INTERVAL_DAYS: f64 = 1.0;
pub const SECOND_INTERVAL_DAYS: f64 = 6.0;
pub const PASSING_QUALITY: u8 = 3;
pub const QUALITY_CORRECT: u8 = 4;
pub const QUALITY_INCORRECT: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScheduleState {
    pub interval_days: f64,
    pub ease: f64,
    pub repetitions: i64,
    pub lapses: i64,
}

pub fn initial_state() -> ScheduleState {
    ScheduleState { interval_days: 0.0, ease: INITIAL_EASE, repetitions: 0, lapses: 0 }
}

pub fn quality_for(correct: bool, self_grade: Option<SelfGrade>) -> u8 {
    match self_grade {
        Some(SelfGrade::Again) => 1,
        Some(SelfGrade::Hard) => 3,
        Some(SelfGrade::Good) => 4,
        Some(SelfGrade::Easy) => 5,
        None if correct => QUALITY_CORRECT,
        None => QUALITY_INCORRECT,
    }
}

pub fn apply(state: &ScheduleState, quality: u8) -> ScheduleState {
    let difference = 5.0 - f64::from(quality);
    let adjusted_ease =
        (state.ease + (0.1 - difference * (0.08 + difference * 0.02))).max(MINIMUM_EASE);

    if quality < PASSING_QUALITY {
        return ScheduleState {
            interval_days: FIRST_INTERVAL_DAYS,
            ease: state.ease,
            repetitions: 0,
            lapses: state.lapses + 1,
        };
    }

    let repetitions = state.repetitions + 1;
    let interval_days = match repetitions {
        1 => FIRST_INTERVAL_DAYS,
        2 => SECOND_INTERVAL_DAYS,
        _ => (state.interval_days * adjusted_ease).round(),
    };

    ScheduleState { interval_days, ease: adjusted_ease, repetitions, lapses: state.lapses }
}

pub fn replay(qualities: &[u8]) -> ScheduleState {
    qualities.iter().fold(initial_state(), |state, quality| apply(&state, *quality))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_follows_the_specified_table() {
        assert_eq!(quality_for(true, None), 4);
        assert_eq!(quality_for(false, None), 2);
        assert_eq!(quality_for(false, Some(SelfGrade::Again)), 1);
        assert_eq!(quality_for(true, Some(SelfGrade::Hard)), 3);
        assert_eq!(quality_for(true, Some(SelfGrade::Good)), 4);
        assert_eq!(quality_for(true, Some(SelfGrade::Easy)), 5);
    }

    #[test]
    fn the_first_two_intervals_are_one_day_and_six_days() {
        let first = apply(&initial_state(), 4);
        assert_eq!(first.repetitions, 1);
        assert_eq!(first.interval_days, 1.0);

        let second = apply(&first, 4);
        assert_eq!(second.repetitions, 2);
        assert_eq!(second.interval_days, 6.0);
    }

    #[test]
    fn the_third_interval_multiplies_by_the_ease_factor() {
        let state = replay(&[4, 4]);
        let third = apply(&state, 4);
        assert_eq!(third.repetitions, 3);
        assert_eq!(third.interval_days, 15.0);
        assert!(third.interval_days > 6.0);
    }

    #[test]
    fn a_lapse_resets_the_repetitions_and_counts_itself() {
        let state = replay(&[4, 4, 4]);
        let lapsed = apply(&state, 1);
        assert_eq!(lapsed.repetitions, 0);
        assert_eq!(lapsed.interval_days, 1.0);
        assert_eq!(lapsed.lapses, 1);
    }

    #[test]
    fn a_lapse_leaves_the_ease_factor_untouched() {
        let state = replay(&[4, 4, 4]);
        let lapsed = apply(&state, 1);
        assert_eq!(
            lapsed.ease, state.ease,
            "the original SM-2 restarts repetitions without changing the E-Factor",
        );
    }

    #[test]
    fn a_hard_answer_lowers_the_ease_without_lapsing() {
        let state = apply(&initial_state(), 3);
        assert_eq!(state.repetitions, 1);
        assert_eq!(state.lapses, 0);
        assert_eq!(state.ease, 2.36, "quality 3 must reduce the ease from 2.5 to exactly 2.36");
    }

    #[test]
    fn an_easy_answer_raises_the_ease() {
        let state = apply(&initial_state(), 5);
        assert_eq!(state.ease, 2.6, "quality 5 must raise the ease from 2.5 to exactly 2.6");
    }

    #[test]
    fn the_ease_never_falls_below_the_minimum() {
        let mut state = initial_state();
        for _ in 0..40 {
            state = apply(&state, 3);
        }
        assert!(
            state.ease >= 1.3,
            "ease fell through the floor: {}",
            state.ease,
        );
        assert_eq!(state.ease, 1.3);
    }

    #[test]
    fn replaying_equals_applying_one_review_at_a_time() {
        let qualities = [4, 5, 3, 1, 4, 4, 2, 5, 3, 4];
        let mut incremental = initial_state();
        for quality in qualities {
            incremental = apply(&incremental, quality);
        }
        assert_eq!(replay(&qualities), incremental);
    }

    #[test]
    fn replaying_nothing_is_the_initial_state() {
        assert_eq!(replay(&[]), initial_state());
    }
}
