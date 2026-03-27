pub mod latex;
pub mod markdown;
pub mod org;
pub mod plaintext;

/// A region of text classified by a format parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    /// Prose text that should be reflowed with semantic line breaks.
    Prose(String),
    /// Structural content that must pass through unchanged.
    Structure(String),
    /// Blank line(s) preserved as paragraph separators.
    BlankLines(String),
}

/// Trait for format-specific parsers that classify text into regions.
pub trait FormatParser {
    fn parse(&self, input: &str) -> Vec<Region>;
}
