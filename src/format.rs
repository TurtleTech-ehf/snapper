//! Document format detection and representation.
//!
//! The [`Format`] enum identifies the markup language of a document,
//! enabling format-specific parsing in the pipeline. Format is detected
//! from file extensions or can be specified explicitly via CLI flags.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Org,
    Latex,
    Markdown,
    Rst,
    Plaintext,
}

impl Format {
    /// Detect format from file extension, defaulting to Plaintext.
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("org") => Format::Org,
            Some("tex" | "latex" | "ltx" | "sty" | "cls") => Format::Latex,
            Some("md" | "markdown" | "mkd" | "mdx") => Format::Markdown,
            Some("rst" | "rest") => Format::Rst,
            _ => Format::Plaintext,
        }
    }

    /// Detect format from a bare file extension string (without the dot).
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "org" => Format::Org,
            "tex" | "latex" | "ltx" | "sty" | "cls" => Format::Latex,
            "md" | "markdown" | "mkd" | "mdx" => Format::Markdown,
            "rst" | "rest" => Format::Rst,
            _ => Format::Plaintext,
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
