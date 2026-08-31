use crate::normalise::normalise;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfGrade {
    Again,
    Hard,
    Good,
    Easy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradableChoice {
    pub choice_id: i64,
    pub is_correct: bool,
}

pub fn parse_self_grade(raw: &str) -> Option<SelfGrade> {
    match raw {
        "again" => Some(SelfGrade::Again),
        "hard" => Some(SelfGrade::Hard),
        "good" => Some(SelfGrade::Good),
        "easy" => Some(SelfGrade::Easy),
        _ => None,
    }
}

pub fn self_grade_as_text(self_grade: SelfGrade) -> &'static str {
    match self_grade {
        SelfGrade::Again => "again",
        SelfGrade::Hard => "hard",
        SelfGrade::Good => "good",
        SelfGrade::Easy => "easy",
    }
}

pub fn correctness_of_self_grade(self_grade: SelfGrade) -> bool {
    !matches!(self_grade, SelfGrade::Again)
}

pub fn grade_multiple_choice(
    choices: &[GradableChoice],
    chosen_choice_id: i64,
) -> Option<bool> {
    choices
        .iter()
        .find(|choice| choice.choice_id == chosen_choice_id)
        .map(|choice| choice.is_correct)
}

pub fn grade_text_answer(given: &str, accepted_normalised: &[String]) -> bool {
    let comparison_key = normalise(given);
    if comparison_key.is_empty() {
        return false;
    }
    accepted_normalised.contains(&comparison_key)
}

pub const FUZZY_DIVISOR: usize = 8;
pub const FUZZY_MAX_TOLERANCE: usize = 2;
pub const FUZZY_MAX_LENGTH: usize = 120;

pub fn fuzzy_tolerance(expected_length: usize) -> usize {
    (expected_length / FUZZY_DIVISOR).min(FUZZY_MAX_TOLERANCE)
}

pub fn levenshtein_distance(left: &str, right: &str) -> usize {
    let left_characters: Vec<char> = left.chars().collect();
    let right_characters: Vec<char> = right.chars().collect();

    if left_characters.is_empty() {
        return right_characters.len();
    }
    if right_characters.is_empty() {
        return left_characters.len();
    }

    let mut previous_row: Vec<usize> = (0..=right_characters.len()).collect();
    let mut current_row: Vec<usize> = vec![0; right_characters.len() + 1];

    for (left_index, left_character) in left_characters.iter().enumerate() {
        current_row[0] = left_index + 1;
        for (right_index, right_character) in right_characters.iter().enumerate() {
            let substitution_cost = usize::from(left_character != right_character);
            current_row[right_index + 1] = (previous_row[right_index + 1] + 1)
                .min(current_row[right_index] + 1)
                .min(previous_row[right_index] + substitution_cost);
        }
        std::mem::swap(&mut previous_row, &mut current_row);
    }

    previous_row[right_characters.len()]
}

pub fn grade_flashcard_typed(given: &str, answer_md: &str) -> bool {
    let submitted = normalise(given);
    let expected = normalise(answer_md);

    if submitted.is_empty() || expected.is_empty() {
        return false;
    }
    if submitted == expected {
        return true;
    }

    let expected_length = expected.chars().count();
    let submitted_length = submitted.chars().count();
    if expected_length > FUZZY_MAX_LENGTH || submitted_length > FUZZY_MAX_LENGTH {
        return false;
    }

    let tolerance = fuzzy_tolerance(expected_length);
    if expected_length.abs_diff(submitted_length) > tolerance {
        return false;
    }

    levenshtein_distance(&submitted, &expected) <= tolerance
}

#[cfg(test)]
mod tests {
    use super::{
        correctness_of_self_grade, fuzzy_tolerance, grade_flashcard_typed, grade_multiple_choice,
        grade_text_answer, levenshtein_distance, normalise, parse_self_grade, self_grade_as_text,
        GradableChoice, SelfGrade, FUZZY_MAX_LENGTH,
    };

