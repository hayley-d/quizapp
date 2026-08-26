//! The comparison key for short-answer grading.
//!
//! Computed once on insert into `accepted.normalised` so matching an answer is
//! an indexed lookup, not a scan that re-normalises every row. Pure and
//! DB-free, per the spec's "testable core"; Part 3's grading calls the same
//! function on the student's input.

use unicode_normalization::UnicodeNormalization;

/// NFKC, lowercase, punctuation to spaces, whitespace collapsed, trimmed.
///
/// Punctuation becomes a space rather than being deleted so that "k-means"
/// and "k means" produce the same key. Deleting it in place would make those
/// two spellings disagree, which is the case this exists to handle.
pub fn normalise(input: &str) -> String {
    let folded: String = input.nfkc().flat_map(char::to_lowercase).collect();

    let spaced: String = folded
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();

    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::normalise;

    #[test]
    fn folds_case() {
        assert_eq!(normalise("K-Means"), normalise("k-means"));
    }

    #[test]
    fn punctuation_becomes_a_space_not_nothing() {
        // The whole point: hyphenated and spaced spellings must share a key.
        assert_eq!(normalise("k-means"), "k means");
        assert_eq!(normalise("k means"), "k means");
        assert_eq!(normalise("Bayes' theorem"), "bayes theorem");
    }

    #[test]
    fn collapses_and_trims_whitespace() {
        assert_eq!(normalise("  decision \t\n  tree  "), "decision tree");
    }

    #[test]
    fn applies_nfkc_compatibility_folding() {
        // Full-width K (U+FF2B) and the fi ligature (U+FB01) only fold under NFKC.
        assert_eq!(normalise("\u{FF2B}-means"), "k means");
        assert_eq!(normalise("con\u{FB01}dence"), "confidence");
    }

    #[test]
    fn digits_and_letters_survive() {
        assert_eq!(normalise("10,000 rows"), "10 000 rows");
    }

    #[test]
    fn is_idempotent() {
        let once = normalise("K-Means  Clustering!");
        assert_eq!(normalise(&once), once);
    }

    #[test]
    fn punctuation_only_input_is_empty() {
        assert_eq!(normalise("  ---  "), "");
    }

    #[test]
    fn non_alphanumeric_becomes_space_not_nothing() {
        // The positive test asserts "k-means" == "k means".
        // These assert that pairs differing only in a non-alphanumeric character
        // stay distinct — the core failure mode to catch is an implementation that
        // deletes non-alphanumerics instead of replacing them with spaces.
        assert_ne!(normalise("a-b"), normalise("ab"));
        assert_ne!(normalise("1,000"), normalise("1000"));
        assert_ne!(normalise("F1"), normalise("F 1"));
    }

    #[test]
    fn trailing_symbols_vanish_a_known_limitation() {
        // Answers distinguished only by trailing symbols cannot be told apart,
        // because trailing symbols become trailing spaces and are trimmed.
        // This is inherent to the design and accepted for a data-mining course
        // where such answers do not arise.
        assert_eq!(normalise("C++"), normalise("C"));
    }
}
