use regex::Regex;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

use crate::abbreviations::{ABBREVIATIONS, MULTI_ABBREVS};
use crate::sentence::SentenceSplitter;

/// Patterns for inline tokens that should not be split across sentences.
/// These get replaced with safe placeholders before sentence detection.
static INLINE_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        &[
            r"\[\[[^\]]*\]\]",           // Org links: [[url]] or [[url][desc]]
            r"\[\[[^\]]*\]\[[^\]]*\]\]", // Org links with desc
            r"\[[^\]]+\]\([^)]+\)",      // Markdown links: [text](url)
            r"!\[[^\]]*\]\([^)]+\)",     // Markdown images: ![alt](url)
            r"\$[^$]+\$",                // Inline math: $...$
            r"\\([a-zA-Z]+)\{[^}]*\}",   // LaTeX commands: \cmd{arg}
            r"~[^~]+~",                  // Org inline code: ~code~
            r"=[^=]+=",                  // Org verbatim: =text=
            r"`[^`]+`",                  // Markdown inline code: `code`
        ]
        .join("|"),
    )
    .expect("valid inline token regex")
});

static ABBREV_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    let alts = ABBREVIATIONS.to_vec();
    // Allow quotes, parens, brackets before the abbreviation (not just whitespace)
    let pattern = format!(r#"(?:^|[\s"'`(\[])(?:{})$"#, alts.join("|"));
    Regex::new(&pattern).expect("valid abbreviation regex")
});

static MULTI_ABBREV_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    let alts: Vec<String> = MULTI_ABBREVS.iter().map(|a| regex::escape(a)).collect();
    let pattern = format!(r"(?:^|\s)(?:{})$", alts.join("|"));
    Regex::new(&pattern).expect("valid multi-abbreviation regex")
});

/// Sentence splitter using Unicode UAX #29 with abbreviation-aware merging.
pub struct UnicodeSentenceSplitter {
    /// Compiled regex for extra user-provided abbreviations, if any.
    extra_pattern: Option<Regex>,
}

impl UnicodeSentenceSplitter {
    /// Create a splitter with only built-in abbreviations.
    pub fn new() -> Self {
        Self {
            extra_pattern: None,
        }
    }

    /// Create a splitter with additional user-provided abbreviations.
    pub fn with_extra_abbreviations(extras: &[String]) -> Self {
        if extras.is_empty() {
            return Self::new();
        }
        let alts: Vec<String> = extras.iter().map(|a| regex::escape(a)).collect();
        let pattern = format!(r"(?:^|\s)(?:{})$", alts.join("|"));
        Self {
            extra_pattern: Some(Regex::new(&pattern).expect("valid extra abbreviation regex")),
        }
    }
}

impl Default for UnicodeSentenceSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl SentenceSplitter for UnicodeSentenceSplitter {
    fn split(&self, text: &str) -> Vec<String> {
        let text = text.trim();
        if text.is_empty() {
            return vec![];
        }

        // Replace inline tokens with safe placeholders to prevent
        // the sentence splitter from breaking inside them.
        let mut placeholders: Vec<String> = Vec::new();
        let protected = INLINE_TOKEN_RE.replace_all(text, |caps: &regex::Captures| {
            let idx = placeholders.len();
            placeholders.push(caps[0].to_string());
            // Use a placeholder that won't trigger sentence breaks
            format!("\x00PH{idx}\x00")
        });

        let raw_segments: Vec<&str> = protected.unicode_sentences().collect();

        if raw_segments.is_empty() {
            return vec![text.to_string()];
        }

        let merged = merge_abbreviation_splits(&raw_segments, self.extra_pattern.as_ref());

        // Restore placeholders and clean up
        merged
            .into_iter()
            .map(|s| {
                let mut restored = s.trim().to_string();
                for (i, original) in placeholders.iter().enumerate() {
                    let ph = format!("\x00PH{i}\x00");
                    restored = restored.replace(&ph, original);
                }
                restored
            })
            .filter(|s| !s.is_empty())
            .collect()
    }
}

fn merge_abbreviation_splits(segments: &[&str], extra: Option<&Regex>) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(segments.len());

    for &segment in segments {
        let should_merge = if let Some(prev) = result.last() {
            is_abbreviation_ending(prev, extra)
        } else {
            false
        };

        if should_merge {
            let prev = result.last_mut().unwrap();
            prev.push_str(segment);
        } else {
            result.push(segment.to_string());
        }
    }

    result
}

fn is_abbreviation_ending(s: &str, extra: Option<&Regex>) -> bool {
    let trimmed = s.trim_end();
    if !trimmed.ends_with('.') {
        return false;
    }
    let before_dot = &trimmed[..trimmed.len() - 1];

    if ABBREV_PATTERN.is_match(before_dot) {
        return true;
    }

    if MULTI_ABBREV_PATTERN.is_match(before_dot) {
        return true;
    }

    if let Some(re) = extra {
        if re.is_match(before_dot) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<String> {
        UnicodeSentenceSplitter::new().split(text)
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

    #[test]
    fn extra_abbreviations() {
        // "Abstr" is not a built-in abbreviation, so the default splitter
        // would break at "Abstr." The extra list prevents that.
        let splitter = UnicodeSentenceSplitter::with_extra_abbreviations(&[
            "Abstr".to_string(),
            "Suppl".to_string(),
        ]);
        assert_eq!(
            splitter.split("See Abstr. 5 for details. The results follow."),
            vec!["See Abstr. 5 for details.", "The results follow."]
        );
        // Without extra, "Abstr." would cause a false break:
        let default = UnicodeSentenceSplitter::new();
        let result = default.split("See Abstr. 5 for details. The results follow.");
        // Default splits at "Abstr." since it doesn't know the abbreviation
        assert!(result.len() > 1);
    }

    #[test]
    fn inline_org_link_preserved() {
        assert_eq!(
            split("See [[https://example.com][Ex. Site]] for details. Then continue."),
            vec![
                "See [[https://example.com][Ex. Site]] for details.",
                "Then continue."
            ]
        );
    }

    #[test]
    fn inline_math_preserved() {
        assert_eq!(
            split("The value $x = 3.14$ matters. Next sentence."),
            vec!["The value $x = 3.14$ matters.", "Next sentence."]
        );
    }

    #[test]
    fn inline_markdown_link_preserved() {
        assert_eq!(
            split("Visit [Example Inc.](https://example.com) now. Then read more."),
            vec![
                "Visit [Example Inc.](https://example.com) now.",
                "Then read more."
            ]
        );
    }

    #[test]
    fn inline_code_preserved() {
        assert_eq!(
            split("Use `std.io.Read` for input. Then process."),
            vec!["Use `std.io.Read` for input.", "Then process."]
        );
    }
}
