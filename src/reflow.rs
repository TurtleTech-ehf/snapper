use std::collections::HashMap;

use crate::config::CodeLang;
use crate::parser::Region;
use crate::sentence::SentenceSplitter;

/// Configuration for the reflow engine.
#[derive(Default)]
pub struct ReflowConfig<'a> {
    /// Maximum line width. 0 means unlimited.
    pub max_width: usize,
    /// Per-language code-block configuration (borrowed from `FormatConfig`).
    pub code: Option<&'a HashMap<String, CodeLang>>,
    /// When `true`, the per-language `formatter` runs after comment reflow.
    pub format_code: bool,
    /// Prefer soft breaks after independent-clause punctuation (`,`, `;`,
    /// `:`, em dash, `--`) when wrapping under `max_width`.
    pub clause_breaks: bool,
}

/// Minimum region count before parallelizing reflow (large multi-MB org/md files).
#[cfg(feature = "cli")]
const PARALLEL_REGION_THRESHOLD: usize = 32;

/// Reflow a sequence of regions, applying sentence breaks to Prose regions.
///
/// With the `cli` feature, files that parse into many regions (typical large
/// Org/Markdown trees) reflow independent regions in parallel via rayon, then
/// concatenate in order.
pub fn reflow(
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    #[cfg(feature = "cli")]
    {
        if regions.len() >= PARALLEL_REGION_THRESHOLD {
            return reflow_parallel(regions, splitter, config);
        }
    }
    reflow_sequential(regions, splitter, config)
}

fn reflow_sequential(
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    let mut output = String::new();
    for (idx, region) in regions.iter().enumerate() {
        output.push_str(&reflow_one(region, idx, regions, splitter, config));
    }
    output
}

#[cfg(feature = "cli")]
fn reflow_parallel(
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    use rayon::prelude::*;
    // Indexed parallel map preserves order on collect.
    let parts: Vec<String> = regions
        .par_iter()
        .enumerate()
        .map(|(idx, region)| reflow_one(region, idx, regions, splitter, config))
        .collect();
    let mut output = String::new();
    for p in parts {
        output.push_str(&p);
    }
    output
}

fn reflow_one(
    region: &Region,
    idx: usize,
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    let mut output = String::new();
    match region {
        Region::Structure(s) => output.push_str(s),
        Region::BlankLines(s) => output.push_str(s),
        Region::Code {
            lang,
            header,
            body,
            footer,
        } => {
            output.push_str(header);
            let code_cfg = lang
                .as_deref()
                .and_then(|l| config.code.and_then(|m| m.get(l)));
            let reflowed = if let Some(cfg) = code_cfg {
                crate::code_block::reflow_code_body(body, cfg, splitter, config.format_code)
            } else {
                body.clone()
            };
            output.push_str(&reflowed);
            output.push_str(footer);
        }
        Region::Prose(text) => {
            let sentences = splitter.split(text);
            for (i, sentence) in sentences.iter().enumerate() {
                if config.max_width > 0 {
                    let wrapped = if config.clause_breaks {
                        wrap_with_clause_breaks(sentence, config.max_width)
                    } else {
                        textwrap::fill(sentence, config.max_width)
                    };
                    output.push_str(&wrapped);
                } else {
                    output.push_str(sentence);
                }
                if i < sentences.len() - 1 {
                    output.push('\n');
                }
            }
            if !sentences.is_empty() {
                // No forced paragraph break before inline islands (math/code) or
                // tight punctuation structures — those continue the same line.
                let suppress = matches!(
                    regions.get(idx + 1),
                    Some(Region::Structure(s)) if suppress_prose_trailing_newline(s)
                );
                if !suppress {
                    output.push('\n');
                }
            }
        }
    }
    output
}

/// True when `word` ends with independent-clause punctuation (sembr rule 5),
/// ignoring trailing closing quotes and brackets. Words come from whitespace
/// splitting, so a match always marks a lossless break site: the punctuation
/// is followed by real whitespace in the source.
fn ends_with_clause_punct(word: &str) -> bool {
    let core = word.trim_end_matches(['"', '\'', ')', ']', '}']);
    core.ends_with(',')
        || core.ends_with(';')
        || core.ends_with(':')
        || core.ends_with('\u{2014}') // em dash —
        || core.ends_with("--")
}

