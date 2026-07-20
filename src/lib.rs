//! # snapper
//!
//! Semantic line break formatter for prose documents. Reformats text so each
//! sentence occupies its own line, producing minimal git diffs when
//! collaborating on papers and documentation.
//!
//! The crate is published as `snapper-fmt` on crates.io; the binary it
//! installs is called `snapper`.
//!
//! ## Supported formats
//!
//! - **Org-mode**: drawers, tables, keywords preserved; `#+BEGIN_SRC` is
//!   `Region::Code` (comment reflow via `[code.<lang>]`, optional formatters)
//! - **LaTeX**: preamble and math preserved; `minted` / `lstlisting` are code regions
//! - **Markdown**: front matter and headings preserved; fenced blocks are code regions
//! - **RST**: directives and literals preserved; `.. code-block::` is a code region
//! - **Plaintext**: everything treated as prose
//!
//! ## Library usage
//!
//! ```rust
//! use snapper_fmt::{format_text, FormatConfig};
//! use snapper_fmt::format::Format;
//!
//! let input = "Hello world. This is a test. Another sentence.";
//! let config = FormatConfig {
//!     format: Format::Plaintext,
//!     ..Default::default()
//! };
//! let output = format_text(input, &config).unwrap();
//! assert_eq!(output, "Hello world.\nThis is a test.\nAnother sentence.");
//! ```

pub mod abbreviations;
#[cfg(feature = "cli")]
pub mod cli;
pub mod code_block;
pub mod config;
pub mod diff;
#[cfg(not(target_arch = "wasm32"))]
pub mod files;
pub mod format;
#[cfg(not(target_arch = "wasm32"))]
pub mod git_diff;
#[cfg(feature = "cli")]
pub mod init;
#[cfg(feature = "lsp")]
pub mod lsp;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod output;
pub mod parser;
pub mod reflow;
#[cfg(not(target_arch = "wasm32"))]
pub mod sdiff;
pub mod sentence;
#[cfg(feature = "wasm")]
pub mod wasm;
#[cfg(feature = "watch")]
pub mod watch;

use std::collections::HashMap;

use anyhow::Result;

use crate::config::CodeLang;
use crate::format::Format;
use crate::reflow::{ReflowConfig, reflow};
use crate::sentence::SentenceSplitter;
use crate::sentence::unicode::UnicodeSentenceSplitter;

/// Configuration for the formatting pipeline.
pub struct FormatConfig {
    pub format: Format,
    pub max_width: usize,
    pub use_neural: bool,
    pub neural_lang: String,
    pub neural_model_path: Option<std::path::PathBuf>,
    pub extra_abbreviations: Vec<String>,
    pub use_pandoc: bool,
    /// Pandoc input format string (for pandoc backend).
    pub pandoc_format: Option<String>,
    /// How to obtain the pandoc AST when `use_pandoc` is set.
    /// `Ffi` uses in-process Haskell/C bindings; `Cli` uses a subprocess.
    #[cfg(feature = "pandoc")]
    pub pandoc_backend: parser::pandoc::PandocBackend,
    /// Per-language code-block configuration loaded from `[code]` in
    /// `.snapperrc.toml`. Empty by default; an empty map disables all
    /// per-language code-block behaviour (block passes through untouched).
    pub code: HashMap<String, CodeLang>,
    /// When `true`, the reflow stage invokes each language's `formatter`
    /// after comment reflow. Default `false` preserves v0.7.7 behaviour
    /// (no subprocess is spawned).
    pub format_code: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            format: Format::Plaintext,
            max_width: 0,
            use_neural: false,
            neural_lang: "en".to_string(),
            neural_model_path: None,
            extra_abbreviations: vec![],
            use_pandoc: false,
            pandoc_format: None,
            #[cfg(feature = "pandoc")]
            pandoc_backend: parser::pandoc::PandocBackend::default(),
            code: HashMap::new(),
            format_code: false,
        }
    }
}

