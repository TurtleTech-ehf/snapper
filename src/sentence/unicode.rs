use regex::Regex;
use std::sync::LazyLock;

/// Matches segments ending with sentence punctuation followed by closing quotes/parens,
/// where the punctuation is not a true sentence boundary (e.g., `"wow!" and`, `(emphasis!) loudly`).
static QUOTED_PUNCT_END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r##"[.!?]["')\]]+\s*$"##).expect("valid quoted-punct regex"));

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
            // Org emphasis must be protected before sentence splits so a line
            // cannot begin with `*rest` (false headline) or leave markers open.
            // Org requires a non-space immediately after the opener and before
            // the closer; content may include spaces and sentence punctuation.
            // (Rust `regex` has no lookbehind; encode the border as char classes.)
            r"\*[^*\s\n](?:[^*\n]*[^*\s\n])?\*", // Org bold: *text*
            r"/[^/\s\n](?:[^/\n]*[^/\s\n])?/",   // Org italic: /text/
            r"_[^_\s\n](?:[^_\n]*[^_\s\n])?_",   // Org underline: _text_
            r"\+[^\+\s\n](?:[^\+\n]*[^\+\s\n])?\+", // Org strike-through: +text+
            r"~[^~\n]+~",                        // Org inline code: ~code~
            r"=[^=\n]+=",                        // Org verbatim: =text=
            r"`[^`\n]+`",                        // Markdown inline code: `code`
            r#"https?://\S+[^.\s!?,;:)\]'""]"#,  // URLs (don't swallow trailing punctuation)
            r"file:\S+",                         // Org file: links
            r"@@[a-zA-Z]+:[^@]*@@",              // Org inline export snippets: @@backend:value@@
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

        // UAX #29 sentence bounds. `unicode_sentences()` filters
        // whitespace-only segments but also drops trailing closing
        // punctuation like `>` after a sentence-terminating `.`, which
        // clips inputs such as `Vec<...>` or `<a.>` at end-of-prose.
        // We re-collect from the unfiltered iterator and merge any
        // non-sentence tail back onto the preceding sentence.
        let raw_segments: Vec<&str> = merge_tail_punctuation(&protected);

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
        // Rejoin segments while a delimited span is still open so we do not
        // break on `.` / `?` / `!` + capital *inside* quotes or brackets,
        // while still allowing a real boundary after the closer.
        let merged = merge_splits_inside_delimiters(merged);

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

/// Walk the UAX #29 sentence bounds and merge any trailing non-sentence
/// segments back onto the preceding sentence. Without this glue, a prose
/// region ending in characters like `>` after a sentence-terminating `.`
/// would lose those characters: `Vec<...>` becomes `Vec<...`. The standard
/// `unicode_sentences()` filter silently discards such tails because they
/// contain no letter/digit/quote.
///
/// We never *split* further than the bounds iterator does; we only merge
/// adjacent fragments where one is a real sentence and its neighbour is
/// content-free (no alphanumeric characters). This mirrors the existing
/// `unicode_sentences()` filter rule but reattaches the tail rather than
/// dropping it.
fn merge_tail_punctuation(text: &str) -> Vec<&str> {
    use unicode_segmentation::UnicodeSegmentation;

    fn has_content(s: &str) -> bool {
        s.chars().any(|c| c.is_alphanumeric())
    }

    let bounds: Vec<&str> = text.split_sentence_bounds().collect();
    if bounds.is_empty() {
        return Vec::new();
    }

    // Build a merged Vec<&str> by walking left to right and re-slicing the
    // original `text` so we return `&str`s. The slice boundaries align
    // because `split_sentence_bounds` returns adjacent subslices.
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(bounds.len());
    let mut cursor: usize = 0;
    for seg in &bounds {
        let start = cursor;
        let end = cursor + seg.len();
        if has_content(seg) {
            merged.push((start, end));
        } else if let Some(last) = merged.last_mut() {
            // Glue onto the previous sentence.
            last.1 = end;
        } else {
            // Leading whitespace/punctuation only: preserve as a segment;
            // the downstream pipeline trims it.
            merged.push((start, end));
        }
        cursor = end;
    }

    merged.into_iter().map(|(s, e)| &text[s..e]).collect()
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

/// Rejoin UAX segments while any “span” is still open: ASCII/curly/guillemet
/// quotes, LaTeX ```` / `''` style quotes, and balanced `()` / `[]` / `{}`.
/// Single quotes alone are ignored (apostrophes in `don't`). Escaped `\"`
/// does not toggle ASCII double-quote state.
fn merge_splits_inside_delimiters(segments: Vec<String>) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(segments.len());
    let mut state = DelimState::default();

    for segment in segments {
        if state.is_inside() {
            if let Some(last) = result.last_mut() {
                last.push_str(&segment);
            } else {
                result.push(segment.clone());
            }
        } else {
            result.push(segment.clone());
        }
        state.feed(&segment);
    }

    result
}

#[derive(Default)]
struct DelimState {
    ascii_double_open: bool,
    curly_depth: i32,
    guillemet_depth: i32,
    latex_quote_depth: i32,
    paren_depth: i32,
    bracket_depth: i32,
    brace_depth: i32,
}

