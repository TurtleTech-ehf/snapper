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

    let splitter = UnicodeSentenceSplitter::new().with_format(format);
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

    let fmt = match format.or_else(|| Format::recognized_from_path(old_path)) {
        Some(f) => f,
        None => anyhow::bail!(
            "{}: not a prose format; pass --format or use .org/.tex/.md/.rst/.txt",
            old_path.display()
        ),
    };

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
    fn sentence_extraction_uses_document_inline_syntax() {
        for (input, expected) in [
            ("A/B works. C/D works.", vec!["A/B works.", "C/D works."]),
            (
                "foo_bar works. Baz_qux works.",
                vec!["foo_bar works.", "Baz_qux works."],
            ),
            ("a+b works. C+d works.", vec!["a+b works.", "C+d works."]),
        ] {
            assert_eq!(extract_sentences(input, Format::Markdown), expected);
        }
        for marker in ['/', '+', '_'] {
            let span = format!("{marker}First sentence. Second sentence{marker}");
            let input = format!("{span} stays together. Next sentence.");
            assert_eq!(
                extract_sentences(&input, Format::Org),
                vec![format!("{span} stays together."), "Next sentence.".into()]
            );
        }
    }

    #[test]
    fn markdown_inline_character_reflow_produces_no_semantic_diff() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.md");
        let new = dir.path().join("new.md");
        for (input, formatted) in [
            ("A/B works. C/D works.\n", "A/B works.\nC/D works.\n"),
            (
                "foo_bar works. Baz_qux works.\n",
                "foo_bar works.\nBaz_qux works.\n",
            ),
            ("a+b works. C+d works.\n", "a+b works.\nC+d works.\n"),
        ] {
            std::fs::write(&old, input).unwrap();
            std::fs::write(&new, formatted).unwrap();
            assert!(
                sentence_diff(&old, &new, None, false).unwrap().is_empty(),
                "{input:?}"
            );
        }
    }

    fn temp_pair(
        left: &str,
        right: &str,
    ) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.txt");
        let new = dir.path().join("new.txt");
        std::fs::write(&old, left).unwrap();
        std::fs::write(&new, right).unwrap();
        (dir, old, new)
    }

    #[test]
    fn identical_files_produce_empty_diff() {
        let (_dir, tmp1, tmp2) = temp_pair(
            "Hello world. This is a test.\n",
            "Hello world. This is a test.\n",
        );
        let result = sentence_diff(&tmp1, &tmp2, Some(Format::Plaintext), false).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn changed_sentence_shows_diff() {
        let (_dir, tmp1, tmp2) = temp_pair(
            "Hello world. This is old. Goodbye.\n",
            "Hello world. This is new. Goodbye.\n",
        );
        let result = sentence_diff(&tmp1, &tmp2, Some(Format::Plaintext), false).unwrap();
        assert!(result.contains("-This is old."));
        assert!(result.contains("+This is new."));
    }

    #[test]
    fn reflow_produces_no_diff() {
        let (_dir, tmp1, tmp2) = temp_pair(
            "Hello world. This is a test. Another sentence.\n",
            "Hello world.\nThis is a test.\nAnother sentence.\n",
        );
        let result = sentence_diff(&tmp1, &tmp2, Some(Format::Plaintext), false).unwrap();
        assert!(result.is_empty(), "reflow should not produce a diff");
    }

    #[test]
    fn colored_sentence_diff_contains_ansi() {
        let (_dir, tmp1, tmp2) = temp_pair(
            "Hello world. This is old. Goodbye.\n",
            "Hello world. This is new. Goodbye.\n",
        );
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
    }
}