/// Wrap `sentence` under `max_width`, preferring breaks after clause
/// punctuation (sembr rule 5). A sentence that already fits stays on one
/// line. Breaks only ever land at whitespace, so tokens like `1,000`,
/// `10:30`, URLs, and `--flags` are never split apart.
pub fn wrap_with_clause_breaks(sentence: &str, max_width: usize) -> String {
    if max_width == 0 {
        return sentence.to_string();
    }
    wrap_words_preferring_clause(sentence, max_width).join("\n")
}

/// Greedy word wrap that, when forced to break, prefers the last word on the
/// line that ends with clause punctuation; otherwise breaks at the last word
/// boundary that fits.
fn wrap_words_preferring_clause(text: &str, max_width: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let mut end = start;
        let mut line_len = 0usize;
        while end < words.len() {
            let wlen = words[end].chars().count();
            let next_len = if end == start {
                wlen
            } else {
                line_len + 1 + wlen
            };
            if next_len > max_width && end > start {
                break;
            }
            line_len = next_len;
            end += 1;
            // Single overlong word: take it alone
            if end == start + 1 && line_len > max_width {
                break;
            }
        }
        // Only a forced break gets pulled back to a clause boundary; the
        // final line of a sentence keeps its remaining words together.
        let mut break_at = end;
        if end < words.len() {
            for j in (start..end).rev() {
                if ends_with_clause_punct(words[j]) {
                    break_at = j + 1;
                    break;
                }
            }
        }
        lines.push(words[start..break_at].join(" "));
        start = break_at;
    }
    lines
}

