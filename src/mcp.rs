//! MCP (Model Context Protocol) server for snapper.
//!
//! Exposes formatting tools to MCP clients via the standard MCP protocol
//! on stdin/stdout.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, ServerHandler, ServiceExt, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::FormatConfig;
use crate::check::{DiagnosticKind, collect_diagnostics, resolve_long_threshold, would_reformat};
use crate::format::Format;

// -- Tool parameter types --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
pub struct LineRange {
    /// 1-indexed inclusive start line.
    pub start: usize,
    /// 1-indexed inclusive end line.
    pub end: usize,
}

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
    /// Prefer soft breaks after independent-clause punctuation when wrapping
    /// under `max_width` (same as CLI `--clause-breaks`).
    #[serde(default)]
    pub clause_breaks: bool,
    /// Optional 1-indexed inclusive line range. Same meaning as CLI `--range`.
    #[serde(default)]
    pub range: Option<LineRange>,
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
    /// Maximum line width (0 = unlimited). Used as the `long` threshold when set.
    #[serde(default)]
    pub max_width: usize,
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

fn make_config(
    format: Format,
    max_width: usize,
    extra_abbreviations: Vec<String>,
    clause_breaks: bool,
) -> FormatConfig {
    FormatConfig {
        format,
        max_width,
        extra_abbreviations,
        clause_breaks,
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
pub struct LineDiagnosticDto {
    /// 1-indexed source line.
    pub line: usize,
    /// `fused`, `wrap`, or `long`.
    pub kind: String,
    /// Source line excerpt.
    pub excerpt: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CheckFormattingResult {
    /// Line numbers (1-indexed) containing multiple sentences (fused).
    pub violations: Vec<usize>,
    /// Whether the text matches formatted output (same as CLI `--check` without `--strict-long`).
    pub passed: bool,
    /// True when `format_text` would change the input. Identical to CLI `--check`.
    pub would_reformat: bool,
    /// Line-level fused / wrap / long diagnostics.
    pub diagnostics: Vec<LineDiagnosticDto>,
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
        description = "Format text with semantic line breaks. Each sentence is placed on its own line, producing minimal git diffs. Preserves math, tables, and other structure; source-block fences stay fixed while configured language comments reflow (optional external formatters are CLI-only via --format-code). Supports clause_breaks and an optional 1-indexed range (same as the CLI)."
    )]
    fn format_text(
        &self,
        Parameters(params): Parameters<FormatTextParams>,
    ) -> Result<Json<FormatTextResult>, rmcp::ErrorData> {
        let format = parse_format(&params.format);
        let config = make_config(
            format,
            params.max_width,
            params.extra_abbreviations,
            params.clause_breaks,
        );
        let result = if let Some(range) = params.range {
            crate::format_range(&params.text, &config, range.start, range.end)
        } else {
            crate::format_text(&params.text, &config)
        };
        match result {
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
        description = "Check text for semantic line break violations. Returns would_reformat (identical to CLI --check), line diagnostics (fused/wrap/long), and fused line numbers."
    )]
    fn check_formatting(
        &self,
        Parameters(params): Parameters<CheckFormattingParams>,
    ) -> Json<CheckFormattingResult> {
        let format = parse_format(&params.format);
        let config = make_config(format, params.max_width, vec![], false);
        let splitter = crate::build_splitter(&config).unwrap();
        let would = would_reformat(&params.text, &config).unwrap_or(true);
        let threshold = resolve_long_threshold(params.max_width, None);
        let diagnostics = collect_diagnostics(
            &params.text,
            format,
            splitter.as_ref(),
            threshold,
            Some(&config),
        );
        let violations: Vec<usize> = diagnostics
            .iter()
            .filter(|d| d.kind == DiagnosticKind::Fused)
            .map(|d| d.line)
            .collect();
        let dto = diagnostics
            .into_iter()
            .map(|d| LineDiagnosticDto {
                line: d.line,
                kind: d.kind.as_str().to_string(),
                excerpt: d.excerpt,
            })
            .collect();
        Json(CheckFormattingResult {
            violations,
            passed: !would,
            would_reformat: would,
            diagnostics: dto,
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
        if input.contains(":PROPERTIES:") || input.contains(":END:") || input.contains("#+begin_") {
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

/// Run the MCP server on stdin/stdout.
pub async fn run_mcp() -> anyhow::Result<()> {
    let server = SnapperMcpServer::new();
    let transport = rmcp::transport::io::stdio();
    let running = server
        .serve(transport)
        .await
        .map_err(|e| anyhow::anyhow!("MCP server failed to start: {e}"))?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(params: FormatTextParams) -> String {
        let server = SnapperMcpServer::new();
        server
            .format_text(Parameters(params))
            .expect("format_text")
            .0
            .formatted
    }

    fn plaintext(text: &str) -> FormatTextParams {
        FormatTextParams {
            text: text.to_string(),
            format: "plaintext".to_string(),
            max_width: 0,
            extra_abbreviations: vec![],
            clause_breaks: false,
            range: None,
        }
    }

    fn check(text: &str) -> CheckFormattingResult {
        let server = SnapperMcpServer::new();
        server
            .check_formatting(Parameters(CheckFormattingParams {
                text: text.to_string(),
                format: "plaintext".to_string(),
                max_width: 0,
            }))
            .0
    }

    #[test]
    fn default_features_include_mcp() {
        let manifest = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let after = manifest
            .split("[features]")
            .nth(1)
            .expect("Cargo.toml [features]");
        let default_line = after
            .lines()
            .find(|l| l.starts_with("default"))
            .expect("default = [...]");
        assert!(
            default_line.contains("\"mcp\""),
            "default features must include mcp so release binaries ship the server: {default_line}"
        );
    }

    #[test]
    fn dist_workspace_does_not_strip_mcp() {
        let dist = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/dist-workspace.toml"));
        assert!(
            !dist.contains("no-default-features")
                && !dist.contains("default-features")
                && !dist.lines().any(|l| l.contains("features")
                    && !l.contains("cargo-dist-version")
                    && !l.trim_start().starts_with('#')),
            "dist-workspace.toml must not override default features (mcp ships via Cargo.toml default)"
        );
    }

    #[test]
    fn format_text_params_max_width_defaults_to_zero() {
        let params: FormatTextParams = serde_json::from_str(r#"{"text":"Hi."}"#).unwrap();
        assert_eq!(params.max_width, 0);
        assert!(!params.clause_breaks);
        assert!(params.range.is_none());
    }

    #[test]
    fn format_text_params_accept_clause_breaks_range_and_max_width() {
        let params: FormatTextParams = serde_json::from_str(
            r#"{
                "text": "Hi.",
                "clause_breaks": true,
                "range": {"start": 2, "end": 3},
                "max_width": 80
            }"#,
        )
        .unwrap();
        assert!(params.clause_breaks);
        assert_eq!(params.range, Some(LineRange { start: 2, end: 3 }));
        assert_eq!(params.max_width, 80);
    }

    #[test]
    fn format_text_clause_breaks_wraps_after_commas() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let mut params = plaintext(sentence);
        params.max_width = 80;
        params.clause_breaks = true;
        let out = format(params);
        assert!(
            out.contains("orchestrated,\nalong with"),
            "clause_breaks must break after first comma: {out:?}"
        );
        assert!(
            out.contains("plan,\nwithout"),
            "clause_breaks must break after second comma: {out:?}"
        );
    }

    #[test]
    fn format_text_clause_breaks_noop_without_max_width() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let mut params = plaintext(sentence);
        params.clause_breaks = true;
        let out = format(params);
        assert!(
            !out.contains("orchestrated,\n"),
            "unlimited max_width must not clause-break: {out:?}"
        );
    }

    #[test]
    fn format_text_range_formats_only_specified_lines() {
        let mut params = plaintext(
            "Line one. Stay same.\nLine two. Should split. Into two.\nLine three. Stay same.\n",
        );
        params.range = Some(LineRange { start: 2, end: 2 });
        let out = format(params);
        assert!(
            out.starts_with("Line one. Stay same.\n"),
            "lines before range stay: {out:?}"
        );
        assert!(
            out.contains("Line two.\nShould split.\nInto two.\n"),
            "range line must reflow: {out:?}"
        );
        assert!(
            out.ends_with("Line three. Stay same.\n"),
            "lines after range stay: {out:?}"
        );
    }

    #[test]
    fn check_formatting_would_reformat_matches_cli_check() {
        let fused = check("Hello world. This is a test.\n");
        assert!(
            fused.would_reformat,
            "fused input must match CLI --check dirty"
        );
        assert!(!fused.passed);
        assert_eq!(fused.violations, vec![1]);

        let ok = check("Hello world.\nThis is a test.\n");
        assert!(
            !ok.would_reformat,
            "already-formatted input must match CLI --check clean"
        );
        assert!(ok.passed);
        assert!(ok.violations.is_empty());
    }
}
