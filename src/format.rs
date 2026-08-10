//! Document format detection and representation.
//!
//! The [`Format`] enum identifies the markup language of a document,
//! enabling format-specific parsing in the pipeline. Format is detected
//! from file extensions or can be specified explicitly via CLI flags.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Org,
    Latex,
    Markdown,
    Rst,
    Plaintext,
}

impl Format {
    /// Known prose extensions. `None` means "not a document snapper should
    /// touch" (`.rs`, `.py`, no extension). `.txt` is plaintext; unknown
    /// extensions are not silently treated as prose.
    pub fn recognized_from_extension(ext: &str) -> Option<Self> {
        match ext {
            "org" => Some(Format::Org),
            "tex" | "latex" | "ltx" | "sty" | "cls" => Some(Format::Latex),
            "md" | "markdown" | "mkd" | "mdx" => Some(Format::Markdown),
            "rst" | "rest" => Some(Format::Rst),
            "txt" | "text" => Some(Format::Plaintext),
            _ => None,
        }
    }

    pub fn recognized_from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::recognized_from_extension)
    }

    /// Detect format from file extension, defaulting to Plaintext.
    ///
    /// Prefer [`recognized_from_path`] at CLI boundaries so `.rs` is not
    /// formatted as prose. This fallback stays for stdin and explicit
    /// `--format plaintext`.
    pub fn from_path(path: &Path) -> Self {
        Self::recognized_from_path(path).unwrap_or(Format::Plaintext)
    }

    /// Detect format from a bare file extension string (without the dot).
    pub fn from_extension(ext: &str) -> Self {
        Self::recognized_from_extension(ext).unwrap_or(Format::Plaintext)
    }

    pub fn config_key(self) -> &'static str {
        match self {
            Format::Org => "org",
            Format::Latex => "latex",
            Format::Markdown => "markdown",
            Format::Rst => "rst",
            Format::Plaintext => "plaintext",
        }
    }

    #[cfg(feature = "cli")]
    pub fn from_arg(arg: crate::cli::FormatArg) -> Self {
        match arg {
            crate::cli::FormatArg::Org => Format::Org,
            crate::cli::FormatArg::Latex => Format::Latex,
            crate::cli::FormatArg::Markdown => Format::Markdown,
            crate::cli::FormatArg::Rst => Format::Rst,
            crate::cli::FormatArg::Plaintext => Format::Plaintext,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn known_extensions_are_recognized() {
        assert_eq!(
            Format::recognized_from_path(Path::new("paper.org")),
            Some(Format::Org)
        );
        assert_eq!(
            Format::recognized_from_path(Path::new("paper.tex")),
            Some(Format::Latex)
        );
        assert_eq!(
            Format::recognized_from_path(Path::new("notes.md")),
            Some(Format::Markdown)
        );
        assert_eq!(
            Format::recognized_from_path(Path::new("index.rst")),
            Some(Format::Rst)
        );
        assert_eq!(
            Format::recognized_from_path(Path::new("notes.txt")),
            Some(Format::Plaintext)
        );
    }

    #[test]
    fn source_extensions_are_not_prose() {
        assert_eq!(Format::recognized_from_path(Path::new("main.rs")), None);
        assert_eq!(Format::recognized_from_path(Path::new("app.py")), None);
        assert_eq!(Format::recognized_from_path(Path::new("README")), None);
    }
}
