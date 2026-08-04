use std::path::Path;

use anyhow::{Context, Result};
use imara_diff::{Algorithm, BasicLineDiffPrinter, Diff, InternedInput, UnifiedDiffConfig};

use crate::diff::colorize_unified_diff;
use crate::format::Format;
use crate::parser::Region;
use crate::sentence::SentenceSplitter;
use crate::sentence::unicode::UnicodeSentenceSplitter;

/// Extract all sentences from a document, preserving their order.
fn extract_sentences(input: &str, format: Format) -> Vec<String> {
    let parser = crate::parser::parser_for_format(format);

    let splitter = UnicodeSentenceSplitter::new();
    let regions = parser.parse(input);
    let mut sentences = Vec::new();

    for region in &regions {
        match region {
            Region::Prose(text) => {
                for s in splitter.split(text) {
                    if !s.is_empty() {
                        sentences.push(s);
                    }
                }
            }
            Region::Structure(s) => {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    sentences.push(trimmed.to_string());
                }
            }
            Region::BlankLines(_) => {
                sentences.push(String::new());
            }
            Region::Code {
                header,
                body,
                footer,
                ..
            } => {
                // Treat each non-empty code-block line as a single sentence
                // for diff purposes; this matches the previous behaviour where
                // code lines were emitted via `Region::Structure`.
                for line in header.lines().chain(body.lines()).chain(footer.lines()) {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        sentences.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    sentences
}

/// Run a sentence-level diff between two files.
pub fn sentence_diff(
    old_path: &Path,
    new_path: &Path,
    format: Option<Format>,
    color: bool,
) -> Result<String> {
    let old_text = std::fs::read_to_string(old_path)
        .with_context(|| format!("failed to read {}", old_path.display()))?;
    let new_text = std::fs::read_to_string(new_path)
        .with_context(|| format!("failed to read {}", new_path.display()))?;

    let fmt = format.unwrap_or_else(|| Format::from_path(old_path));

    let old_sentences = extract_sentences(&old_text, fmt);
    let new_sentences = extract_sentences(&new_text, fmt);

    // Join sentences as lines for diffing
    let old_lines = old_sentences.join("\n");
    let new_lines = new_sentences.join("\n");

    let input = InternedInput::new(old_lines.as_str(), new_lines.as_str());
    let diff = Diff::compute(Algorithm::Histogram, &input);

    let config = UnifiedDiffConfig::default(); // 3 lines context
    let printer = BasicLineDiffPrinter(&input.interner);
    let diff_text = diff.unified_diff(&printer, config, &input).to_string();

    if diff_text.is_empty() {
        return Ok(String::new());
    }

    let old_name = old_path.display();
    let new_name = new_path.display();
    let mut plain = String::new();
    plain.push_str(&format!("--- a/{old_name}\n"));
    plain.push_str(&format!("+++ b/{new_name}\n"));
    plain.push_str(&diff_text);
    if !diff_text.ends_with('\n') {
        plain.push('\n');
    }

    if color {
        Ok(colorize_unified_diff(&plain))
    } else {
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_files_produce_empty_diff() {
        let tmp1 = std::env::temp_dir().join("sdiff_same_a.txt");
        let tmp2 = std::env::temp_dir().join("sdiff_same_b.txt");
        std::fs::write(&tmp1, "Hello world. This is a test.\n").unwrap();
        std::fs::write(&tmp2, "Hello world. This is a test.\n").unwrap();
        let result = sentence_diff(&tmp1, &tmp2, Some(Format::Plaintext), false).unwrap();
        assert!(result.is_empty());
        std::fs::remove_file(&tmp1).ok();
        std::fs::remove_file(&tmp2).ok();
    }

    #[test]
    fn changed_sentence_shows_diff() {
        let tmp1 = std::env::temp_dir().join("sdiff_change_a.txt");
        let tmp2 = std::env::temp_dir().join("sdiff_change_b.txt");
        std::fs::write(&tmp1, "Hello world. This is old. Goodbye.\n").unwrap();
        std::fs::write(&tmp2, "Hello world. This is new. Goodbye.\n").unwrap();
        let result = sentence_diff(&tmp1, &tmp2, Some(Format::Plaintext), false).unwrap();
        assert!(result.contains("-This is old."));
        assert!(result.contains("+This is new."));
        std::fs::remove_file(&tmp1).ok();
        std::fs::remove_file(&tmp2).ok();
    }

    #[test]
    fn reflow_produces_no_diff() {
        let tmp1 = std::env::temp_dir().join("sdiff_reflow_a.txt");
        let tmp2 = std::env::temp_dir().join("sdiff_reflow_b.txt");
        std::fs::write(&tmp1, "Hello world. This is a test. Another sentence.\n").unwrap();
        std::fs::write(&tmp2, "Hello world.\nThis is a test.\nAnother sentence.\n").unwrap();
        let result = sentence_diff(&tmp1, &tmp2, Some(Format::Plaintext), false).unwrap();
        assert!(result.is_empty(), "reflow should not produce a diff");
        std::fs::remove_file(&tmp1).ok();
        std::fs::remove_file(&tmp2).ok();
    }

    #[test]
    fn colored_sentence_diff_contains_ansi() {
        let tmp1 = std::env::temp_dir().join("sdiff_color_a.txt");
        let tmp2 = std::env::temp_dir().join("sdiff_color_b.txt");
        std::fs::write(&tmp1, "Hello world. This is old. Goodbye.\n").unwrap();
        std::fs::write(&tmp2, "Hello world. This is new. Goodbye.\n").unwrap();
        let colored = sentence_diff(&tmp1, &tmp2, Some(Format::Plaintext), true).unwrap();
        let plain = sentence_diff(&tmp1, &tmp2, Some(Format::Plaintext), false).unwrap();
        assert!(
            colored.contains("\x1b["),
            "color=true must emit ANSI: {colored:?}"
        );
        assert!(
            !plain.contains("\x1b["),
            "color=false must not emit ANSI: {plain:?}"
        );
        assert!(colored.contains("\x1b[31m-This is old.\x1b[0m"));
        assert!(colored.contains("\x1b[32m+This is new.\x1b[0m"));
        std::fs::remove_file(&tmp1).ok();
        std::fs::remove_file(&tmp2).ok();
    }
}
