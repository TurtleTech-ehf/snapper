use crate::parser::Region;
use crate::sentence::SentenceSplitter;

/// Configuration for the reflow engine.
pub struct ReflowConfig {
    /// Maximum line width. 0 means unlimited.
    pub max_width: usize,
}

/// Reflow a sequence of regions, applying sentence breaks to Prose regions.
pub fn reflow(
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    let mut output = String::new();

    for (idx, region) in regions.iter().enumerate() {
        match region {
            Region::Structure(s) => output.push_str(s),
            Region::BlankLines(s) => output.push_str(s),
            Region::Prose(text) => {
                let sentences = splitter.split(text);
                for (i, sentence) in sentences.iter().enumerate() {
                    if config.max_width > 0 {
                        let wrapped = textwrap::fill(sentence, config.max_width);
                        output.push_str(&wrapped);
                    } else {
                        output.push_str(sentence);
                    }
                    if i < sentences.len() - 1 {
                        output.push('\n');
                    }
                }
                // Add trailing newline when followed by BlankLines or
                // another Prose region, so paragraph breaks are preserved.
                // Skip when followed by Structure (e.g. the "\n" after
                // headlines/list items) to avoid double newlines.
                if !sentences.is_empty() {
                    let needs_newline = match regions.get(idx + 1) {
                        Some(Region::BlankLines(_)) | Some(Region::Prose(_)) => true,
                        None => true,
                        Some(Region::Structure(_)) => false,
                    };
                    if needs_newline {
                        output.push('\n');
                    }
                }
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::unicode::UnicodeSentenceSplitter;

    fn reflow_text(input: &str) -> String {
        let regions = vec![Region::Prose(input.to_string())];
        let config = ReflowConfig { max_width: 0 };
        reflow(&regions, &UnicodeSentenceSplitter, &config)
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
        let config = ReflowConfig { max_width: 0 };
        let result = reflow(&regions, &UnicodeSentenceSplitter, &config);
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
        let config = ReflowConfig { max_width: 40 };
        let result = reflow(&regions, &UnicodeSentenceSplitter, &config);
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
}
