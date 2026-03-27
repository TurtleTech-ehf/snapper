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
//! - **Org-mode**: blocks, drawers, tables, keywords preserved
//! - **LaTeX**: preamble, math, environments, comments preserved
//! - **Markdown**: code blocks, front matter, headings preserved
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
//!     max_width: 0,
//!     use_neural: false,
//!     extra_abbreviations: vec![],
//! };
//! let output = format_text(input, &config).unwrap();
//! assert_eq!(output, "Hello world.\nThis is a test.\nAnother sentence.");
//! ```

pub mod abbreviations;
pub mod cli;
pub mod config;
pub mod format;
pub mod parser;
pub mod reflow;
pub mod sentence;

use anyhow::Result;

use crate::format::Format;
use crate::parser::FormatParser;
use crate::parser::latex::LatexParser;
use crate::parser::markdown::MarkdownParser;
use crate::parser::org::OrgParser;
use crate::parser::plaintext::PlaintextParser;
use crate::reflow::{ReflowConfig, reflow};
use crate::sentence::SentenceSplitter;
use crate::sentence::unicode::UnicodeSentenceSplitter;

/// Configuration for the formatting pipeline.
pub struct FormatConfig {
    pub format: Format,
    pub max_width: usize,
    pub use_neural: bool,
    /// Extra abbreviations from project config.
    pub extra_abbreviations: Vec<String>,
}

/// Format text with semantic line breaks.
pub fn format_text(input: &str, config: &FormatConfig) -> Result<String> {
    let parser: Box<dyn FormatParser> = match config.format {
        Format::Org => Box::new(OrgParser),
        Format::Latex => Box::new(LatexParser),
        Format::Markdown => Box::new(MarkdownParser),
        Format::Plaintext => Box::new(PlaintextParser),
    };

    let splitter: Box<dyn SentenceSplitter> = if config.use_neural {
        #[cfg(feature = "neural")]
        {
            // Neural splitter would go here
            anyhow::bail!("Neural splitter not yet implemented");
        }
        #[cfg(not(feature = "neural"))]
        {
            anyhow::bail!(
                "Neural sentence detection requires the 'neural' feature. \
                 Build with: cargo build --features neural"
            );
        }
    } else if config.extra_abbreviations.is_empty() {
        Box::new(UnicodeSentenceSplitter::new())
    } else {
        Box::new(UnicodeSentenceSplitter::with_extra_abbreviations(
            &config.extra_abbreviations,
        ))
    };

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

    let regions = parser.parse(work_input);
    let reflow_config = ReflowConfig {
        max_width: config.max_width,
    };

    let mut output = reflow(&regions, splitter.as_ref(), &reflow_config);

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
