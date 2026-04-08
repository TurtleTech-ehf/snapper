use regex::Regex;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

/// Matches segments ending with sentence punctuation followed by closing quotes/parens,
/// where the punctuation is not a true sentence boundary (e.g., `"wow!" and`, `(emphasis!) loudly`).
static QUOTED_PUNCT_END_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"[.!?]["')\]]+\s*$"##).expect("valid quoted-punct regex")
});

use crate::abbreviations;
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
            r#"https?://\S+[^.\s!?,;:)\]'""]"#, // URLs (don't swallow trailing punctuation)
            r"file:\S+",                 // Org file: links
            r"@@[a-zA-Z]+:[^@]*@@",     // Org inline export snippets: @@backend:value@@
        ]
        .join("|"),
    )
    .expect("valid inline token regex")
});

// Static patterns removed -- now compiled per-instance in UnicodeSentenceSplitter::for_lang().

/// Sentence splitter using Unicode UAX #29 with abbreviation-aware merging.
pub struct UnicodeSentenceSplitter {
    /// Compiled regex for extra user-provided abbreviations, if any.
    extra_pattern: Option<Regex>,
    /// Compiled abbreviation pattern for the selected language.
    lang_abbrev_pattern: Regex,
    /// Compiled multi-abbreviation pattern for the selected language.
    lang_multi_pattern: Regex,
}

impl UnicodeSentenceSplitter {
    /// Create a splitter with only built-in English abbreviations.
    pub fn new() -> Self {
        Self::for_lang("en", &[])
    }

    /// Create a splitter with additional user-provided abbreviations.
    pub fn with_extra_abbreviations(extras: &[String]) -> Self {
        Self::for_lang("en", extras)
    }

    /// Create a splitter for a specific language, optionally with extra abbreviations.
    pub fn for_lang(lang: &str, extras: &[String]) -> Self {
        let abbrevs = abbreviations::abbreviations_for_lang(lang);
        let multi = abbreviations::multi_abbrevs_for_lang(lang);

        let alts: Vec<&str> = abbrevs.to_vec();
        let pattern = format!(r#"(?:^|[\s"'`(\[])(?:{})$"#, alts.join("|"));
        let lang_abbrev_pattern = Regex::new(&pattern).expect("valid abbreviation regex");

        let multi_alts: Vec<String> = multi.iter().map(|a| regex::escape(a)).collect();
        let multi_pattern = format!(r"(?:^|\s)(?:{})$", multi_alts.join("|"));
        let lang_multi_pattern =
            Regex::new(&multi_pattern).expect("valid multi-abbreviation regex");

        let extra_pattern = if extras.is_empty() {
            None
        } else {
            let alts: Vec<String> = extras.iter().map(|a| regex::escape(a)).collect();
            let pattern = format!(r"(?:^|\s)(?:{})$", alts.join("|"));
            Some(Regex::new(&pattern).expect("valid extra abbreviation regex"))
        };

        Self {
            extra_pattern,
            lang_abbrev_pattern,
            lang_multi_pattern,
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

        let merged = merge_abbreviation_splits(
            &raw_segments,
            &self.lang_abbrev_pattern,
            &self.lang_multi_pattern,
            self.extra_pattern.as_ref(),
        );

        // Merge false splits from punctuation inside quotes/parens
        let merged = merge_quoted_punct_splits(merged);

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

fn merge_abbreviation_splits(
    segments: &[&str],
    abbrev_re: &Regex,
    multi_re: &Regex,
    extra: Option<&Regex>,
) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(segments.len());

    for &segment in segments {
        let should_merge = if let Some(prev) = result.last() {
            is_abbreviation_ending(prev, abbrev_re, multi_re, extra)
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

/// Merge false splits caused by sentence punctuation inside quotes or parens.
/// E.g., `He said "wow!"` + `and left.` should stay as one sentence when
/// the next segment starts with a lowercase letter.
fn merge_quoted_punct_splits(segments: Vec<String>) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(segments.len());

    for segment in segments {
        let should_merge = if let Some(prev) = result.last() {
            // Previous segment ends with punctuation + closing quote/paren
            QUOTED_PUNCT_END_RE.is_match(prev.trim_end())
                // Next segment starts with lowercase (continuation, not new sentence)
                && segment
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_lowercase())
        } else {
            false
        };

        if should_merge {
            let prev = result.last_mut().unwrap();
            prev.push_str(&segment);
        } else {
            result.push(segment);
        }
    }

    result
}

fn is_abbreviation_ending(
    s: &str,
    abbrev_re: &Regex,
    multi_re: &Regex,
    extra: Option<&Regex>,
) -> bool {
    let trimmed = s.trim_end();
    if !trimmed.ends_with('.') {
        return false;
    }
    let before_dot = &trimmed[..trimmed.len() - 1];

    if abbrev_re.is_match(before_dot) {
        return true;
    }

    if multi_re.is_match(before_dot) {
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

    #[test]
    fn quoted_exclamation_no_false_split() {
        assert_eq!(
            split(r#"He said "wow!" and left. She agreed."#),
            vec![r#"He said "wow!" and left."#, "She agreed."]
        );
    }

    #[test]
    fn paren_exclamation_no_false_split() {
        assert_eq!(
            split("He replied (with emphasis!) loudly. She agreed."),
            vec!["He replied (with emphasis!) loudly.", "She agreed."]
        );
    }

    #[test]
    fn paren_question_no_false_split() {
        assert_eq!(
            split("The answer (really?) surprised them. Next sentence."),
            vec![
                "The answer (really?) surprised them.",
                "Next sentence."
            ]
        );
    }

    #[test]
    fn url_trailing_period_not_swallowed() {
        assert_eq!(
            split("Visit https://example.com/path. Then read more."),
            vec!["Visit https://example.com/path.", "Then read more."]
        );
    }

    #[test]
    fn url_with_query_trailing_period() {
        assert_eq!(
            split("See https://example.com/path?q=1&r=2. Next sentence."),
            vec![
                "See https://example.com/path?q=1&r=2.",
                "Next sentence."
            ]
        );
    }

    #[test]
    fn ellipsis_splits() {
        assert_eq!(
            split("Sentence one... Sentence two."),
            vec!["Sentence one...", "Sentence two."]
        );
    }

    #[test]
    fn quoted_period_end_of_sentence() {
        // "done." followed by uppercase Start is a real sentence boundary
        assert_eq!(
            split(r#"End of quote: "done." Start again."#),
            vec![r#"End of quote: "done.""#, "Start again."]
        );
    }
}
