use nnsplit::NNSplit;

use super::SentenceSplitter;
use super::unicode::UnicodeSentenceSplitter;

/// Sentence splitter using nnsplit's neural network (byte-level LSTM via tract).
/// Models download and cache to ~/.cache/nnsplit/ on first use.
///
/// After the model proposes boundaries, segments are passed through the same
/// abbreviation + delimiter-span post-pipeline as [`UnicodeSentenceSplitter`]
/// so dialogue quotes and `Dr.`-style titles stay consistent with the rules path.
pub struct NeuralSentenceSplitter {
    inner: NNSplit,
    post: UnicodeSentenceSplitter,
}

impl NeuralSentenceSplitter {
    /// Load a model by language code (e.g. "en", "de", "fr").
    /// Downloads and caches to ~/.cache/nnsplit/ on first use.
    pub fn new(language: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_extras(language, &[])
    }

    /// Like [`Self::new`] but merges project `extra_abbreviations` in post-process.
    pub fn with_extras(
        language: &str,
        extras: &[String],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let options = nnsplit::NNSplitOptions::default();
        let inner = NNSplit::load(language, options)?;
        Ok(Self {
            inner,
            post: UnicodeSentenceSplitter::for_lang(language, extras),
        })
    }

    /// Load from a custom ONNX model file path.
    pub fn from_path(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        Self::from_path_with_extras(path, "en", &[])
    }

    /// Custom model path plus language/extras for the shared post-pipeline.
    pub fn from_path_with_extras(
        path: &std::path::Path,
        language: &str,
        extras: &[String],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let options = nnsplit::NNSplitOptions::default();
        let inner = NNSplit::new(path, options)?;
        Ok(Self {
            inner,
            post: UnicodeSentenceSplitter::for_lang(language, extras),
        })
    }
}

impl SentenceSplitter for NeuralSentenceSplitter {
    fn split(&self, text: &str) -> Vec<String> {
        let text = text.trim();
        if text.is_empty() {
            return vec![];
        }

        let splits = self.inner.split(&[text]);
        if splits.is_empty() {
            return self.post.refine_segments(vec![text.to_string()]);
        }

        // Level 0 = sentences in nnsplit's hierarchy
        let raw: Vec<String> = splits[0]
            .flatten(0)
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        self.post.refine_segments(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neural_english_basic() {
        let splitter = NeuralSentenceSplitter::new("en").unwrap();
        let result = splitter.split("Hello world. This is a test. Another sentence.");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn neural_empty_input() {
        let splitter = NeuralSentenceSplitter::new("en").unwrap();
        assert!(splitter.split("").is_empty());
    }

    #[test]
    fn neural_abbreviation_handling() {
        let splitter = NeuralSentenceSplitter::new("en").unwrap();
        let result = splitter.split("Dr. Smith went home. He was tired.");
        assert!(result.len() >= 2);
    }

    #[test]
    fn neural_dialogue_quote_not_fractured() {
        let splitter = NeuralSentenceSplitter::new("en").unwrap();
        let result = splitter.split(r#"He said "Hello world. How are you?" Then he left."#);
        assert!(
            result
                .iter()
                .all(|s| !s.contains("world.\n") && !s.ends_with("world.")),
            "unexpected mid-quote fracture in segments: {result:?}"
        );
        // Prefer a single segment carrying the closing quote before "Then".
        let joined = result.join(" | ");
        assert!(
            !joined.contains(r#"world. | How"#) && !joined.contains(r#"world.| How"#),
            "dialogue split inside quotes: {result:?}"
        );
        assert!(
            result
                .iter()
                .any(|s| s.contains("Hello world.") && s.contains("How are you?")),
            "expected glued dialogue span, got {result:?}"
        );
    }
}
