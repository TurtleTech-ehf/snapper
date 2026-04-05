//! MCP (Model Context Protocol) server for snapper.
//!
//! Exposes formatting tools to AI assistants (Claude Desktop/Code, etc.)
//! via the standard MCP protocol on stdin/stdout.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, ServerHandler, ServiceExt, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::format::Format;
use crate::sentence::SentenceSplitter;
use crate::FormatConfig;

// -- Tool parameter types --

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FormatTextParams {
    /// Text to format with semantic line breaks.
    pub text: String,
    /// Document format: "org", "latex", "markdown", "rst", or "plaintext".
    #[serde(default = "default_format")]
    pub format: String,
    /// Maximum line width (0 = unlimited).
    #[serde(default)]
    pub max_width: usize,
    /// Extra abbreviations that should not trigger sentence breaks.
    #[serde(default)]
    pub extra_abbreviations: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DetectFormatParams {
    /// Text to analyze for format detection.
    pub text: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CheckFormattingParams {
    /// Text to check for semantic line break violations.
    pub text: String,
    /// Document format: "org", "latex", "markdown", "rst", or "plaintext".
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SplitSentencesParams {
    /// Text to split into individual sentences.
    pub text: String,
}

fn default_format() -> String {
    "plaintext".to_string()
}

fn parse_format(s: &str) -> Format {
    Format::from_extension(s)
}

fn make_config(format: Format, max_width: usize, extra_abbreviations: Vec<String>) -> FormatConfig {
    FormatConfig {
        format,
        max_width,
        extra_abbreviations,
        ..Default::default()
    }
}

// -- Response types --

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FormatTextResult {
    pub formatted: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DetectFormatResult {
    pub format: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CheckFormattingResult {
    /// Line numbers (1-indexed) containing multiple sentences.
    pub violations: Vec<usize>,
    /// Whether the text passes the formatting check.
    pub passed: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SplitSentencesResult {
    pub sentences: Vec<String>,
}

// -- Server --

pub struct SnapperMcpServer {
    tool_router: ToolRouter<Self>,
}

impl SnapperMcpServer {
    pub fn new() -> Self {
        let tool_router = Self::tool_router();
        Self { tool_router }
    }
}

#[tool_router]
impl SnapperMcpServer {
    #[tool(
        name = "format_text",
        description = "Format text with semantic line breaks. Each sentence is placed on its own line, producing minimal git diffs. Preserves code blocks, math, tables, and other structure."
    )]
    fn format_text(
        &self,
        Parameters(params): Parameters<FormatTextParams>,
    ) -> Result<Json<FormatTextResult>, rmcp::ErrorData> {
        let format = parse_format(&params.format);
        let config = make_config(format, params.max_width, params.extra_abbreviations);
        match crate::format_text(&params.text, &config) {
            Ok(formatted) => Ok(Json(FormatTextResult { formatted })),
            Err(e) => Err(rmcp::ErrorData::internal_error(
                format!("formatting failed: {e}"),
                None,
            )),
        }
    }

    #[tool(
        name = "detect_format",
        description = "Detect the document format of text using content heuristics. Returns one of: org, latex, markdown, rst, plaintext."
    )]
    fn detect_format(
        &self,
        Parameters(params): Parameters<DetectFormatParams>,
    ) -> Json<DetectFormatResult> {
        let format = detect_format_heuristic(&params.text);
        Json(DetectFormatResult {
            format: format_name(format),
        })
    }

    #[tool(
        name = "check_formatting",
        description = "Check text for semantic line break violations. Returns line numbers where multiple sentences appear on a single line."
    )]
    fn check_formatting(
        &self,
        Parameters(params): Parameters<CheckFormattingParams>,
    ) -> Json<CheckFormattingResult> {
        let format = parse_format(&params.format);
        let config = make_config(format, 0, vec![]);
        let splitter = crate::build_splitter(&config).unwrap();
        let violations = find_violations(&params.text, splitter.as_ref());
        let passed = violations.is_empty();
        Json(CheckFormattingResult {
            violations,
            passed,
        })
    }

    #[tool(
        name = "split_sentences",
        description = "Split text into individual sentences using Unicode-aware sentence boundary detection with abbreviation handling."
    )]
    fn split_sentences(
        &self,
        Parameters(params): Parameters<SplitSentencesParams>,
    ) -> Json<SplitSentencesResult> {
        let config = FormatConfig::default();
        let splitter = crate::build_splitter(&config).unwrap();
        let sentences = splitter.split(&params.text);
        Json(SplitSentencesResult { sentences })
    }
}

impl ServerHandler for SnapperMcpServer {}

// -- Helpers --

/// Heuristic format detection from text content.
fn detect_format_heuristic(input: &str) -> Format {
    let lines: Vec<&str> = input.lines().take(20).collect();

    if input.contains("\\begin{")
        || input.contains("\\section{")
        || input.contains("\\documentclass")
    {
        return Format::Latex;
    }

    if lines
        .iter()
        .any(|l| l.starts_with("#+") || l.starts_with("* "))
    {
        if input.contains(":PROPERTIES:")
            || input.contains(":END:")
            || input.contains("#+begin_")
        {
            return Format::Org;
        }
    }

    if lines
        .iter()
        .any(|l| l.starts_with("# ") || l.starts_with("## "))
    {
        return Format::Markdown;
    }

    if input.contains(".. ")
        || lines
            .iter()
            .any(|l| l.chars().all(|c| c == '=' || c == '-') && l.len() > 3)
    {
        return Format::Rst;
    }

    Format::Plaintext
}

fn format_name(f: Format) -> String {
    match f {
        Format::Org => "org",
        Format::Latex => "latex",
        Format::Markdown => "markdown",
        Format::Rst => "rst",
        Format::Plaintext => "plaintext",
    }
    .to_string()
}

/// Find lines containing multiple sentences.
fn find_violations(input: &str, splitter: &dyn SentenceSplitter) -> Vec<usize> {
    let mut violations = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let sentences = splitter.split(trimmed);
        if sentences.len() > 1 {
            violations.push(idx + 1);
        }
    }
    violations
}

/// Run the MCP server on stdin/stdout.
pub async fn run_mcp() -> anyhow::Result<()> {
    let server = SnapperMcpServer::new();
    let transport = rmcp::transport::io::stdio();
    let running = server.serve(transport).await
        .map_err(|e| anyhow::anyhow!("MCP server failed to start: {e}"))?;
    running.waiting().await?;
    Ok(())
}
