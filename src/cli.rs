use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FormatArg {
    Org,
    Latex,
    Markdown,
    Plaintext,
}

#[derive(Debug, Parser)]
#[command(name = "snapper", version, about = "Semantic line break formatter")]
pub struct Cli {
    /// Input files. Reads stdin if omitted.
    #[arg()]
    pub files: Vec<PathBuf>,

    /// Input format (auto-detected from extension if omitted).
    #[arg(short, long)]
    pub format: Option<FormatArg>,

    /// Output file (stdout if omitted).
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Modify files in place.
    #[arg(short, long)]
    pub in_place: bool,

    /// Maximum line width (0 = unlimited).
    #[arg(short = 'w', long, default_value_t = 0)]
    pub max_width: usize,

    /// Use neural sentence detection (requires neural feature).
    #[arg(long)]
    pub neural: bool,

    /// Exit with code 1 if any file would change.
    #[arg(long)]
    pub check: bool,

    /// Show a unified diff of what would change.
    #[arg(long)]
    pub diff: bool,

    /// Path to config file (default: .snapperrc.toml in current or parent dirs).
    #[arg(long)]
    pub config: Option<PathBuf>,
}
