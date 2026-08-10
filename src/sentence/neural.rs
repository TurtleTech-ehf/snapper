use std::sync::Mutex;

use nnsplit::NNSplit;

use super::SentenceSplitter;
use super::unicode::{UnicodeSentenceSplitter, protect_inline_tokens_with, restore_inline_tokens};

/// nnsplit downloads models to `~/.cache/nnsplit/` on first use; concurrent
/// loads race the download and can observe a partially written model
/// ("model proto does not contain a graph"). Serialize loads process-wide.
static MODEL_LOAD_LOCK: Mutex<()> = Mutex::new(());

/// Sentence splitter using nnsplit's neural network (byte-level LSTM via tract).
/// Models download and cache to ~/.cache/nnsplit/ on first use.
///
/// Pipeline matches the rules path for markup safety:
/// protect inline tokens → model on protected text → restore → abbrev + delim refine.
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
        let inner = {
            let _guard = MODEL_LOAD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            NNSplit::load(language, options)?
        };
        Ok(Self {
            inner,
            post: UnicodeSentenceSplitter::for_lang(language, extras),
        })
    }

    /// Extra LaTeX command names tokenized like `\verb` before split.
    pub fn with_verbatim_commands(mut self, cmds: Vec<String>) -> Self {
        self.post = self.post.with_verbatim_commands(cmds);
        self
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
        let inner = {
            let _guard = MODEL_LOAD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            NNSplit::new(path, options)?
        };
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

        let (protected, placeholders) =
            protect_inline_tokens_with(text, self.post.verbatim_commands());

        let splits = self.inner.split(&[protected.as_str()]);
        let raw: Vec<String> = if splits.is_empty() {
            vec![protected.clone()]
        } else {
            splits[0]
                .flatten(0)
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };

        let restored = restore_inline_tokens(raw, &placeholders);
        self.post.refine_segments(restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::unicode::UnicodeSentenceSplitter;

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
                .any(|s| s.contains("Hello world.") && s.contains("How are you?")),
            "expected glued dialogue span, got {result:?}"
        );
    }

    #[test]
    fn neural_org_emphasis_matches_rules_protection() {
        let neural = NeuralSentenceSplitter::new("en").unwrap();
        let rules = UnicodeSentenceSplitter::new();
        let input = "End of first. *Bold spans period. Continues* after.";
        let n = neural.split(input);
        let r = rules.split(input);
        assert!(
            n.iter()
                .any(|s| s.contains("*Bold spans period. Continues*")),
            "neural fractured emphasis: {n:?}"
        );
        assert!(
            r.iter()
                .any(|s| s.contains("*Bold spans period. Continues*")),
            "rules fractured emphasis: {r:?}"
        );
    }

    #[test]
    fn neural_org_link_not_split_on_abbrev_in_desc() {
        let neural = NeuralSentenceSplitter::new("en").unwrap();
        let input = "See [[https://example.com][Ex. Site]] for details. Then continue.";
        let n = neural.split(input);
        assert!(
            n.iter()
                .any(|s| s.contains("[[https://example.com][Ex. Site]]")),
            "neural split inside org link: {n:?}"
        );
        assert!(n.len() >= 2, "expected sentence after link: {n:?}");
    }
}