impl DelimState {
    fn is_inside(&self) -> bool {
        self.ascii_double_open
            || self.curly_depth > 0
            || self.guillemet_depth > 0
            || self.latex_quote_depth > 0
            || self.paren_depth > 0
            || self.bracket_depth > 0
            || self.brace_depth > 0
    }

    fn feed(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            let next = chars.get(i + 1).copied();

            // LaTeX-style open `` and close '' (ASCII backticks/apostrophes).
            if ch == '`' && next == Some('`') {
                self.latex_quote_depth += 1;
                i += 2;
                continue;
            }
            if ch == '\'' && next == Some('\'') {
                self.latex_quote_depth = (self.latex_quote_depth - 1).max(0);
                i += 2;
                continue;
            }

            // Escaped ASCII double quote does not toggle.
            if ch == '\\' && next == Some('"') {
                i += 2;
                continue;
            }

            match ch {
                '"' => self.ascii_double_open = !self.ascii_double_open,
                '\u{201C}' => self.curly_depth += 1,
                '\u{201D}' => self.curly_depth = (self.curly_depth - 1).max(0),
                '\u{00AB}' => self.guillemet_depth += 1,
                '\u{00BB}' => self.guillemet_depth = (self.guillemet_depth - 1).max(0),
                '(' => self.paren_depth += 1,
                ')' => self.paren_depth = (self.paren_depth - 1).max(0),
                '[' => self.bracket_depth += 1,
                ']' => self.bracket_depth = (self.bracket_depth - 1).max(0),
                // Avoid treating LaTeX `\}` style escapes as braces when
                // preceded by backslash; still count raw `{` / `}`.
                '{' if i == 0 || chars[i - 1] != '\\' => self.brace_depth += 1,
                '}' if i == 0 || chars[i - 1] != '\\' => {
                    self.brace_depth = (self.brace_depth - 1).max(0);
                }
                _ => {}
            }
            i += 1;
        }
    }
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
    fn org_bold_with_internal_period_not_split() {
        // Splitting would leave a line starting with `*Bold...` (false headline).
        assert_eq!(
            split("End of first. *Bold spans period. Continues* after."),
            vec!["End of first.", "*Bold spans period. Continues* after."]
        );
    }

    #[test]
    fn org_italic_with_internal_period_not_split() {
        assert_eq!(
            split("Lead-in. /Italic has a period. Still italic/ trail."),
            vec!["Lead-in.", "/Italic has a period. Still italic/ trail."]
        );
    }

    #[test]
    fn angle_bracket_tail_after_period_preserved() {
        // UAX #29 can drop a lone `>` after `.` without merge_tail_punctuation.
        assert_eq!(
            split("snapshot field is Box[T], not Vec[T]"),
            vec!["snapshot field is Box[T], not Vec[T]"]
        );
        assert_eq!(split("see <a.>"), vec!["see <a.>"]);
    }

    #[test]
    fn double_quoted_span_with_internal_period_not_split() {
        assert_eq!(
            split(r#"He said "Hello world. How are you?" Then he left."#),
            vec![r#"He said "Hello world. How are you?""#, "Then he left."]
        );
    }

    #[test]
    fn curly_double_quoted_span_with_internal_period_not_split() {
        assert_eq!(
            split("He said \u{201C}Hello world. How are you?\u{201D} Then he left."),
            vec![
                "He said \u{201C}Hello world. How are you?\u{201D}",
                "Then he left."
            ]
        );
    }

    #[test]
    fn quoted_title_with_abbrev_stays_one_sentence() {
        assert_eq!(
            split(r#"See the note "Fig. 3 is wrong." in the appendix."#),
            vec![r#"See the note "Fig. 3 is wrong." in the appendix."#]
        );
    }

    #[test]
    fn plaintext_format_keeps_dialogue_quote_together() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "He said \"Hello world. How are you?\" Then he left.\n";
        let cfg = FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("world.\nHow"),
            "must not break inside ASCII double quotes, got:\n{out}"
        );
        assert!(
            out.contains("you?\"\nThen") || out.contains("you?\" Then"),
            "may break after closing quote; got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn paren_span_with_internal_period_capital_not_split() {
        assert_eq!(
            split("See (Fig. 3 is wrong. Really.) Next."),
            vec!["See (Fig. 3 is wrong. Really.)", "Next."]
        );
    }

    #[test]
    fn bracket_span_with_internal_period_not_split() {
        assert_eq!(
            split("See [note. One] more."),
            vec!["See [note. One] more."]
        );
    }

    #[test]
    fn latex_style_quotes_with_internal_period_not_split() {
        assert_eq!(
            split("He said ``Hello world. How?'' Then."),
            vec!["He said ``Hello world. How?''", "Then."]
        );
    }

    #[test]
    fn escaped_ascii_quote_does_not_toggle_early() {
        // Backslash-escaped quotes are common in code-ish plaintext; do not
        // treat `\"` as ending the outer dialogue span.
        let out = split(r#"She said "He said \"no.\" Then left." Done."#);
        assert_eq!(out.len(), 2, "got {out:?}");
        assert!(
            out[0].contains(r#"\"no.\""#) || out[0].contains("no."),
            "{out:?}"
        );
        assert_eq!(out[1], "Done.");
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
            vec!["The answer (really?) surprised them.", "Next sentence."]
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
            vec!["See https://example.com/path?q=1&r=2.", "Next sentence."]
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
