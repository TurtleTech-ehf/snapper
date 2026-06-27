#[cfg(feature = "neural")]
pub mod neural;
pub mod unicode;

/// Trait for sentence boundary detection.
///
/// `Send + Sync` so large documents can reflow independent regions in parallel
/// (cli feature / rayon) without cloning the splitter.
pub trait SentenceSplitter: Send + Sync {
    /// Split a prose string into individual sentences.
    fn split(&self, text: &str) -> Vec<String>;
}
