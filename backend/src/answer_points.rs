use serde::{Deserialize, Serialize};

use crate::grading::{fuzzy_tolerance, levenshtein_distance, SelfGrade};
use crate::mastery::MasteryLevel;
use crate::normalise::normalise;

pub const MINIMUM_POINTS: usize = 2;
pub const PARTIAL_CREDIT_FLOOR: f64 = 0.5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MultiPointMode {
    #[default]
    Auto,
    On,
    Off,
}

pub fn parse_multi_point_mode(raw: &str) -> Option<MultiPointMode> {
    match raw {
        "auto" => Some(MultiPointMode::Auto),
        "on" => Some(MultiPointMode::On),
        "off" => Some(MultiPointMode::Off),
        _ => None,
    }
}

pub fn multi_point_mode_as_text(mode: MultiPointMode) -> &'static str {
    match mode {
        MultiPointMode::Auto => "auto",
        MultiPointMode::On => "on",
        MultiPointMode::Off => "off",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnswerPoint {
    pub text: String,
    pub key: String,
    pub first_word: String,
    pub first_letter: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnswerBreakdown {
    pub points: Vec<AnswerPoint>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnswerLine {
    text: String,
    carries_list_marker: bool,
}

fn strip_list_marker(line: &str) -> Option<&str> {
    let trimmed = line.trim();

    for bullet in ["- ", "* ", "+ ", "\u{2022} ", "\u{2013} ", "\u{2014} "] {
        if let Some(rest) = trimmed.strip_prefix(bullet) {
            return Some(rest.trim_start());
        }
    }

    let opening_bracket = trimmed.chars().next().filter(|character| *character == '(' || *character == '[');
    let digits_start = usize::from(opening_bracket.is_some());
    let digit_count = trimmed[digits_start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .count();
    if digit_count == 0 {
        return None;
    }

    let after_digits = &trimmed[digits_start + digit_count..];
    let closers: &[&str] = match opening_bracket {
        Some('(') => &[") "],
        Some(_) => &["] "],
        None => &[". ", ") "],
    };
    for closer in closers {
        if let Some(rest) = after_digits.strip_prefix(closer) {
            return Some(rest.trim_start());
        }
    }
    None
}

fn answer_lines(source: &str) -> Vec<AnswerLine> {
    source
        .lines()
        .filter_map(|line| {
            let stripped = strip_list_marker(line);
            let text = stripped.unwrap_or(line).trim().to_string();
            if normalise(&text).is_empty() {
                return None;
            }
            Some(AnswerLine { text, carries_list_marker: stripped.is_some() })
        })
        .collect()
}

pub fn split_answer_points(source: &str) -> Vec<String> {
    answer_lines(source).into_iter().map(|line| line.text).collect()
}

fn cue_for(text: &str) -> (String, String) {
    let comparison_key = normalise(text);
    let first_word = comparison_key
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    let first_letter = first_word
        .chars()
        .next()
        .map(|character| character.to_uppercase().to_string())
        .unwrap_or_default();
    (first_word, first_letter)
}

fn answer_point_of(text: String) -> AnswerPoint {
    let key = normalise(&text);
    let (first_word, first_letter) = cue_for(&text);
    AnswerPoint { text, key, first_word, first_letter }
}

pub fn breakdown_of(source: &str, mode: MultiPointMode) -> Option<AnswerBreakdown> {
    if mode == MultiPointMode::Off {
        return None;
    }

    let lines = answer_lines(source);
    let marked_count = lines.iter().filter(|line| line.carries_list_marker).count();

    let points_are_the_marked_lines = if marked_count >= MINIMUM_POINTS {
        mode == MultiPointMode::On || marked_count * 2 > lines.len()
    } else {
        false
    };

    if !points_are_the_marked_lines {
        if mode == MultiPointMode::On && lines.len() >= MINIMUM_POINTS && marked_count == 0 {
            return Some(AnswerBreakdown {
                points: lines.into_iter().map(|line| answer_point_of(line.text)).collect(),
                notes: Vec::new(),
            });
        }
        return None;
    }

    let mut points: Vec<AnswerPoint> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for line in lines {
        if line.carries_list_marker {
            points.push(answer_point_of(line.text));
        } else {
            notes.push(line.text);
        }
    }
    Some(AnswerBreakdown { points, notes })
}

pub fn is_multi_point(source: &str, mode: MultiPointMode) -> bool {
    breakdown_of(source, mode).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CueTier {
    Word,
    Letter,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnswerCues {
    pub tier: CueTier,
    pub visible: Vec<String>,
    pub behind_the_hint: Vec<String>,
}

pub fn cue_tier_for(level: MasteryLevel) -> CueTier {
    match level {
        MasteryLevel::Unseen | MasteryLevel::Shaky => CueTier::Word,
        MasteryLevel::Learning => CueTier::Letter,
        MasteryLevel::Solid | MasteryLevel::Mastered => CueTier::None,
    }
}

pub fn cues_for(points: &[AnswerPoint], tier: CueTier) -> AnswerCues {
    let first_words: Vec<String> =
        points.iter().map(|point| point.first_word.clone()).collect();
    let first_letters: Vec<String> =
        points.iter().map(|point| point.first_letter.clone()).collect();

    let (visible, behind_the_hint) = match tier {
        CueTier::Word => (first_words, Vec::new()),
        CueTier::Letter => (first_letters, first_words),
        CueTier::None => (Vec::new(), first_letters),
    };
    AnswerCues { tier, visible, behind_the_hint }
}

fn lines_match(submitted: &str, expected: &str) -> bool {
    if submitted.is_empty() || expected.is_empty() {
        return false;
    }
    if submitted == expected {
        return true;
    }
    let expected_length = expected.chars().count();
    let submitted_length = submitted.chars().count();
    let tolerance = fuzzy_tolerance(expected_length);
    if expected_length.abs_diff(submitted_length) > tolerance {
        return false;
    }
    levenshtein_distance(submitted, expected) <= tolerance
}

pub fn match_typed_points(given: &str, points: &[AnswerPoint]) -> Vec<bool> {
    let submitted_keys: Vec<String> = split_answer_points(given)
        .iter()
        .map(|line| normalise(line))
        .collect();
    let mut consumed = vec![false; submitted_keys.len()];
    let mut recalled = vec![false; points.len()];

    for (point_index, point) in points.iter().enumerate() {
        let exact = submitted_keys
            .iter()
            .enumerate()
            .position(|(line_index, key)| !consumed[line_index] && *key == point.key);
        let matched = exact.or_else(|| {
            submitted_keys
                .iter()
                .enumerate()
                .position(|(line_index, key)| {
                    !consumed[line_index] && lines_match(key, &point.key)
                })
        });
        if let Some(line_index) = matched {
            consumed[line_index] = true;
            recalled[point_index] = true;
        }
    }

    recalled
}

pub fn self_grade_from_point_score(recalled: usize, total: usize, hints_used: bool) -> SelfGrade {
    if total == 0 {
        return SelfGrade::Again;
    }
    if recalled >= total {
        return if hints_used { SelfGrade::Good } else { SelfGrade::Easy };
    }
    if recalled as f64 / total as f64 > PARTIAL_CREDIT_FLOOR {
        SelfGrade::Hard
    } else {
        SelfGrade::Again
    }
}

#[cfg(test)]
mod tests {
    use super::{
        breakdown_of, cue_tier_for, cues_for, is_multi_point, match_typed_points,
        multi_point_mode_as_text, parse_multi_point_mode, self_grade_from_point_score,
        split_answer_points, AnswerBreakdown, CueTier, MasteryLevel, MultiPointMode, SelfGrade,
    };

    const SEVEN_REQUIREMENTS: &str = "\
1. Ability to mine a variety of pattern types
2. Mining should be interactive
3. Mine patterns at varying granularity levels
4. Allow users to provide a priori knowledge hints
5. Should associate quality measures with patterns
6. Presentation and visualisation is often very important
7. Noisy and incomplete data must be handled";

    fn automatic(source: &str) -> Option<AnswerBreakdown> {
        breakdown_of(source, MultiPointMode::Auto)
    }

    fn point_texts(source: &str, mode: MultiPointMode) -> Vec<String> {
        breakdown_of(source, mode)
            .expect("expected a multi-point breakdown")
            .points
            .into_iter()
            .map(|point| point.text)
            .collect()
    }

    fn points_of(source: &str) -> Vec<String> {
        point_texts(source, MultiPointMode::Auto)
    }

    #[test]
    fn a_numbered_list_splits_into_its_points_without_the_numbers() {
        let points = points_of(SEVEN_REQUIREMENTS);
        assert_eq!(points.len(), 7);
        assert_eq!(points[0], "Ability to mine a variety of pattern types");
        assert_eq!(points[1], "Mining should be interactive");
        assert_eq!(points[6], "Noisy and incomplete data must be handled");
        assert!(automatic(SEVEN_REQUIREMENTS).expect("multi-point").notes.is_empty());
    }

    #[test]
    fn every_supported_list_marker_is_recognised() {
        let markers = [
            "- ", "* ", "+ ", "1. ", "1) ", "(1) ", "[1] ", "12. ", "\u{2022} ", "\u{2013} ",
            "\u{2014} ",
        ];
        for marker in markers {
            let source = format!("{marker}alpha\n{marker}beta");
            assert_eq!(
                points_of(&source),
                vec!["alpha", "beta"],
                "{marker:?} was not recognised as a list marker",
            );
        }
    }

    #[test]
    fn unmarked_lines_are_not_a_list() {
        assert!(
            automatic("Volume\nVelocity\nVariety").is_none(),
            "plain lines are ambiguous prose, so auto must leave them alone",
        );
    }

    #[test]
    fn forcing_the_mode_on_makes_unmarked_lines_into_points() {
        assert_eq!(
            point_texts("Volume\nVelocity\nVariety", MultiPointMode::On),
            vec!["Volume", "Velocity", "Variety"],
        );
    }

    #[test]
    fn a_marker_in_the_middle_of_a_line_is_left_alone() {
        assert_eq!(
            points_of("- the ratio is 3 - 4 wide\n- step 1. do the thing"),
            vec!["the ratio is 3 - 4 wide", "step 1. do the thing"],
            "only a leading marker is a marker",
        );
    }

    #[test]
    fn a_bare_number_without_a_separator_is_not_a_marker() {
        assert_eq!(points_of("- 1984 was a year\n- 7 of them"), vec!["1984 was a year", "7 of them"]);
        assert!(automatic("1984 was a year\n7 of them").is_none());
    }

    #[test]
    fn blank_and_punctuation_only_lines_are_dropped() {
        assert_eq!(points_of("- alpha\n\n   \n- beta"), vec!["alpha", "beta"]);
        assert_eq!(
            points_of("- alpha\n---\n- beta"),
            vec!["alpha", "beta"],
            "a markdown rule normalises to nothing and is neither a point nor a note",
        );
        assert!(automatic("").is_none());
        assert!(automatic("   \n  ").is_none());
    }

    #[test]
    fn a_fenced_code_block_is_never_mistaken_for_a_list() {
        let answer = "```\nIF size = small AND ISO9000 = true THEN class = solvent\n```\n\nEach condition becomes one clause.";
        assert!(
            automatic(answer).is_none(),
            "a code fence plus prose carries no list markers and must stay a plain answer",
        );
    }

    #[test]
    fn surrounding_whitespace_never_survives() {
        assert_eq!(points_of("   1.    alpha   \n\t- beta\t"), vec!["alpha", "beta"]);
    }

    #[test]
    fn a_single_marked_point_is_not_a_list() {
        assert!(
            automatic("- alpha").is_none(),
            "one point is not a list worth scoring out of one",
        );
    }

    #[test]
    fn marked_lines_must_be_a_majority_for_auto_to_claim_the_answer() {
        let mostly_prose = "\
Every root-to-leaf path is one rule.
- Trees are easier to read.
- Rules are easier to execute.
A rule set also needs a default rule.";
        assert!(
            automatic(mostly_prose).is_none(),
            "two bullets inside four lines of prose is an explanation, not a checklist",
        );
        assert_eq!(
            point_texts(mostly_prose, MultiPointMode::On),
            vec!["Trees are easier to read.", "Rules are easier to execute."],
            "forcing the mode on must take the marked lines and leave the prose as notes",
        );
    }

    #[test]
    fn prose_around_a_clear_majority_of_points_becomes_notes() {
        let answer = "\
A data warehouse.
1. A unified schema across the sources.
2. It usually resides at a single site.
3. The data is pre-processed on the way in.
It is also organised by major subject.";
        let breakdown = automatic(answer).expect("three of five lines are marked");
        assert_eq!(
            breakdown.points.iter().map(|point| point.text.as_str()).collect::<Vec<_>>(),
            [
                "A unified schema across the sources.",
                "It usually resides at a single site.",
                "The data is pre-processed on the way in.",
            ],
        );
        assert_eq!(
            breakdown.notes,
            ["A data warehouse.", "It is also organised by major subject."],
            "unmarked prose is context to read, never a point to recall",
        );
    }

    #[test]
    fn off_overrides_even_an_unambiguous_list() {
        assert!(!is_multi_point(SEVEN_REQUIREMENTS, MultiPointMode::Off));
        assert!(breakdown_of(SEVEN_REQUIREMENTS, MultiPointMode::Off).is_none());
    }

    #[test]
    fn forcing_the_mode_on_never_invents_points_from_one_line() {
        assert!(!is_multi_point("k-means", MultiPointMode::On));
        assert!(!is_multi_point("- k-means", MultiPointMode::On));
        assert!(!is_multi_point("", MultiPointMode::On));
    }

    #[test]
    fn the_mode_text_round_trips() {
        for mode in [MultiPointMode::Auto, MultiPointMode::On, MultiPointMode::Off] {
            assert_eq!(parse_multi_point_mode(multi_point_mode_as_text(mode)), Some(mode));
        }
        assert_eq!(parse_multi_point_mode("sometimes"), None);
        assert_eq!(parse_multi_point_mode("Auto"), None, "matching is exact, not case folded");
        assert_eq!(MultiPointMode::default(), MultiPointMode::Auto);
    }

    #[test]
    fn cues_come_from_the_normalised_first_word() {
        let breakdown = automatic(SEVEN_REQUIREMENTS).expect("multi-point");
        let letters: Vec<String> =
            breakdown.points.iter().map(|point| point.first_letter.clone()).collect();
        assert_eq!(letters, ["A", "M", "M", "A", "S", "P", "N"]);
        assert_eq!(breakdown.points[0].first_word, "ability");
        assert_eq!(breakdown.points[0].key, "ability to mine a variety of pattern types");
    }

    #[test]
    fn a_cue_survives_a_point_that_starts_with_punctuation() {
        let breakdown =
            automatic("- \"veracity\" of the data\n- **volume** of the data").expect("multi-point");
        assert_eq!(breakdown.points[0].first_letter, "V");
        assert_eq!(breakdown.points[1].first_word, "volume");
    }

    fn forced_points(source: &str) -> Vec<super::AnswerPoint> {
        breakdown_of(source, MultiPointMode::On).expect("multi-point").points
    }

    #[test]
    fn the_cue_tier_fades_as_the_card_climbs_the_ladder() {
        assert_eq!(cue_tier_for(MasteryLevel::Unseen), CueTier::Word);
        assert_eq!(cue_tier_for(MasteryLevel::Shaky), CueTier::Word);
        assert_eq!(cue_tier_for(MasteryLevel::Learning), CueTier::Letter);
        assert_eq!(cue_tier_for(MasteryLevel::Solid), CueTier::None);
        assert_eq!(cue_tier_for(MasteryLevel::Mastered), CueTier::None);
    }

    #[test]
    fn each_tier_shows_one_step_of_help_and_hides_the_next() {
        let points = breakdown_of(SEVEN_REQUIREMENTS, MultiPointMode::Auto)
            .expect("multi-point")
            .points;

        let word_tier = cues_for(&points, CueTier::Word);
        assert_eq!(word_tier.visible[1], "mining");
        assert!(
            word_tier.behind_the_hint.is_empty(),
            "the first words are already the most help short of the answer",
        );

        let letter_tier = cues_for(&points, CueTier::Letter);
        assert_eq!(letter_tier.visible, ["A", "M", "M", "A", "S", "P", "N"]);
        assert_eq!(letter_tier.behind_the_hint[1], "mining");

        let no_tier = cues_for(&points, CueTier::None);
        assert!(no_tier.visible.is_empty(), "a solid card recalls from the count alone");
        assert_eq!(no_tier.behind_the_hint, ["A", "M", "M", "A", "S", "P", "N"]);
    }

    #[test]
    fn every_tier_reports_one_cue_per_point_or_none_at_all() {
        let points = breakdown_of(SEVEN_REQUIREMENTS, MultiPointMode::Auto)
            .expect("multi-point")
            .points;
        for tier in [CueTier::Word, CueTier::Letter, CueTier::None] {
            let cues = cues_for(&points, tier);
            assert_eq!(cues.tier, tier);
            for cue_list in [&cues.visible, &cues.behind_the_hint] {
                assert!(
                    cue_list.is_empty() || cue_list.len() == points.len(),
                    "{tier:?} produced {} cues for {} points",
                    cue_list.len(),
                    points.len(),
                );
            }
        }
    }

    #[test]
    fn no_cue_ever_carries_a_whole_point() {
        let points = breakdown_of(SEVEN_REQUIREMENTS, MultiPointMode::Auto)
            .expect("multi-point")
            .points;
        for tier in [CueTier::Word, CueTier::Letter, CueTier::None] {
            let cues = cues_for(&points, tier);
            for (cue, point) in cues.visible.iter().zip(&points) {
                assert!(
                    cue.len() < point.key.len(),
                    "the cue {cue:?} gives away the whole point {:?}",
                    point.key,
                );
            }
        }
    }

    #[test]
    fn typed_lines_match_their_points_in_any_order() {
        let points = forced_points("Volume\nVelocity\nVariety\nVeracity");
        assert_eq!(
            match_typed_points("Veracity\nVolume", &points),
            vec![true, false, false, true],
        );
    }

    #[test]
    fn matching_ignores_case_punctuation_and_list_markers_in_the_submission() {
        let points = forced_points("Volume\nVelocity");
        assert_eq!(match_typed_points("1. VOLUME!\n- velocity", &points), vec![true, true]);
    }

    #[test]
    fn a_typo_inside_the_tolerance_still_matches() {
        let points = forced_points("data characterisation\ndata discrimination");
        assert_eq!(
            match_typed_points("data characterisaton\ndata discriminaton", &points),
            vec![true, true],
        );
    }

    #[test]
    fn a_wrong_point_matches_nothing() {
        let points = forced_points("Volume\nVelocity");
        assert_eq!(match_typed_points("bananas", &points), vec![false, false]);
        assert_eq!(match_typed_points("", &points), vec![false, false]);
        assert_eq!(match_typed_points("   \n!!!", &points), vec![false, false]);
    }

    #[test]
    fn one_typed_line_is_consumed_by_one_point_only() {
        let points = forced_points("Volume\nVolume");
        assert_eq!(
            match_typed_points("Volume", &points),
            vec![true, false],
            "a single typed line must not satisfy two identical points",
        );
    }

    #[test]
    fn an_exact_match_is_preferred_over_a_fuzzy_one() {
        let points = forced_points("type i error\ntype ii error");
        assert_eq!(
            match_typed_points("type ii error\ntype i error", &points),
            vec![true, true],
            "each point must claim its exact line rather than fuzzily stealing the other",
        );
    }

    #[test]
    fn matching_counts_characters_not_bytes() {
        let points = forced_points("caf\u{e9} data\nna\u{ef}ve bayes");
        assert_eq!(match_typed_points("cafe data\nnaive bayes", &points), vec![true, true]);
    }

    #[test]
    fn extra_typed_lines_never_create_credit() {
        let points = forced_points("Volume\nVelocity");
        assert_eq!(
            match_typed_points("Volume\nVelocity\nVariety\nVeracity", &points),
            vec![true, true],
        );
    }

    #[test]
    fn the_raw_splitter_keeps_every_line_of_a_submission() {
        assert_eq!(
            split_answer_points("1. alpha\nbeta\n- gamma"),
            vec!["alpha", "beta", "gamma"],
            "a submission is graded line by line however the learner punctuated it",
        );
    }

    #[test]
    fn full_marks_without_hints_is_easy_and_with_hints_is_good() {
        assert_eq!(self_grade_from_point_score(7, 7, false), SelfGrade::Easy);
        assert_eq!(self_grade_from_point_score(7, 7, true), SelfGrade::Good);
    }

    #[test]
    fn more_than_half_is_hard_and_half_or_less_is_again() {
        assert_eq!(self_grade_from_point_score(6, 7, false), SelfGrade::Hard);
        assert_eq!(self_grade_from_point_score(5, 7, false), SelfGrade::Hard);
        assert_eq!(self_grade_from_point_score(4, 7, false), SelfGrade::Hard);
        assert_eq!(self_grade_from_point_score(3, 7, false), SelfGrade::Again);
        assert_eq!(self_grade_from_point_score(0, 7, false), SelfGrade::Again);
    }

    #[test]
    fn exactly_half_is_again_not_hard() {
        assert_eq!(
            self_grade_from_point_score(2, 4, false),
            SelfGrade::Again,
            "half recalled is a failed rep, per the agreed threshold",
        );
        assert_eq!(self_grade_from_point_score(1, 2, false), SelfGrade::Again);
        assert_eq!(self_grade_from_point_score(2, 3, false), SelfGrade::Hard);
    }

    #[test]
    fn hints_never_matter_below_full_marks() {
        for recalled in 0..7 {
            assert_eq!(
                self_grade_from_point_score(recalled, 7, true),
                self_grade_from_point_score(recalled, 7, false),
            );
        }
    }

    #[test]
    fn a_degenerate_score_grades_again_rather_than_panicking() {
        assert_eq!(self_grade_from_point_score(0, 0, false), SelfGrade::Again);
        assert_eq!(
            self_grade_from_point_score(9, 7, false),
            SelfGrade::Easy,
            "an over-count must saturate at full marks, not divide past one",
        );
    }
}
