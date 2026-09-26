//! Pandoc parses first; snapper reflows second; pandoc writes third.
//!
//! For any format pandoc can read:
//! 1. **Parse** the source with pandoc → document AST (JSON via CLI, or
//!    in-process FFI).
//! 2. **Apply** snapper only to prose-bearing nodes (`Para` / `Plain`); leave
//!    `Header`, `CodeBlock`, `Table`, etc. alone because the AST says they are
//!    not prose.
//! 3. **Write** the mutated AST through an in-process FFI writer when the
//!    loaded library exports one, otherwise `pandoc -f json --wrap=preserve`.
//!    Do not splice original source bytes. A missing CLI writer is an
//!    explicit PATH error (no silent spawn).
//!
//! That is the opposite of the native path (guess structure from source lines,
//! then splice). Here pandoc owns structure; snapper owns sentence line breaks
//! on the prose leaves.
//!
//! Backends that produce the same AST for [`ast::regions_from_pandoc`]:
//! - **CLI** ([`PandocBackend::Cli`]): `pandoc -t json` (full installed readers).
//! - **FFI** ([`PandocBackend::Ffi`]): `libsnapper_pandoc` (linked library readers).
//!
//! The shipped library is reader-only. Write stays in-process only when
//! `snapper_pandoc_write` is exported; otherwise help/error say the writer
//! still needs `pandoc` on PATH.

pub mod ast;
pub mod cache;
pub mod cli;
pub mod ffi;
pub mod reflow;
mod shield;
pub mod write;

use std::path::Path;
use std::str::FromStr;

use thiserror::Error;

use crate::parser::{FormatParser, Region};

pub use ast::{regions_from_pandoc, regions_from_pandoc_json};
pub use cli::pandoc_cli_available as pandoc_available;
pub use ffi::{ffi_available, ffi_write_available};
pub use reflow::reflow_pandoc;
pub use write::{WRITER_NEEDS_PATH, write_ast, write_via_cli};

/// How to obtain the pandoc AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PandocBackend {
    /// Prefer in-process FFI when `libsnapper_pandoc` loads; else CLI.
    /// Parse amortizes the RTS. Write stays in-process only when the
    /// library exports a writer; otherwise the CLI writer is explicit.
    #[default]
    Auto,
    /// In-process Haskell FFI (`libsnapper_pandoc`). Explicit error if unavailable.
    Ffi,
    /// Subprocess `pandoc -t json`. Explicit error if pandoc fails.
    Cli,
}

impl FromStr for PandocBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" | "default" => Ok(Self::Auto),
            "ffi" | "lib" | "inprocess" | "in-process" => Ok(Self::Ffi),
            "cli" | "subprocess" | "command" => Ok(Self::Cli),
            other => Err(format!(
                "unknown pandoc backend '{other}' (expected 'auto', 'ffi', or 'cli')"
            )),
        }
    }
}

