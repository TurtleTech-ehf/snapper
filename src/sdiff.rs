use std::path::Path;

use anyhow::{Context, Result};
use imara_diff::{Algorithm, BasicLineDiffPrinter, Diff, InternedInput, UnifiedDiffConfig};

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

    let mut output = String::new();
    let old_name = old_path.display();
    let new_name = new_path.display();

    if color {
        output.push_str(&format!("\x1b[1m--- a/{old_name}\x1b[0m\n"));
        output.push_str(&format!("\x1b[1m+++ b/{new_name}\x1b[0m\n"));
        for line in diff_text.lines() {
            if line.starts_with("@@") {
                output.push_str(&format!("\x1b[36m{line}\x1b[0m\n"));
            } else if line.starts_with('+') {
                output.push_str(&format!("\x1b[32m{line}\x1b[0m\n"));
            } else if line.starts_with('-') {
                output.push_str(&format!("\x1b[31m{line}\x1b[0m\n"));
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }
    } else {
        output.push_str(&format!("--- a/{old_name}\n"));
        output.push_str(&format!("+++ b/{new_name}\n"));
        output.push_str(&diff_text);
        if !diff_text.ends_with('\n') {
            output.push('\n');
        }
    }

    Ok(output)
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
}