/// Build the appropriate sentence splitter from config.
pub fn build_splitter(config: &FormatConfig) -> Result<Box<dyn SentenceSplitter>> {
    if config.use_neural {
        #[cfg(feature = "neural")]
        {
            let neural = if let Some(ref path) = config.neural_model_path {
                sentence::neural::NeuralSentenceSplitter::from_path_with_extras(
                    path,
                    &config.neural_lang,
                    &config.extra_abbreviations,
                )
            } else {
                sentence::neural::NeuralSentenceSplitter::with_extras(
                    &config.neural_lang,
                    &config.extra_abbreviations,
                )
            };
            Ok(Box::new(neural.map_err(|e| anyhow::anyhow!("{e}"))?))
        }
        #[cfg(not(feature = "neural"))]
        {
            Err(anyhow::anyhow!(
                "neural sentence splitting requires the 'neural' feature"
            ))
        }
    } else {
        Ok(Box::new(UnicodeSentenceSplitter::for_lang(
            &config.neural_lang,
            &config.extra_abbreviations,
        )))
    }
}

/// Format text with semantic line breaks.
pub fn format_text(input: &str, config: &FormatConfig) -> Result<String> {
    let splitter = build_splitter(config)?;
    format_text_with_splitter(input, config, splitter.as_ref())
}

/// Format text using a pre-constructed splitter (avoids reloading models per file).
pub fn format_text_with_splitter(
    input: &str,
    config: &FormatConfig,
    splitter: &dyn SentenceSplitter,
) -> Result<String> {
    let had_trailing_newline = input.ends_with('\n');
    let uses_crlf = input.contains("\r\n");

    // Normalize to LF for processing, restore CRLF at the end if needed.
    let normalized;
    let work_input = if uses_crlf {
        normalized = input.replace("\r\n", "\n");
        &normalized
    } else {
        input
    };

    // Two pipelines:
    // - use_pandoc: pandoc parses source → AST → regions by node kind → reflow prose only.
    // - else: native line parsers (markdown/org/…) then reflow. Never mixed after success.
    let regions = if config.use_pandoc {
        #[cfg(feature = "pandoc")]
        {
            let pandoc_fmt = config
                .pandoc_format
                .as_deref()
                .unwrap_or(match config.format {
                    Format::Org => "org",
                    Format::Latex => "latex",
                    Format::Markdown => "markdown",
                    Format::Rst => "rst",
                    Format::Plaintext => "markdown",
                });
            let parser =
                parser::pandoc::PandocParser::with_backend(pandoc_fmt, config.pandoc_backend);
            // Pandoc path: fail closed (no silent all-prose, no native re-parse).
            parser
                .try_parse(work_input)
                .map_err(|e| anyhow::anyhow!("{e}"))?
        }
        #[cfg(not(feature = "pandoc"))]
        {
            return Err(anyhow::anyhow!(
                "pandoc backend requires the 'pandoc' feature"
            ));
        }
    } else {
        parser::parser_for_format(config.format).parse(work_input)
    };

    let reflow_config = ReflowConfig {
        max_width: config.max_width,
        code: Some(&config.code),
        format_code: config.format_code,
    };

    let mut output = reflow(&regions, splitter, &reflow_config);

    // Preserve the original file's trailing newline convention.
    if had_trailing_newline && !output.ends_with('\n') {
        output.push('\n');
    } else if !had_trailing_newline {
        while output.ends_with('\n') {
            output.pop();
        }
    }

    // Restore CRLF if the input used it.
    if uses_crlf {
        output = output.replace('\n', "\r\n");
    }

    Ok(output)
}

/// Format only lines within a range (1-indexed, inclusive).
/// Lines outside the range pass through unchanged.
pub fn format_range(
    input: &str,
    config: &FormatConfig,
    start: usize,
    end: usize,
) -> Result<String> {
    let lines: Vec<&str> = input.lines().collect();
    let total = lines.len();

    // Clamp range
    let start = start.max(1);
    let end = end.min(total);

    if start > total {
        return Ok(input.to_string());
    }

    // Extract the range as a contiguous block
    let range_text = lines[start - 1..end].join("\n");
    let formatted = format_text(&range_text, config)?;

    // Reassemble: before + formatted + after
    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if line_num < start {
            result.push_str(line);
            result.push('\n');
        }
    }
    result.push_str(&formatted);
    if !formatted.ends_with('\n') && end < total {
        result.push('\n');
    }
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        if line_num > end {
            result.push_str(line);
            if line_num < total {
                result.push('\n');
            }
        }
    }

    // Preserve original trailing newline convention
    if input.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    } else if !input.ends_with('\n') {
        while result.ends_with('\n') {
            result.pop();
        }
    }

    Ok(result)
}
