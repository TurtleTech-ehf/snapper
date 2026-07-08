//! Pandoc parses first; snapper reflows second.
//!
//! For any format pandoc can read:
//! 1. **Parse** the source with pandoc → document AST (JSON via CLI, or
//!    in-process FFI).
//! 2. **Apply** snapper only to prose-bearing nodes (`Para` / `Plain`); leave
//!    `Header`, `CodeBlock`, `Table`, etc. alone because the AST says they are
//!    not prose.
//!
//! That is the opposite of the native path (guess structure from source lines,
//! then reflow). Here pandoc owns structure; snapper owns sentence line breaks
//! on the prose leaves.
//!
//! Backends that produce the same AST for [`ast::regions_from_pandoc`]:
//! - **CLI** ([`PandocBackend::Cli`]): `pandoc -t json` (full installed readers).
//! - **FFI** ([`PandocBackend::Ffi`]): `libsnapper_pandoc` (linked library readers).

pub mod ast;
pub mod cli;
pub mod ffi;

use std::path::Path;
use std::str::FromStr;

use thiserror::Error;

use crate::parser::{FormatParser, Region};

pub use ast::{regions_from_pandoc, regions_from_pandoc_json};
pub use cli::pandoc_cli_available as pandoc_available;
pub use ffi::ffi_available;

/// How to obtain the pandoc AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PandocBackend {
    /// In-process Haskell FFI (`libsnapper_pandoc`). Explicit error if unavailable.
    Ffi,
    /// Subprocess `pandoc -t json`. Explicit error if pandoc fails.
    #[default]
    Cli,
}

impl FromStr for PandocBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "ffi" | "lib" | "inprocess" | "in-process" => Ok(Self::Ffi),
            "cli" | "subprocess" | "command" => Ok(Self::Cli),
            other => Err(format!(
                "unknown pandoc backend '{other}' (expected 'ffi' or 'cli')"
            )),
        }
    }
}

impl PandocBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ffi => "ffi",
            Self::Cli => "cli",
        }
    }
}

/// Errors from either pandoc backend when explicitly selected.
#[derive(Debug, Error)]
pub enum PandocError {
    #[error(transparent)]
    Ffi(#[from] ffi::FfiError),
    #[error(transparent)]
    Cli(#[from] cli::CliError),
}

/// Parse input with the selected backend and classify via the pandoc AST.
pub fn parse_with_backend(
    input: &str,
    format: &str,
    backend: PandocBackend,
) -> Result<Vec<Region>, PandocError> {
    match backend {
        PandocBackend::Ffi => Ok(ffi::parse_via_ffi(input, format)?),
        PandocBackend::Cli => Ok(cli::parse_via_cli(input, format)?),
    }
}

/// Parser that uses pandoc for universal format support.
pub struct PandocParser {
    /// Pandoc input format (e.g. "latex", "markdown", "org", "rst", "typst")
    input_format: String,
    backend: PandocBackend,
}

impl PandocParser {
    pub fn new(format: &str) -> Self {
        Self {
            input_format: format.to_string(),
            backend: PandocBackend::default(),
        }
    }

    pub fn with_backend(format: &str, backend: PandocBackend) -> Self {
        Self {
            input_format: format.to_string(),
            backend,
        }
    }

    pub fn backend(&self) -> PandocBackend {
        self.backend
    }

    /// Fallible parse used by the library entry path (preferred).
    pub fn try_parse(&self, input: &str) -> Result<Vec<Region>, PandocError> {
        parse_with_backend(input, &self.input_format, self.backend)
    }

    /// Detect pandoc input format from file extension.
    pub fn format_for_path(path: &Path) -> Option<String> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("org") => Some("org".to_string()),
            Some("tex" | "latex" | "ltx") => Some("latex".to_string()),
            Some("md" | "markdown" | "mkd" | "mdx") => Some("markdown".to_string()),
            Some("rst" | "rest") => Some("rst".to_string()),
            Some("typ") => Some("typst".to_string()),
            Some("adoc" | "asciidoc") => Some("asciidoc".to_string()),
            Some("html" | "htm") => Some("html".to_string()),
            Some("docx") => Some("docx".to_string()),
            Some("txt") => Some("markdown".to_string()),
            _ => None,
        }
    }
}

impl FormatParser for PandocParser {
    /// Prefer [`PandocParser::try_parse`] / `format_text` (they surface errors).
    /// On failure this returns **empty** regions — never all-prose fallback.
    /// (`format_text` does not use this trait method for the pandoc path.)
    fn parse(&self, input: &str) -> Vec<Region> {
        self.try_parse(input).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_from_str() {
        assert_eq!("ffi".parse::<PandocBackend>().unwrap(), PandocBackend::Ffi);
        assert_eq!("cli".parse::<PandocBackend>().unwrap(), PandocBackend::Cli);
        assert!("bogus".parse::<PandocBackend>().is_err());
    }

    #[test]
    fn pandoc_format_detection() {
        assert_eq!(
            PandocParser::format_for_path(Path::new("paper.typ")),
            Some("typst".to_string())
        );
        assert_eq!(
            PandocParser::format_for_path(Path::new("doc.adoc")),
            Some("asciidoc".to_string())
        );
        assert_eq!(PandocParser::format_for_path(Path::new("file.xyz")), None);
    }

    #[test]
    fn try_parse_ffi_without_lib_is_err_not_all_prose() {
        // When the library is missing, FFI mode must error.
        if ffi_available() {
            // Environment has the lib; still verify parse returns regions of mixed kinds
            // if we can (optional live check).
            return;
        }
        let parser = PandocParser::with_backend("markdown", PandocBackend::Ffi);
        let err = parser.try_parse("Hello world.\n\n# Title\n").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unavailable") || msg.contains("FFI") || msg.contains("library"),
            "expected explicit FFI unavailability, got: {msg}"
        );
    }
}