impl PandocBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Ffi => "ffi",
            Self::Cli => "cli",
        }
    }

    /// Resolve Auto → Ffi if the library loads, else Cli.
    pub fn resolve(self) -> Self {
        match self {
            Self::Auto => {
                if ffi_available() {
                    Self::Ffi
                } else {
                    Self::Cli
                }
            }
            other => other,
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
    #[error("pandoc AST cache/classify: {0}")]
    Ast(String),
    /// Backstop: write-through deleted an RST `..` comment or a
    /// `snapper:off` / `snapper:on` that the shield failed to restore.
    #[error("pandoc path would drop {0}; pass --native to keep comments and snapper pragmas")]
    DropsComments(String),
}

/// Whether the CLI default can run the pandoc path without erroring.
///
/// True when an in-process FFI writer is loaded or `pandoc` is on PATH.
/// Reader-only FFI without PATH is not enough (write would fail).
pub fn pandoc_default_available() -> bool {
    ffi_write_available() || pandoc_available()
}

/// Parse input with the selected backend and classify via the pandoc AST.
///
/// Uses a content-addressed AST JSON cache (memory + disk) so repeated formats
/// of the same source skip pandoc entirely after the first successful parse.
pub fn parse_with_backend(
    input: &str,
    format: &str,
    backend: PandocBackend,
) -> Result<Vec<Region>, PandocError> {
    if let Some(json) = cache::get_json(format, input) {
        return regions_from_pandoc_json(json.as_ref()).map_err(PandocError::Ast);
    }
    let (regions, json_opt) = match backend.resolve() {
        PandocBackend::Auto => unreachable!("resolve collapses Auto"),
        PandocBackend::Ffi => {
            let (regs, json) = ffi::parse_via_ffi_with_json(input, format)?;
            (regs, Some(json))
        }
        PandocBackend::Cli => {
            let (regs, json) = cli::parse_via_cli_with_json(input, format)?;
            (regs, Some(json))
        }
    };
    if let Some(json) = json_opt {
        cache::put_json(format, input, &json);
    }
    Ok(regions)
}

/// Obtain pandoc JSON for `(format, input)` (cache, then FFI or CLI).
pub fn json_with_backend(
    input: &str,
    format: &str,
    backend: PandocBackend,
) -> Result<String, PandocError> {
    if let Some(json) = cache::get_json(format, input) {
        return Ok(json.as_ref().to_string());
    }
    let json = match backend.resolve() {
        PandocBackend::Auto => unreachable!("resolve collapses Auto"),
        PandocBackend::Ffi => {
            let (_regs, json) = ffi::parse_via_ffi_with_json(input, format)?;
            json
        }
        PandocBackend::Cli => {
            let (_regs, json) = cli::parse_via_cli_with_json(input, format)?;
            json
        }
    };
    cache::put_json(format, input, &json);
    Ok(json)
}

/// Why write-through would delete comments or pragmas. `None` if the
/// source has none of those constructs. Does not invoke pandoc.
pub fn dropped_comment_kind(input: &str, format: &str) -> Option<&'static str> {
    if input
        .lines()
        .any(|line| crate::parser::check_pragma(line).is_some())
    {
        return Some("snapper:off/on");
    }
    if shield::is_rst_pandoc_format(format)
        && crate::parser::rst::source_has_dropped_rst_comments(input)
    {
        return Some("RST comment");
    }
    None
}

/// Writer-side backstop: a detector miss must still not exit 0 after a drop.
fn refuse_if_comments_missing(input: &str, format: &str, output: &str) -> Result<(), PandocError> {
    for token in ["snapper:off", "snapper:on"] {
        if input.contains(token) && !output.contains(token) {
            return Err(PandocError::DropsComments("snapper:off/on".into()));
        }
    }
    if shield::is_rst_pandoc_format(format) {
        let lines: Vec<&str> = input.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let trimmed = lines[i].trim_start();
            if !crate::parser::rst::is_rst_dropped_comment_opener(trimmed) {
                i += 1;
                continue;
            }
            let payload = trimmed.strip_prefix("..").unwrap_or("").trim();
            if !payload.is_empty() && !output.contains(payload) {
                return Err(PandocError::DropsComments("RST comment".into()));
            }
            let leading = lines[i].len() - trimmed.len();
            let comment_indent = leading + 1;
            i += 1;
            while i < lines.len() {
                let body = lines[i];
                let lead = body.len() - body.trim_start().len();
                if body.trim().is_empty() || lead >= comment_indent {
                    let body_txt = body.trim();
                    if !body_txt.is_empty() && !output.contains(body_txt) {
                        return Err(PandocError::DropsComments("RST comment".into()));
                    }
                    i += 1;
                    continue;
                }
                break;
            }
        }
    }
    Ok(())
}

