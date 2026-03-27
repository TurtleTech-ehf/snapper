use std::path::Path;

use crate::cli::FormatArg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Org,
    Latex,
    Markdown,
    Plaintext,
}

impl Format {
    /// Detect format from file extension, defaulting to Plaintext.
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("org") => Format::Org,
            Some("tex" | "latex" | "ltx" | "sty" | "cls") => Format::Latex,
            Some("md" | "markdown" | "mkd" | "mdx") => Format::Markdown,
            _ => Format::Plaintext,
        }
    }

    pub fn from_arg(arg: FormatArg) -> Self {
        match arg {
            FormatArg::Org => Format::Org,
            FormatArg::Latex => Format::Latex,
            FormatArg::Markdown => Format::Markdown,
            FormatArg::Plaintext => Format::Plaintext,
        }
    }
}