/// When the next region is an inline structure island (pandoc `Math`/`Code` as
/// Structure), do not end the preceding prose with a hard line break.
fn suppress_prose_trailing_newline(s: &str) -> bool {
    if s == "\n" || s.starts_with('}') || s.starts_with(']') || s.starts_with(')') {
        return true;
    }
    // Islands may carry a leading space for glue after reflow trims prose.
    let t = s.trim();
    // Inline math: single-line `$...$` (not display `$$...$$`).
    if t.starts_with('$') && !t.starts_with("$$") && !t.contains('\n') {
        return true;
    }
    // Inline code island: single-line `...` (optional trailing space already trimmed).
    let code = t.trim_end_matches(' ');
    if code.starts_with('`') && code.ends_with('`') && code.len() >= 2 && !code.contains('\n') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::unicode::UnicodeSentenceSplitter;

    fn reflow_text(input: &str) -> String {
        let regions = vec![Region::Prose(input.to_string())];
        let config = ReflowConfig::default();
        reflow(&regions, &UnicodeSentenceSplitter::new(), &config)
    }

    #[test]
    fn simple_reflow() {
        let result = reflow_text("Hello world. This is a test. Another sentence.");
        assert_eq!(result, "Hello world.\nThis is a test.\nAnother sentence.\n");
    }

    #[test]
    fn idempotent() {
        let input = "Hello world.\nThis is a test.\nAnother sentence.";
        let first = reflow_text(input);
        let second = reflow_text(&first);
        assert_eq!(first, second, "reflow must be idempotent");
    }

    #[test]
    fn preserves_structure() {
        let regions = vec![
            Region::Structure("#+TITLE: Test\n".to_string()),
            Region::BlankLines("\n".to_string()),
            Region::Prose("First sentence. Second sentence.".to_string()),
        ];
        let config = ReflowConfig::default();
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        assert_eq!(
            result,
            "#+TITLE: Test\n\nFirst sentence.\nSecond sentence.\n"
        );
    }

    #[test]
    fn max_width_wrapping() {
        let regions = vec![Region::Prose(
            "This is a very long sentence that should be wrapped at a reasonable width for readability in narrow terminals.".to_string(),
        )];
        let config = ReflowConfig {
            max_width: 40,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        // Every line should be <= 40 chars
        for line in result.lines() {
            assert!(
                line.len() <= 40,
                "Line too long: {} chars: {:?}",
                line.len(),
                line
            );
        }
    }

    #[test]
    fn clause_breaks_prefer_commas_under_max_width() {
        // Issue #7 sample: max_width=80 with clause breaks should land soft
        // breaks after the independent-clause commas rather than packing
        // mid-phrase as plain textwrap::fill does.
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let wrapped = wrap_with_clause_breaks(sentence, 80);
        let expected = "\
It contains rules which govern how the Objectives are orchestrated,
along with rules which can automatically activate the Objectives in the plan,
without additional human intervention.";
        assert_eq!(
            wrapped, expected,
            "clause-first wrap:\n--- got ---\n{wrapped}\n--- expected ---\n{expected}"
        );
        for line in wrapped.lines() {
            assert!(
                line.chars().count() <= 80,
                "line exceeds max_width: {line:?}"
            );
        }
    }

    #[test]
    fn clause_breaks_off_matches_textwrap_fill() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let regions = vec![Region::Prose(sentence.to_string())];
        let config = ReflowConfig {
            max_width: 80,
            clause_breaks: false,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        let plain = format!("{}\n", textwrap::fill(sentence, 80));
        assert_eq!(result, plain);
        // And that plain fill is *not* the clause-first shape
        assert!(
            result.contains("orchestrated, along with\n"),
            "control path still packs past the first comma: {result:?}"
        );
    }

    #[test]
    fn clause_breaks_via_reflow_config() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let regions = vec![Region::Prose(sentence.to_string())];
        let config = ReflowConfig {
            max_width: 80,
            clause_breaks: true,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        assert!(
            result.contains("orchestrated,\nalong with"),
            "reflow with clause_breaks must break after first comma: {result:?}"
        );
        assert!(
            result.contains("plan,\nwithout"),
            "reflow with clause_breaks must break after second comma: {result:?}"
        );
    }

    #[test]
    fn clause_breaks_handles_semicolon_colon_emdash() {
        let s = "First clause; second clause: third clause — fourth clause.";
        // Fits under the limit: no break is forced, the sentence stays whole.
        assert_eq!(wrap_with_clause_breaks(s, 80), s);
        // Forced under a tight limit: every break lands after clause punctuation.
        assert_eq!(
            wrap_with_clause_breaks(s, 20),
            "First clause;\nsecond clause:\nthird clause —\nfourth clause."
        );
    }

    #[test]
    fn clause_breaks_leave_fitting_sentences_alone() {
        let regions = vec![Region::Prose(
            "Hello, world. Short, sweet, and done.".to_string(),
        )];
        let config = ReflowConfig {
            max_width: 80,
            clause_breaks: true,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        assert_eq!(result, "Hello, world.\nShort, sweet, and done.\n");
    }

    #[test]
    fn clause_breaks_never_split_inside_tokens() {
        // Clause punctuation not followed by whitespace stays inside its
        // token; a break there would render as an inserted space.
        let s = "Totals reached 1,000,000 by 10:30 via https://example.com/a,b using --clause-breaks and rock—paper logic in a sentence long enough to need wrapping.";
        let wrapped = wrap_with_clause_breaks(s, 30);
        let rejoined: Vec<&str> = wrapped.split_whitespace().collect();
        let original: Vec<&str> = s.split_whitespace().collect();
        assert_eq!(rejoined, original, "wrapping must be lossless: {wrapped:?}");
        for token in [
            "1,000,000",
            "10:30",
            "https://example.com/a,b",
            "--clause-breaks",
            "rock—paper",
        ] {
            assert!(
                wrapped.lines().any(|l| l.contains(token)),
                "{token:?} must stay on a single line: {wrapped:?}"
            );
        }
    }

    #[test]
    fn clause_breaks_idempotent() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let config = ReflowConfig {
            max_width: 80,
            clause_breaks: true,
            ..Default::default()
        };
        let splitter = UnicodeSentenceSplitter::new();
        let first = reflow(
            &[Region::Prose(sentence.to_string())],
            &splitter,
            &config,
        );
        let second = reflow(
            &[Region::Prose(first.trim_end().to_string())],
            &splitter,
            &config,
        );
        assert_eq!(first, second, "clause-break reflow must be idempotent");
    }

    #[test]
    fn long_clause_still_word_wraps() {
        let long = "This is a deliberately long independent clause without internal punctuation that must still wrap under a tight max width constraint for the test.";
        let wrapped = wrap_with_clause_breaks(long, 40);
        for line in wrapped.lines() {
            assert!(
                line.chars().count() <= 40,
                "overlong clause must still wrap: {line:?}"
            );
        }
        assert!(wrapped.contains('\n'));
    }
}
