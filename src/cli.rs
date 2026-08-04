use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::diff::ColorMode;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FormatArg {
    Org,
    Latex,
    Markdown,
    Rst,
    Plaintext,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Sarif,
}

/// Control when colored output is used (ruff-style).
#[derive(Debug, Clone, Copy, ValueEnum, Default, PartialEq, Eq)]
pub enum ColorWhen {
    /// Display colors if the output goes to an interactive terminal.
    #[default]
    Auto,
    /// Always display colors.
    Always,
    /// Never display colors.
    Never,
}

impl From<ColorWhen> for ColorMode {
    fn from(value: ColorWhen) -> Self {
        match value {
            ColorWhen::Auto => ColorMode::Auto,
            ColorWhen::Always => ColorMode::Always,
            ColorWhen::Never => ColorMode::Never,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "snapper", version, about = "Semantic line break formatter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Input files. Reads stdin if omitted.
    #[arg()]
    pub files: Vec<PathBuf>,

    /// Input format (auto-detected from extension if omitted).
    #[arg(short, long)]
    pub format: Option<FormatArg>,

    /// Assume this filename when reading stdin (for format auto-detection).
    #[arg(long)]
    pub stdin_filepath: Option<PathBuf>,

    /// Output file (stdout if omitted).
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Modify files in place.
    #[arg(short, long)]
    pub in_place: bool,

    /// Maximum line width (0 = unlimited).
    #[arg(short = 'w', long, default_value_t = 0)]
    pub max_width: usize,

    /// Prefer soft breaks after independent-clause punctuation (comma,
    /// semicolon, colon, em dash) when wrapping under `--max-width`.
    #[arg(long, default_value_t = false)]
    pub clause_breaks: bool,

    /// Use neural sentence detection (nnsplit LSTM model).
    #[arg(long)]
    pub neural: bool,

    /// Language for neural sentence detection (default: en).
    /// Available: en, de, fr, no, sv, zh, tr, ru, uk.
    #[arg(long)]
    pub lang: Option<String>,

    /// Path to custom ONNX model file for neural detection.
    #[arg(long)]
    pub model_path: Option<PathBuf>,

    /// Use pandoc as parser backend (universal format support).
    #[arg(long)]
    pub use_pandoc: bool,

    /// Pandoc AST source when `--use-pandoc` is set:
    /// `auto` (prefer in-process FFI, else CLI), `ffi` (`libsnapper_pandoc`),
    /// or `cli` (`pandoc` subprocess). Default: `auto`.
    /// `ffi` fails explicitly if the library is missing.
    #[arg(long, default_value = "auto", value_name = "BACKEND")]
    pub pandoc_backend: String,

    /// Exit with code 1 if any file would change.
    #[arg(long)]
    pub check: bool,

    /// Show a unified diff of what would change.
    #[arg(long)]
    pub diff: bool,

    /// Control when colored output is used.
    ///
    /// Possible values:
    /// - auto:   Display colors if the output goes to an interactive terminal
    ///   and the `NO_COLOR` environment variable is unset
    /// - always: Always display colors
    /// - never:  Never display colors
    #[arg(long, value_enum, default_value_t = ColorWhen::Auto, global = true, value_name = "WHEN")]
    pub color: ColorWhen,

    /// Path to config file (default: .snapperrc.toml in current or parent dirs).
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Only format lines in this range (1-indexed, inclusive). Format: START:END.
    #[arg(long)]
    pub range: Option<String>,

    /// Output format for --check mode.
    #[arg(long, default_value = "text")]
    pub output_format: OutputFormat,

    /// Pipe each code block's body through the per-language formatter
    /// configured under `[code.<lang>.formatter]` in `.snapperrc.toml`.
    /// The formatter runs after the in-block comment reflow. Missing
    /// binaries, non-zero exits, and timeouts surface as stderr
    /// diagnostics; snapper still exits 0.
    #[arg(long, default_value_t = false)]
    pub format_code: bool,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize snapper for a project (generate config, pre-commit, gitattributes).
    Init {
        /// Preview what would be generated without writing files.
        #[arg(long)]
        dry_run: bool,
    },
    /// Sentence-level diff between two files.
    Sdiff {
        /// Original file.
        old: PathBuf,
        /// Modified file.
        new: PathBuf,
        /// Input format (auto-detected from extension if omitted).
        #[arg(short, long)]
        format: Option<FormatArg>,
        /// Disable colored output (alias for `--color never`).
        #[arg(long)]
        no_color: bool,
    },
    /// Sentence-level diff against a git ref.
    GitDiff {
        /// Git ref to compare against (default: HEAD).
        #[arg(default_value = "HEAD")]
        git_ref: String,
        /// Files to diff. If omitted, diffs all changed prose files.
        #[arg()]
        files: Vec<PathBuf>,
        /// Input format (auto-detected from extension if omitted).
        #[arg(short, long)]
        format: Option<FormatArg>,
        /// Disable colored output (alias for `--color never`).
        #[arg(long)]
        no_color: bool,
    },
    /// Start the LSP server (stdin/stdout).
    Lsp,
    /// Start the MCP server (stdin/stdout).
    Mcp,
    /// Watch files and reformat on change.
    Watch {
        /// Files or glob patterns to watch.
        #[arg(required = true)]
        patterns: Vec<String>,
        /// Input format (auto-detected from extension if omitted).
        #[arg(short, long)]
        format: Option<FormatArg>,
    },
}

/// Parse a range string "START:END" into (start, end) 1-indexed inclusive.
pub fn parse_range(s: &str) -> Option<(usize, usize)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let start = parts[0].parse::<usize>().ok()?;
    let end = parts[1].parse::<usize>().ok()?;
    if start == 0 || end == 0 || start > end {
        return None;
    }
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_valid() {
        assert_eq!(parse_range("1:10"), Some((1, 10)));
        assert_eq!(parse_range("5:5"), Some((5, 5)));
        assert_eq!(parse_range("1:1"), Some((1, 1)));
    }

    #[test]
    fn parse_range_zero_rejected() {
        assert_eq!(parse_range("0:5"), None);
        assert_eq!(parse_range("5:0"), None);
        assert_eq!(parse_range("0:0"), None);
    }

    #[test]
    fn parse_range_reversed_rejected() {
        assert_eq!(parse_range("10:5"), None);
    }

    #[test]
    fn parse_range_bad_format() {
        assert_eq!(parse_range("abc"), None);
        assert_eq!(parse_range("1:2:3"), None);
        assert_eq!(parse_range(""), None);
        assert_eq!(parse_range("a:b"), None);
        assert_eq!(parse_range(":5"), None);
        assert_eq!(parse_range("5:"), None);
    }
}
