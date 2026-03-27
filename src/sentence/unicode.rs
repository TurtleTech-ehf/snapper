use regex::Regex;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

use crate::abbreviations::{ABBREVIATIONS, MULTI_ABBREVS};
use crate::sentence::SentenceSplitter;

static ABBREV_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Build a regex that matches "Abbrev. " at the end of a segment
    // where Abbrev is one of our known abbreviations.
    let alts = ABBREVIATIONS.to_vec();
    let pattern = format!(r"(?:^|\s)(?:{})$", alts.join("|"));
    Regex::new(&pattern).expect("valid abbreviation regex")
});

static MULTI_ABBREV_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    let alts: Vec<String> = MULTI_ABBREVS.iter().map(|a| regex::escape(a)).collect();
    let pattern = format!(r"(?:^|\s)(?:{})$", alts.join("|"));
    Regex::new(&pattern).expect("valid multi-abbreviation regex")
});

/// Sentence splitter using Unicode UAX #29 with abbreviation-aware merging.
pub struct UnicodeSentenceSplitter;

impl SentenceSplitter for UnicodeSentenceSplitter {
    fn split(&self, text: &str) -> Vec<String> {
        let text = text.trim();
        if text.is_empty() {
            return vec![];
        }

        // Get raw Unicode sentence segments
        let raw_segments: Vec<&str> = text.unicode_sentences().collect();

        if raw_segments.is_empty() {
            return vec![text.to_string()];
        }

        // Merge segments that were split at abbreviation boundaries
        let merged = merge_abbreviation_splits(&raw_segments);

        // Clean up whitespace in each sentence
        merged
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// Merge adjacent segments where the split was caused by an abbreviation period.
fn merge_abbreviation_splits(segments: &[&str]) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(segments.len());

    for &segment in segments {
        let should_merge = if let Some(prev) = result.last() {
            is_abbreviation_ending(prev)
        } else {
            false
        };

        if should_merge {
            // Merge with previous segment
            let prev = result.last_mut().unwrap();
            prev.push_str(segment);
        } else {
            result.push(segment.to_string());
        }
    }

    result
}

/// Check if a string ends with a known abbreviation followed by a period.
fn is_abbreviation_ending(s: &str) -> bool {
    let trimmed = s.trim_end();
    // Must end with a period
    if !trimmed.ends_with('.') {
        return false;
    }
    // Get the part before the period
    let before_dot = &trimmed[..trimmed.len() - 1];

    // Check single-word abbreviations
    if ABBREV_PATTERN.is_match(before_dot) {
        return true;
    }

    // Check multi-word abbreviations (e.g., i.e.)
    if MULTI_ABBREV_PATTERN.is_match(before_dot) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<String> {
        UnicodeSentenceSplitter.split(text)
    }

    #[test]
    fn simple_sentences() {
        assert_eq!(
            split("Hello world. This is a test. Another sentence here."),
            vec!["Hello world.", "This is a test.", "Another sentence here."]
        );
    }

    #[test]
    fn abbreviation_dr() {
        assert_eq!(
            split("Dr. Smith went home. He was tired."),
            vec!["Dr. Smith went home.", "He was tired."]
        );
    }

    #[test]
    fn abbreviation_eg() {
        assert_eq!(
            split("Use a formatter, e.g. snapper. It works well."),
            vec!["Use a formatter, e.g. snapper.", "It works well."]
        );
    }

    #[test]
    fn abbreviation_fig() {
        assert_eq!(
            split("See Fig. 3 for details. The results are clear."),
            vec!["See Fig. 3 for details.", "The results are clear."]
        );
    }

    #[test]
    fn empty_input() {
        assert_eq!(split(""), Vec::<String>::new());
    }

    #[test]
    fn single_sentence() {
        assert_eq!(split("Just one sentence."), vec!["Just one sentence."]);
    }

    #[test]
    fn question_and_exclamation() {
        assert_eq!(
            split("Is this working? Yes! It is."),
            vec!["Is this working?", "Yes!", "It is."]
        );
    }

    #[test]
    fn no_trailing_period() {
        assert_eq!(
            split("First sentence. Second without period"),
            vec!["First sentence.", "Second without period"]
        );
    }
}
