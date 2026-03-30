//! Document format detection and representation.
//!
//! The [`Format`] enum identifies the markup language of a document,
//! enabling format-specific parsing in the pipeline. Format is detected
//! from file extensions or can be specified explicitly via CLI flags.

use std::path::Path;

use crate::cli::FormatArg;

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

    pub fn from_arg(arg: FormatArg) -> Self {
        match arg {
            FormatArg::Org => Format::Org,
            FormatArg::Latex => Format::Latex,
            FormatArg::Markdown => Format::Markdown,
            FormatArg::Rst => Format::Rst,
            FormatArg::Plaintext => Format::Plaintext,
        }
    }
}
