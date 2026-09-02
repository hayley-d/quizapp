use serde::{Deserialize, Serialize};

pub const TRANSFER_FORMAT: &str = "quizapp-transfer";
pub const TRANSFER_FORMAT_VERSION: i64 = 1;
pub const MAX_TRANSFER_BYTES: usize = 32 * 1024 * 1024;

const MAXIMUM_SLUG_LENGTH: usize = 60;
const FALLBACK_SLUG: &str = "deck";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferFile {
    pub format: String,
    pub format_version: i64,
    #[serde(default)]
    pub exported_at: Option<String>,
    pub decks: Vec<TransferDeck>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferDeck {
    #[serde(default)]
    pub module_name: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cards: Vec<TransferCard>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferCard {
    pub kind: String,
    pub prompt_md: String,
    #[serde(default)]
    pub answer_md: Option<String>,
    #[serde(default)]
    pub explanation_md: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub image_base64: Option<String>,
    #[serde(default)]
    pub choices: Vec<TransferChoice>,
    #[serde(default)]
    pub accepted: Vec<TransferAccepted>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferChoice {
    pub text_md: String,
    #[serde(default)]
    pub is_correct: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferAccepted {
    pub text: String,
    #[serde(default)]
    pub is_primary: bool,
}

pub fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());

    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.extend(character.to_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }

    slug.truncate(MAXIMUM_SLUG_LENGTH);
    let trimmed = slug.trim_end_matches('-');

    if trimmed.is_empty() {
        FALLBACK_SLUG.to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn attachment_filename(name: &str) -> String {
    format!("{}.quizapp.json", slugify(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_name_becomes_lowercase_and_hyphenated() {
        assert_eq!(slugify("Beta Blockers"), "beta-blockers");
    }

    #[test]
    fn punctuation_and_latex_collapse_to_single_hyphens() {
        assert_eq!(slugify("Cardiology: $\\beta_1$ ... agonists!"), "cardiology-beta-1-agonists");
    }

    #[test]
    fn non_ascii_characters_never_survive_into_the_slug() {
        let slug = slugify("Pharmacologie générale — β blockers");
        assert!(
            slug.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
            "slug must be header-safe ascii, got {slug}",
        );
        assert_eq!(slug, "pharmacologie-g-n-rale-blockers");
    }

    #[test]
    fn a_name_with_nothing_usable_falls_back() {
        assert_eq!(slugify(""), FALLBACK_SLUG);
        assert_eq!(slugify("   "), FALLBACK_SLUG);
        assert_eq!(slugify("— ??? —"), FALLBACK_SLUG);
    }

    #[test]
    fn leading_and_trailing_separators_are_dropped() {
        assert_eq!(slugify("  ...Neurology...  "), "neurology");
    }

    #[test]
    fn a_long_name_is_truncated_without_a_trailing_hyphen() {
        let slug = slugify(&"word ".repeat(40));
        assert!(slug.len() <= MAXIMUM_SLUG_LENGTH);
        assert!(!slug.ends_with('-'), "truncation must not leave a dangling hyphen: {slug}");
    }

    #[test]
    fn the_filename_carries_the_double_extension() {
        assert_eq!(attachment_filename("Beta Blockers"), "beta-blockers.quizapp.json");
    }
}
