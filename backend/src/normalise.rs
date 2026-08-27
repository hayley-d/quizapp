use unicode_normalization::UnicodeNormalization;

pub fn normalise(input: &str) -> String {
    let folded: String = input.nfkc().flat_map(char::to_lowercase).collect();

    let spaced: String = folded
        .chars()
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
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
        assert_ne!(normalise("a-b"), normalise("ab"));
        assert_ne!(normalise("1,000"), normalise("1000"));
        assert_ne!(normalise("F1"), normalise("F 1"));
    }

    #[test]
    fn trailing_symbols_vanish_a_known_limitation() {
        assert_eq!(normalise("C++"), normalise("C"));
    }
}
