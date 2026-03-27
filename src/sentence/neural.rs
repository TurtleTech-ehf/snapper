use nnsplit::NNSplit;

use super::SentenceSplitter;

/// Sentence splitter using nnsplit's neural network (byte-level LSTM via tract).
/// Models download and cache to ~/.cache/nnsplit/ on first use.
pub struct NeuralSentenceSplitter {
    inner: NNSplit,
}

impl NeuralSentenceSplitter {
    /// Load a model by language code (e.g. "en", "de", "fr").
    /// Downloads and caches to ~/.cache/nnsplit/ on first use.
    pub fn new(language: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let options = nnsplit::NNSplitOptions::default();
        let inner = NNSplit::load(language, options)?;
        Ok(Self { inner })
    }

    /// Load from a custom ONNX model file path.
    pub fn from_path(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let options = nnsplit::NNSplitOptions::default();
        let inner = NNSplit::new(path, options)?;
        Ok(Self { inner })
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
            return vec![text.to_string()];
        }

        // Level 0 = sentences in nnsplit's hierarchy
        splits[0]
            .flatten(0)
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
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
}
