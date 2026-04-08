#[cfg(feature = "neural")]
pub mod neural;
pub mod unicode;

/// Trait for sentence boundary detection.
pub trait SentenceSplitter {
    /// Split a prose string into individual sentences.
    fn split(&self, text: &str) -> Vec<String>;
}