    fn accepted(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| (*key).to_string()).collect()
    }

    #[test]
    fn parses_the_four_grades_and_rejects_anything_else() {
        assert_eq!(parse_self_grade("again"), Some(SelfGrade::Again));
        assert_eq!(parse_self_grade("hard"), Some(SelfGrade::Hard));
        assert_eq!(parse_self_grade("good"), Some(SelfGrade::Good));
        assert_eq!(parse_self_grade("easy"), Some(SelfGrade::Easy));
        assert_eq!(parse_self_grade("medium"), None);
        assert_eq!(parse_self_grade(""), None);
        assert_eq!(parse_self_grade("Good"), None, "matching is exact, not case folded");
        assert_eq!(parse_self_grade(" good "), None, "the caller trims, this does not");
    }

    #[test]
    fn self_grade_text_round_trips() {
        for self_grade in [SelfGrade::Again, SelfGrade::Hard, SelfGrade::Good, SelfGrade::Easy] {
            let text = self_grade_as_text(self_grade);
            assert_eq!(
                parse_self_grade(text),
                Some(self_grade),
                "{text} must parse back to the grade it came from",
            );
        }
    }

    #[test]
    fn again_is_incorrect_and_the_other_three_are_correct() {
        assert!(!correctness_of_self_grade(SelfGrade::Again));
        assert!(correctness_of_self_grade(SelfGrade::Hard));
        assert!(correctness_of_self_grade(SelfGrade::Good));
        assert!(correctness_of_self_grade(SelfGrade::Easy));
    }

    #[test]
    fn an_unknown_choice_id_grades_to_none_not_false() {
        let choices = [
            GradableChoice { choice_id: 1, is_correct: false },
            GradableChoice { choice_id: 2, is_correct: true },
        ];
        assert_eq!(
            grade_multiple_choice(&choices, 99),
            None,
            "a foreign choice id must be rejectable, not silently graded wrong",
        );
        assert_eq!(grade_multiple_choice(&[], 1), None);
    }

    #[test]
    fn the_chosen_choice_decides_correctness() {
        let choices = [
            GradableChoice { choice_id: 1, is_correct: false },
            GradableChoice { choice_id: 2, is_correct: true },
            GradableChoice { choice_id: 3, is_correct: false },
        ];
        assert_eq!(grade_multiple_choice(&choices, 2), Some(true));
        assert_eq!(grade_multiple_choice(&choices, 1), Some(false));
        assert_eq!(grade_multiple_choice(&choices, 3), Some(false));
    }

    #[test]
    fn text_answer_matching_is_normalised() {
        let keys = accepted(&["k means"]);
        assert!(grade_text_answer("k-means", &keys));
        assert!(grade_text_answer("K-Means!", &keys));
        assert!(grade_text_answer("  K   MEANS  ", &keys));
        assert!(grade_text_answer("k means", &keys));
    }

    #[test]
    fn an_empty_or_punctuation_only_answer_is_incorrect_even_when_an_accepted_key_is_empty() {
        let keys = accepted(&[""]);
        assert!(
            !grade_text_answer("", &keys),
            "a blank answer must not match an accepted row that normalised to empty",
        );
        assert!(!grade_text_answer("   ", &keys));
        assert!(
            !grade_text_answer("!!!", &keys),
            "punctuation normalises to empty and must not match either",
        );
    }

    #[test]
    fn text_answer_matching_is_equality_not_substring() {
        let keys = accepted(&["k means clustering"]);
        assert!(!grade_text_answer("k", &keys));
        assert!(!grade_text_answer("clustering", &keys));
        assert!(!grade_text_answer("k means", &keys));
        assert!(grade_text_answer("k means clustering", &keys));
    }

    #[test]
    fn any_accepted_key_matches_not_just_the_first() {
        let keys = accepted(&["k means", "lloyd s algorithm", "kmeans"]);
        assert!(grade_text_answer("k-means", &keys));
        assert!(grade_text_answer("Lloyd's algorithm", &keys));
        assert!(grade_text_answer("kmeans", &keys));
        assert!(!grade_text_answer("hierarchical", &keys));
    }

    #[test]
    fn an_empty_accepted_list_never_matches() {
        assert!(!grade_text_answer("anything", &[]));
    }

    #[test]
    fn the_distance_is_zero_for_identical_strings() {
        assert_eq!(levenshtein_distance("entropy", "entropy"), 0);
        assert_eq!(levenshtein_distance("", ""), 0);
    }

    #[test]
    fn the_distance_against_an_empty_string_is_the_other_length() {
        assert_eq!(levenshtein_distance("kmeans", ""), 6);
        assert_eq!(levenshtein_distance("", "kmeans"), 6);
    }

    #[test]
    fn the_distance_is_symmetric() {
        for (left, right) in [
            ("kitten", "sitting"),
            ("entropy", "entrpy"),
            ("maximise", "minimise"),
            ("information gain", "informaton gain"),
        ] {
            assert_eq!(
                levenshtein_distance(left, right),
                levenshtein_distance(right, left),
                "{left} against {right}",
            );
        }
    }

    #[test]
    fn the_distance_matches_the_known_kitten_sitting_value() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn the_distance_counts_characters_not_bytes() {
        assert_eq!(levenshtein_distance("café", "cafe"), 1);
        assert_eq!(levenshtein_distance("naïve", "naive"), 1);
    }

    #[test]
    fn the_tolerance_matches_the_table_at_every_boundary() {
        assert_eq!(fuzzy_tolerance(0), 0);
        assert_eq!(fuzzy_tolerance(7), 0);
        assert_eq!(fuzzy_tolerance(8), 1);
        assert_eq!(fuzzy_tolerance(15), 1);
        assert_eq!(fuzzy_tolerance(16), 2);
        assert_eq!(fuzzy_tolerance(23), 2);
        assert_eq!(fuzzy_tolerance(24), 2);
        assert_eq!(fuzzy_tolerance(1000), 2);
    }

    #[test]
    fn an_exact_answer_grades_correct() {
        assert!(grade_flashcard_typed("entropy", "entropy"));
    }

    #[test]
    fn casing_and_punctuation_never_matter() {
        assert!(grade_flashcard_typed("  K-MEANS!  ", "k-means"));
        assert!(grade_flashcard_typed("Information Gain", "information gain"));
        assert!(grade_flashcard_typed("k means", "k-means"));
    }

    #[test]
    fn a_single_typo_in_a_longer_answer_is_forgiven() {
        assert!(grade_flashcard_typed("clusterng", "clustering"));
        assert!(grade_flashcard_typed("overfiting", "overfitting"));
        assert!(grade_flashcard_typed("precison", "precision"));
        assert!(grade_flashcard_typed("informaton gain", "information gain"));
    }

    #[test]
    fn a_text_answer_gets_no_tolerance() {
        assert!(!grade_flashcard_typed("ridge", "bridge"));
        assert!(!grade_flashcard_typed("cot", "cat"));
        assert!(!grade_flashcard_typed("bias", "bras"));
        assert!(!grade_flashcard_typed("entrpy", "entropy"));
    }

    #[test]
    fn two_edits_at_the_first_tolerant_length_are_rejected() {
        assert_eq!(levenshtein_distance("maximise", "minimise"), 2);
        assert_eq!(fuzzy_tolerance("minimise".chars().count()), 1);
        assert!(!grade_flashcard_typed("maximise", "minimise"));
    }

    #[test]
    fn a_clearly_wrong_answer_grades_wrong() {
        assert!(!grade_flashcard_typed("bananas", "entropy"));
        assert!(!grade_flashcard_typed("k-means", "hierarchical clustering"));
    }

    #[test]
    fn a_blank_submission_grades_wrong() {
        assert!(!grade_flashcard_typed("", "entropy"));
        assert!(!grade_flashcard_typed("     ", "entropy"));
        assert!(!grade_flashcard_typed("!!!---", "entropy"));
    }

    #[test]
    fn an_answer_that_normalises_to_nothing_never_matches() {
        assert!(!grade_flashcard_typed("entropy", "---"));
        assert!(!grade_flashcard_typed("---", "---"));
        assert!(!grade_flashcard_typed("", ""));
    }

    #[test]
    fn above_the_length_guard_only_an_exact_answer_counts() {
        let long_answer = "clustering ".repeat(20);
        assert!(long_answer.chars().count() > FUZZY_MAX_LENGTH);

        assert!(grade_flashcard_typed(&long_answer, &long_answer));

        let with_one_typo = format!("x{}", &long_answer[1..]);
        assert_eq!(levenshtein_distance(&normalise(&with_one_typo), &normalise(&long_answer)), 1);
        assert!(!grade_flashcard_typed(&with_one_typo, &long_answer));
    }

    #[test]
    fn just_below_the_length_guard_a_typo_is_still_forgiven() {
        let answer = "a".repeat(FUZZY_MAX_LENGTH - 1);
        let with_one_typo = format!("b{}", &answer[1..]);
        assert!(grade_flashcard_typed(&with_one_typo, &answer));
    }

    #[test]
    fn a_length_difference_beyond_the_tolerance_is_rejected() {
        assert!(!grade_flashcard_typed("entropy of a set", "entropy"));
        assert!(!grade_flashcard_typed("ent", "entropy"));
    }

    #[test]
    fn type_one_and_type_two_error_are_not_distinguished() {
        assert_eq!(levenshtein_distance("type i error", "type ii error"), 1);
        assert_eq!(fuzzy_tolerance("type ii error".chars().count()), 1);
        assert!(grade_flashcard_typed("type i error", "type ii error"));
        assert!(grade_flashcard_typed("type ii error", "type i error"));
    }
}
