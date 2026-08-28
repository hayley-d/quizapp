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

pub fn grade_short_answer(given: &str, accepted_normalised: &[String]) -> bool {
    let comparison_key = normalise(given);
    if comparison_key.is_empty() {
        return false;
    }
    accepted_normalised.contains(&comparison_key)
}

#[cfg(test)]
mod tests {
    use super::{
        correctness_of_self_grade, grade_multiple_choice, grade_short_answer, parse_self_grade,
        self_grade_as_text, GradableChoice, SelfGrade,
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
    fn short_answer_matching_is_normalised() {
        let keys = accepted(&["k means"]);
        assert!(grade_short_answer("k-means", &keys));
        assert!(grade_short_answer("K-Means!", &keys));
        assert!(grade_short_answer("  K   MEANS  ", &keys));
        assert!(grade_short_answer("k means", &keys));
    }

    #[test]
    fn an_empty_or_punctuation_only_answer_is_incorrect_even_when_an_accepted_key_is_empty() {
        let keys = accepted(&[""]);
        assert!(
            !grade_short_answer("", &keys),
            "a blank answer must not match an accepted row that normalised to empty",
        );
        assert!(!grade_short_answer("   ", &keys));
        assert!(
            !grade_short_answer("!!!", &keys),
            "punctuation normalises to empty and must not match either",
        );
    }

    #[test]
    fn short_answer_matching_is_equality_not_substring() {
        let keys = accepted(&["k means clustering"]);
        assert!(!grade_short_answer("k", &keys));
        assert!(!grade_short_answer("clustering", &keys));
        assert!(!grade_short_answer("k means", &keys));
        assert!(grade_short_answer("k means clustering", &keys));
    }

    #[test]
    fn any_accepted_key_matches_not_just_the_first() {
        let keys = accepted(&["k means", "lloyd s algorithm", "kmeans"]);
        assert!(grade_short_answer("k-means", &keys));
        assert!(grade_short_answer("Lloyd's algorithm", &keys));
        assert!(grade_short_answer("kmeans", &keys));
        assert!(!grade_short_answer("hierarchical", &keys));
    }

    #[test]
    fn an_empty_accepted_list_never_matches() {
        assert!(!grade_short_answer("anything", &[]));
    }
}