/// Parse → reflow Para/Plain → write through pandoc's writer.
///
/// Not a splice of the original bytes. If the FFI library has no writer,
/// this still needs `pandoc` on PATH and says so (`WRITER_NEEDS_PATH`).
///
/// RST `..` comments and `snapper:off` / `snapper:on` are shielded
/// across the reader so the written output still contains that text.
/// A restore miss is still `DropsComments` (fail-closed backstop).
pub fn format_via_pandoc(
    input: &str,
    format: &str,
    backend: PandocBackend,
    splitter: &dyn crate::sentence::SentenceSplitter,
    reflow_config: &crate::reflow::ReflowConfig,
) -> Result<String, PandocError> {
    let shield = shield::Shield::apply(input, format);
    let json = json_with_backend(&shield.source, format, backend)?;
    let mut doc: pandoc_ast::Pandoc =
        serde_json::from_str(&json).map_err(|e| PandocError::Ast(e.to_string()))?;
    reflow_pandoc(&mut doc, splitter, reflow_config);
    let out_json =
        serde_json::to_string(&doc).map_err(|e| PandocError::Ast(format!("serialize AST: {e}")))?;
    let written = write_ast(&out_json, format).map_err(PandocError::from)?;
    let written = shield.restore(&written);
    refuse_if_comments_missing(input, format, &written)?;
    Ok(written)
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
    /// Pandoc rebuilds regions from an AST, so origins are unset and reflow
    /// falls back to concatenating region strings.
    fn parse_full(&self, input: &str) -> Vec<crate::parser::SpannedRegion> {
        self.try_parse(input)
            .unwrap_or_default()
            .into_iter()
            .map(crate::parser::SpannedRegion::unspanned)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_from_str() {
        assert_eq!(
            "auto".parse::<PandocBackend>().unwrap(),
            PandocBackend::Auto
        );
        assert_eq!("ffi".parse::<PandocBackend>().unwrap(), PandocBackend::Ffi);
        assert_eq!("cli".parse::<PandocBackend>().unwrap(), PandocBackend::Cli);
        assert_eq!(PandocBackend::default(), PandocBackend::Auto);
        assert!("bogus".parse::<PandocBackend>().is_err());
    }

    #[test]
    fn backend_auto_resolves_to_ffi_or_cli() {
        let r = PandocBackend::Auto.resolve();
        assert!(matches!(r, PandocBackend::Ffi | PandocBackend::Cli));
        if ffi_available() {
            assert_eq!(r, PandocBackend::Ffi);
        } else {
            assert_eq!(r, PandocBackend::Cli);
        }
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

    #[test]
    fn dropped_comment_kind_rst_bare_dotdot() {
        assert_eq!(
            dropped_comment_kind("..\n   Secret.\n", "rst"),
            Some("RST comment")
        );
        assert_eq!(
            dropped_comment_kind(".. This is a comment.\n", "rst"),
            Some("RST comment")
        );
        assert_eq!(dropped_comment_kind("Hello world.\n", "rst"), None);
        assert_eq!(dropped_comment_kind(".. note::\n   Body.\n", "rst"), None);
        assert_eq!(dropped_comment_kind(".. _label:\n", "rst"), None);
    }

    #[test]
    fn dropped_comment_kind_pragmas_any_format() {
        assert_eq!(
            dropped_comment_kind("Hello.\n<!-- snapper:off -->\nKeep.\n", "markdown"),
            Some("snapper:off/on")
        );
        assert_eq!(
            dropped_comment_kind("Hello.\nsnapper:off\nKeep.\n", "rst"),
            Some("snapper:off/on")
        );
        assert_eq!(
            dropped_comment_kind("# snapper:off\nKeep.\n", "org"),
            Some("snapper:off/on")
        );
        assert_eq!(dropped_comment_kind("Hello world.\n", "markdown"), None);
    }

    #[test]
    fn format_via_pandoc_rst_comment_is_preserved() {
        if !pandoc_available() && !ffi_write_available() {
            return;
        }
        let input = "..\n   First sentence.\n   Second sentence.\n";
        let out = format_via_pandoc(
            input,
            "rst",
            PandocBackend::Cli,
            &crate::sentence::unicode::UnicodeSentenceSplitter::new(),
            &crate::reflow::ReflowConfig::default(),
        )
        .expect("pandoc path must keep RST comments");
        assert!(
            out.contains("First sentence.") && out.contains("Second sentence."),
            "RST comment body must survive:\n{out}"
        );
    }

    #[test]
    fn default_use_pandoc_is_false() {
        assert!(
            !crate::FormatConfig::default().use_pandoc,
            "library/wasm/LSP stay native; CLI default is snapper-32ps"
        );
    }

    fn assert_no_use_pandoc_knob(label: &str, src: &str) {
        let lower = src.to_ascii_lowercase();
        assert!(
            !lower.contains("set_use_pandoc"),
            "{label} must not expose set_use_pandoc"
        );
        assert!(
            !lower.contains("usepandoc"),
            "{label} must not expose usePandoc"
        );
        assert!(
            !src.contains("use_pandoc"),
            "{label} must not set or forward use_pandoc"
        );
        assert!(
            !src.contains("--use-pandoc"),
            "{label} must not pass --use-pandoc"
        );
    }

    fn repo_src(rel: &str) -> Option<String> {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).ok()
    }

    #[test]
    fn wasm_and_editor_config_cannot_set_use_pandoc() {
        let wasm = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/wasm.rs"));
        assert!(
            wasm.contains("use_pandoc: false"),
            "wasm must hardcode use_pandoc false (snapper-ekc0)"
        );
        assert_eq!(
            wasm.matches("use_pandoc").count(),
            1,
            "wasm must have exactly one use_pandoc (hardcoded false, no setter)"
        );
        assert!(
            !wasm.contains("set_use_pandoc"),
            "WasmConfig must not grow a use_pandoc setter"
        );

        let lsp = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lsp.rs"));
        assert!(
            lsp.contains("use_pandoc: false"),
            "VS Code LSP must force native parsers"
        );
        assert!(!lsp.contains("use_pandoc: true"));
        assert!(!lsp.contains("set_use_pandoc"));
        assert!(!lsp.contains("--use-pandoc"));

        // Editor / JS wrappers live in the git tree, not the crates.io tarball.
        let extras = [
            "packages/snapper-wasm/src/types.ts",
            "packages/snapper-wasm/src/index.ts",
            "editors/obsidian/src/formatter.ts",
            "editors/obsidian/src/settings.ts",
            "editors/word/src/shared/formatter.ts",
            "editors/vscode/src/extension.ts",
        ];
        let mut seen = 0;
        for rel in extras {
            if let Some(src) = repo_src(rel) {
                seen += 1;
                assert_no_use_pandoc_knob(rel, &src);
            }
        }
        if repo_src("editors/vscode/src/extension.ts").is_some() {
            assert_eq!(
                seen,
                extras.len(),
                "all editor sources readable in checkout"
            );
        }
    }

    #[test]
    fn cargo_dist_default_features_stay_ghc_free() {
        let cargo = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let default_line = cargo
            .lines()
            .find(|l| l.starts_with("default ="))
            .expect("default features line");
        assert!(
            !default_line.contains("pandoc-colink"),
            "cargo-dist default builds must stay GHC-free: {default_line}"
        );
        let dist = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/dist-workspace.toml"));
        assert!(
            !dist.contains("pandoc-colink"),
            "dist-workspace.toml must not enable pandoc-colink"
        );
    }

    #[test]
    fn format_via_pandoc_pragma_is_preserved() {
        if !pandoc_available() && !ffi_write_available() {
            return;
        }
        let input = "Hello world. Second.\n<!-- snapper:off -->\nKeep this.\n<!-- snapper:on -->\n";
        let out = format_via_pandoc(
            input,
            "markdown",
            PandocBackend::Cli,
            &crate::sentence::unicode::UnicodeSentenceSplitter::new(),
            &crate::reflow::ReflowConfig::default(),
        )
        .expect("pandoc path must keep snapper:off/on");
        assert!(
            out.contains("<!-- snapper:off -->") && out.contains("<!-- snapper:on -->"),
            "pragma markers must survive:\n{out}"
        );
        assert!(
            out.contains("Keep this."),
            "off-region body must survive:\n{out}"
        );
    }

    #[test]
    fn format_via_pandoc_org_pragma_is_preserved() {
        if !pandoc_available() && !ffi_write_available() {
            return;
        }
        let input = "Hello world. Second.\n# snapper:off\nKeep this. Exactly here.\n# snapper:on\nAfter. Two.\n";
        let out = format_via_pandoc(
            input,
            "org",
            PandocBackend::Cli,
            &crate::sentence::unicode::UnicodeSentenceSplitter::new(),
            &crate::reflow::ReflowConfig::default(),
        )
        .expect("pandoc path must keep org snapper:off/on");
        assert!(
            out.contains("snapper:off") && out.contains("snapper:on"),
            "org pragmas must survive:\n{out}"
        );
        assert!(
            out.contains("Keep this. Exactly here."),
            "org off-region body must survive:\n{out}"
        );
    }

    #[test]
    fn refuse_if_comments_missing_catches_restore_miss() {
        let err = refuse_if_comments_missing(
            "Hello.\n<!-- snapper:off -->\nKeep.\n",
            "markdown",
            "Hello.\nKeep.\n",
        )
        .unwrap_err();
        match err {
            PandocError::DropsComments(kind) => assert!(kind.contains("snapper")),
            other => panic!("expected DropsComments, got {other}"),
        }
        let err = refuse_if_comments_missing(
            "..\n   This comment must not vanish.\n",
            "rst",
            "Hello world.\n",
        )
        .unwrap_err();
        match err {
            PandocError::DropsComments(kind) => assert!(kind.contains("RST")),
            other => panic!("expected DropsComments, got {other}"),
        }
    }
}
