//! Document format detection and representation.
//!
//! The [`Format`] enum identifies the markup language of a document,
//! enabling format-specific parsing in the pipeline. A caller that has a
//! path uses the file extension. [`Format::detect`] does that first, then
//! scores the text when the path is missing or the extension is not a
//! prose format. The CLI can still name a format explicitly.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Org,
    Latex,
    Markdown,
    Rst,
    Plaintext,
    Typst,
    AsciiDoc,
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
            "typ" | "typst" => Some(Format::Typst),
            "adoc" | "asciidoc" | "asc" => Some(Format::AsciiDoc),
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

    /// Prefer `path` when its extension is a known prose format.
    /// Otherwise score `text` over the whole document.
    pub fn detect(path: Option<&Path>, text: &str) -> Self {
        if let Some(path) = path {
            if let Some(format) = Self::recognized_from_path(path) {
                return format;
            }
        }
        let scored = crate::detect::score_text(text);
        if scored == Format::Plaintext && crate::parser::asciidoc::looks_like_asciidoc(text) {
            Format::AsciiDoc
        } else {
            scored
        }
    }

    /// Format name as it appears in `.snapperrc.toml` (`org`, `latex`, …).
    ///
    /// Extension aliases (`md`, `tex`, `typ`, `txt`) are accepted. An unknown
    /// name is `None` so a typo in a path rule can be reported.
    pub fn from_config_key(name: &str) -> Option<Self> {
        match name {
            "org" => Some(Format::Org),
            "latex" | "tex" => Some(Format::Latex),
            "markdown" | "md" => Some(Format::Markdown),
            "rst" | "rest" => Some(Format::Rst),
            "plaintext" | "text" | "txt" => Some(Format::Plaintext),
            "typst" | "typ" => Some(Format::Typst),
            "asciidoc" | "adoc" => Some(Format::AsciiDoc),
            _ => None,
        }
    }

    pub fn config_key(self) -> &'static str {
        match self {
            Format::Org => "org",
            Format::Latex => "latex",
            Format::Markdown => "markdown",
            Format::Rst => "rst",
            Format::Plaintext => "plaintext",
            Format::Typst => "typst",
            Format::AsciiDoc => "asciidoc",
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
            crate::cli::FormatArg::Typst => Format::Typst,
            crate::cli::FormatArg::Asciidoc => Format::AsciiDoc,
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
        assert_eq!(
            Format::recognized_from_path(Path::new("paper.typ")),
            Some(Format::Typst)
        );
        assert_eq!(
            Format::recognized_from_extension("typst"),
            Some(Format::Typst)
        );
        assert_eq!(
            Format::recognized_from_path(Path::new("notes.adoc")),
            Some(Format::AsciiDoc)
        );
        assert_eq!(
            Format::recognized_from_extension("asciidoc"),
            Some(Format::AsciiDoc)
        );
        assert_eq!(
            Format::recognized_from_extension("asc"),
            Some(Format::AsciiDoc)
        );
    }

    #[test]
    fn source_extensions_are_not_prose() {
        assert_eq!(Format::recognized_from_path(Path::new("main.rs")), None);
        assert_eq!(Format::recognized_from_path(Path::new("app.py")), None);
        assert_eq!(Format::recognized_from_path(Path::new("README")), None);
    }
}
