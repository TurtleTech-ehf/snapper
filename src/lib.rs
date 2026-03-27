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
    } else {
        Box::new(UnicodeSentenceSplitter)
    };

    let regions = parser.parse(input);
    let reflow_config = ReflowConfig {
        max_width: config.max_width,
    };

    Ok(reflow(&regions, splitter.as_ref(), &reflow_config))
}
