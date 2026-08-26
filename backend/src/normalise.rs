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
}
