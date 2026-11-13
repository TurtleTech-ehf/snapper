//! MCP (Model Context Protocol) server for snapper.
//!
//! Exposes formatting tools to MCP clients via the standard MCP protocol
//! on stdin/stdout.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{Json, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
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
    /// Prefer soft breaks after independent-clause punctuation
    /// (same as CLI `--clause-breaks`).
    /// `max_width` 0 always breaks at whitespace after the punctuation;
    /// `max_width` greater than 0 is wrap-prefer.
    #[serde(default)]
    pub clause_breaks: bool,
    /// Open a sentence at a lowercase word after a terminator
    /// (same as CLI `--lowercase-starts`).
    #[serde(default)]
    pub lowercase_starts: bool,
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
    /// Prefer soft breaks after independent-clause punctuation
    /// (same as CLI `--clause-breaks`; default false).
    /// `max_width` 0 always breaks at whitespace after the punctuation;
    /// `max_width` greater than 0 is wrap-prefer.
    #[serde(default)]
    pub clause_breaks: bool,
    /// Open a sentence at a lowercase word after a terminator
    /// (same as CLI `--lowercase-starts`; default false).
    #[serde(default)]
    pub lowercase_starts: bool,
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
    lowercase_starts: bool,
) -> FormatConfig {
    FormatConfig {
        format,
        max_width,
        extra_abbreviations,
        clause_breaks,
        lowercase_starts,
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
    /// Read by `#[tool_handler]` for `list_tools` / `call_tool`.
    /// Clippy does not see the macro use, so this is not dead.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl SnapperMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for SnapperMcpServer {
    fn default() -> Self {
        Self::new()
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
            params.lowercase_starts,
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
        description = "Check text for semantic line break violations. Honors clause_breaks (same two-mode contract as format_text). Returns would_reformat (identical to CLI --check), line diagnostics (fused/wrap/long), and fused line numbers."
    )]
    fn check_formatting(
        &self,
        Parameters(params): Parameters<CheckFormattingParams>,
    ) -> Json<CheckFormattingResult> {
        let format = parse_format(&params.format);
        let config = make_config(
            format,
            params.max_width,
            vec![],
            params.clause_breaks,
            params.lowercase_starts,
        );
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

#[tool_handler(
    name = "snapper",
    instructions = "Semantic line-break formatter. Tools: format_text, detect_format, check_formatting, split_sentences."
)]
impl ServerHandler for SnapperMcpServer {}

// -- Helpers --

/// How many leading lines the structure scan reads.
/// A citation or a section command can sit further down; those use the
/// whole buffer. Headlines and headings are decided from the opening.
const DETECT_LINE_LIMIT: usize = 40;

/// Heuristic format detection from text content.
///
/// Org and Markdown structure are decided before LaTeX commands.
/// A note that mentions `\section` or `\cite` is still an Org headline
/// or a Markdown heading. Detecting it as LaTeX joins that line onto
/// the next sentence.
fn detect_format_heuristic(input: &str) -> Format {
    let lines: Vec<&str> = input.lines().take(DETECT_LINE_LIMIT).collect();
    if looks_like_org(input, &lines) {
        return Format::Org;
    }
    if looks_like_markdown(input, &lines) {
        return Format::Markdown;
    }
    if has_latex_signal(input) {
        return Format::Latex;
    }
    if looks_like_rst(input, &lines) {
        return Format::Rst;
    }
    if lines.iter().any(|line| line_is_markdown_list_item(line)) {
        return Format::Markdown;
    }
    Format::Plaintext
}

/// `#+KEY:` / `#+BEGIN_SRC` at any indent. `#+ not a keyword` is prose.
fn line_is_org_keyword(line: &str) -> bool {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let Some(rest) = trimmed.strip_prefix("#+") else {
        return false;
    };
    rest.starts_with(|c: char| c.is_ascii_alphabetic())
}

/// Column-0 stars, a space, then a title. `**bold**` has no space.
fn line_is_org_headline(line: &str) -> bool {
    let stars = line.chars().take_while(|c| *c == '*').count();
    if stars == 0 {
        return false;
    }
    let rest = &line[stars..];
    let trimmed = rest.trim_start_matches([' ', '\t']);
    trimmed.len() != rest.len() && !trimmed.is_empty()
}

fn line_is_org_todo_headline(line: &str) -> bool {
    if !line_is_org_headline(line) {
        return false;
    }
    let stars = line.chars().take_while(|c| *c == '*').count();
    let after = line[stars..].trim_start();
    ["TODO ", "DONE ", "NEXT ", "WAIT "]
        .iter()
        .any(|keyword| after.starts_with(keyword))
}

/// `-` / `*` / `+` with 0–3 leading spaces. Column-0 `*` is also a headline.
fn line_is_unordered_item(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut indent = 0;
    while indent < 3 && indent < bytes.len() && bytes[indent] == b' ' {
        indent += 1;
    }
    let rest = &line[indent..];
    let Some(after) = rest
        .strip_prefix('-')
        .or_else(|| rest.strip_prefix('*'))
        .or_else(|| rest.strip_prefix('+'))
    else {
        return false;
    };
    after.is_empty() || after.starts_with([' ', '\t'])
}

fn leading_marker_indent(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut indent = 0;
    while indent < 3 && indent < bytes.len() && bytes[indent] == b' ' {
        indent += 1;
    }
    indent
}

/// CommonMark list marker, including `1.` / `1)`. Unordered markers share
/// `line_is_unordered_item`.
fn line_is_markdown_list_item(line: &str) -> bool {
    if line_is_unordered_item(line) {
        return true;
    }
    let rest = &line[leading_marker_indent(line)..];
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if !(1..=9).contains(&digits) {
        return false;
    }
    let tail = &rest[digits..];
    let Some(after) = tail.strip_prefix('.').or_else(|| tail.strip_prefix(')')) else {
        return false;
    };
    after.is_empty() || after.starts_with([' ', '\t'])
}

/// CommonMark ATX heading: 0–3 spaces, 1–6 `#`, then space or end.
/// `#+title:` is a keyword, not a heading.
fn line_is_atx_heading(line: &str) -> bool {
    let rest = &line[leading_marker_indent(line)..];
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return false;
    }
    let after = &rest[hashes..];
    after.is_empty() || after.starts_with([' ', '\t'])
}

fn line_is_fence(line: &str) -> bool {
    let rest = &line[leading_marker_indent(line)..];
    let marker = rest.chars().next();
    if marker != Some('`') && marker != Some('~') {
        return false;
    }
    rest.chars().take_while(|c| Some(*c) == marker).count() >= 3
}

/// A headline with a following column-0 paragraph is an Org entry.
/// The next line of a Markdown list is another marker, or it is indented.
fn org_headline_followed_by_prose(lines: &[&str]) -> bool {
    for (i, line) in lines.iter().enumerate() {
        if !line_is_org_headline(line) {
            continue;
        }
        let Some(next) = lines[i + 1..]
            .iter()
            .copied()
            .find(|candidate| !candidate.trim().is_empty())
        else {
            continue;
        };
        if next.starts_with([' ', '\t'])
            || line_is_org_headline(next)
            || line_is_unordered_item(next)
        {
            continue;
        }
        return true;
    }
    false
}

fn looks_like_org(input: &str, lines: &[&str]) -> bool {
    if lines.iter().any(|line| line_is_org_keyword(line)) {
        return true;
    }
    if input.contains("[cite:") || input.contains(":PROPERTIES:") {
        return true;
    }
    if lines.iter().any(|line| {
        line_is_org_todo_headline(line) || (line_is_org_headline(line) && line.starts_with("**"))
    }) {
        return true;
    }
    org_headline_followed_by_prose(lines)
}

/// Pandoc citation `[@key]` / `[see @key]`. Org `[cite:@key]` is not one.
fn has_pandoc_citation(input: &str) -> bool {
    let mut rest = input;
    while let Some(start) = rest.find('[') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find(']') else {
            break;
        };
        let inner = &rest[..end];
        if !inner.contains('\n') && inner.contains('@') && !inner.starts_with("cite") {
            return true;
        }
        rest = &rest[end + 1..];
    }
    false
}

fn looks_like_markdown(input: &str, lines: &[&str]) -> bool {
    lines
        .iter()
        .any(|line| line_is_atx_heading(line) || line_is_fence(line))
        || has_pandoc_citation(input)
        || input.contains("](")
}

fn has_latex_signal(input: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "\\documentclass",
        "\\begin{",
        "\\end{",
        "\\section",
        "\\subsection",
        "\\chapter",
        "\\cite",
        "\\ref{",
        "\\eqref",
    ];
    NEEDLES.iter().any(|needle| input.contains(needle))
}

fn looks_like_rst(input: &str, lines: &[&str]) -> bool {
    input.contains(".. ")
        || lines
            .iter()
            .any(|line| line.len() > 3 && line.chars().all(|c| c == '=' || c == '-'))
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
            lowercase_starts: false,
            range: None,
        }
    }

    fn check(text: &str) -> CheckFormattingResult {
        check_with(text, false)
    }

    fn check_with(text: &str, clause_breaks: bool) -> CheckFormattingResult {
        let server = SnapperMcpServer::new();
        server
            .check_formatting(Parameters(CheckFormattingParams {
                text: text.to_string(),
                format: "plaintext".to_string(),
                max_width: 0,
                clause_breaks,
                lowercase_starts: false,
            }))
            .0
    }

    #[test]
    fn server_info_advertises_tools_and_snapper_name() {
        let info = ServerHandler::get_info(&SnapperMcpServer::new());
        assert!(
            info.capabilities.tools.is_some(),
            "initialize must advertise tools, got {:?}",
            info.capabilities
        );
        assert_eq!(
            info.server_info.name, "snapper",
            "clients should see snapper, not the rmcp default"
        );
    }

    #[test]
    fn tool_router_lists_the_four_tools() {
        let names: Vec<String> = SnapperMcpServer::new()
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        for expected in [
            "format_text",
            "detect_format",
            "check_formatting",
            "split_sentences",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "missing {expected} in {names:?}"
            );
        }
    }

    #[test]
    fn mcp_feature_enables_tokio() {
        let manifest = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let after = manifest
            .split("[features]")
            .nth(1)
            .expect("Cargo.toml [features]");
        let mcp_line = after
            .lines()
            .find(|l| l.starts_with("mcp ") || l.starts_with("mcp="))
            .expect("mcp = [...]");
        assert!(
            mcp_line.contains("\"dep:tokio\"") || mcp_line.contains("\"tokio\""),
            "mcp must enable tokio so `cargo build --no-default-features --features cli,mcp` resolves the runtime: {mcp_line}"
        );
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
    fn format_text_lowercase_starts_splits_a_tool_name() {
        let mut params = plaintext("There are two. setup-python moved to PyPy 8.0.0.");
        params.format = "markdown".to_string();
        params.lowercase_starts = true;
        let out = format(params);
        assert_eq!(out, "There are two.\nsetup-python moved to PyPy 8.0.0.");
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
    fn format_text_clause_breaks_unlimited_breaks_after_commas() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let mut params = plaintext(sentence);
        params.clause_breaks = true;
        let out = format(params);
        assert!(
            out.contains("orchestrated,\nalong with"),
            "clause_breaks with max_width 0 must break after first comma: {out:?}"
        );
        assert!(
            out.contains("plan,\nwithout"),
            "clause_breaks with max_width 0 must break after second comma: {out:?}"
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

    #[test]
    fn check_formatting_params_clause_breaks_defaults_false() {
        let params: CheckFormattingParams = serde_json::from_str(r#"{"text":"Hi."}"#).unwrap();
        assert!(!params.clause_breaks);
        assert_eq!(params.max_width, 0);
    }

    #[test]
    fn check_formatting_clause_breaks_matches_format_text() {
        let fused = check_with("Hello, world.\n", true);
        assert!(
            fused.would_reformat,
            "fused clause with clause_breaks must be would_reformat"
        );

        let broken = check_with("Hello,\nworld.\n", true);
        assert!(
            !broken.would_reformat,
            "already-broken clauses must be clean"
        );
    }

    fn detect(text: &str) -> String {
        let server = SnapperMcpServer::new();
        server
            .detect_format(Parameters(DetectFormatParams {
                text: text.to_string(),
            }))
            .0
            .format
    }

    fn format_as(text: &str, format_name: &str) -> String {
        let mut params = plaintext(text);
        params.format = format_name.to_string();
        format(params)
    }

    fn check_as(text: &str, format_name: &str) -> CheckFormattingResult {
        let server = SnapperMcpServer::new();
        server
            .check_formatting(Parameters(CheckFormattingParams {
                text: text.to_string(),
                format: format_name.to_string(),
                max_width: 0,
                clause_breaks: false,
                lowercase_starts: false,
            }))
            .0
    }

    fn assert_session(text: &str, format_name: &str, expected: &str) {
        assert_eq!(
            detect(text),
            format_name,
            "detect_format returned the wrong format"
        );
        let formatted = format_as(text, format_name);
        assert_eq!(formatted, expected, "format_text changed structure");
        let again = format_as(&formatted, format_name);
        assert_eq!(again, formatted, "format_text was not idempotent");
        let report = check_as(&formatted, format_name);
        assert!(
            report.passed && !report.would_reformat && report.violations.is_empty(),
            "check_formatting rejected the formatted text: {report:?}"
        );
    }

    #[test]
    fn session_org_keeps_headline_cite_math_and_abbreviations() {
        let text = concat!(
            "* Methods\n",
            "Dr. Smith showed that the estimate holds for smooth data, e.g. when the mesh is refined. ",
            "The leading term is \\(x = 1.5\\). ",
            "Details are in [cite:@smith2020;@jones2021]. ",
            "A later sentence stays fused on this line.\n",
            "The export names \\section{Results. More title.} in the latex block.\n",
        );
        let expected = concat!(
            "* Methods\n",
            "Dr. Smith showed that the estimate holds for smooth data, e.g. when the mesh is refined.\n",
            "The leading term is \\(x = 1.5\\).\n",
            "Details are in [cite:@smith2020;@jones2021].\n",
            "A later sentence stays fused on this line.\n",
            "The export names \\section{Results. More title.} in the latex block.\n",
        );
        assert_session(text, "org", expected);
    }

    #[test]
    fn session_markdown_keeps_heading_cite_math_and_abbreviations() {
        let text = concat!(
            "### Methods. With two sentences.\n",
            "\n",
            "Dr. Smith showed that the estimate holds for smooth data, e.g. when the mesh is refined. ",
            "The leading term is $O(h^{2})$. ",
            "Details are in [@smith2020; @jones2021]. ",
            "A later sentence stays fused on this line.\n",
            "People write \\section{Results. More title.} in prose.\n",
        );
        let expected = concat!(
            "### Methods. With two sentences.\n",
            "\n",
            "Dr. Smith showed that the estimate holds for smooth data, e.g. when the mesh is refined.\n",
            "The leading term is $O(h^{2})$.\n",
            "Details are in [@smith2020; @jones2021].\n",
            "A later sentence stays fused on this line.\n",
            "People write \\section{Results. More title.} in prose.\n",
        );
        assert_session(text, "markdown", expected);
    }

    #[test]
    fn session_latex_keeps_section_cite_math_and_abbreviations() {
        let text = concat!(
            "\\section{Results. More title.}\n",
            "Dr. Smith showed that the estimate holds for smooth data, e.g. when the mesh is refined. ",
            "The leading term is \\(x = 1.5\\) on a quasi-uniform family. ",
            "Details are in \\cite{smith2020,jones2021}. ",
            "A later sentence stays fused on this line.\n",
        );
        let expected = concat!(
            "\\section{Results. More title.}\n",
            "Dr. Smith showed that the estimate holds for smooth data, e.g. when the mesh is refined.\n",
            "The leading term is \\(x = 1.5\\) on a quasi-uniform family.\n",
            "Details are in \\cite{smith2020,jones2021}.\n",
            "A later sentence stays fused on this line.\n",
        );
        assert_session(text, "latex", expected);
    }

    #[test]
    fn session_org_keyword_is_not_glued_to_the_next_sentence() {
        let text = concat!(
            "#+title: A note\n",
            "Dr. Smith showed the bound, e.g. for smooth data. ",
            "See [cite:@smith2020]. Next stays fused.\n",
        );
        let formatted = format_as(text, "org");
        assert_eq!(detect(text), "org");
        assert!(
            formatted.starts_with(
                "#+title: A note\nDr. Smith showed the bound, e.g. for smooth data.\n"
            ),
            "keyword line was glued or e.g. split: {formatted:?}"
        );
        assert!(formatted.contains("[cite:@smith2020].\n"));
        assert!(check_as(&formatted, "org").passed);
    }

    #[test]
    fn session_latex_cite_without_a_section_is_latex() {
        let text = "Dr. Smith showed the bound, e.g. for smooth data. See \\cite{smith2020}. The term is \\(x = 1.5\\).\n";
        assert_eq!(detect(text), "latex");
        let formatted = format_as(text, "latex");
        assert!(
            formatted.contains("e.g. for smooth data.\nSee \\cite{smith2020}.\n"),
            "citation or abbreviation moved: {formatted:?}"
        );
        assert!(formatted.contains("\\(x = 1.5\\)."));
        assert!(check_as(&formatted, "latex").passed);
    }

    #[test]
    fn markdown_list_is_not_detected_as_an_org_headline() {
        let text = concat!(
            "* Dr. Smith showed the bound, e.g. for smooth data. Next sentence.\n",
            "* Another item stays. Fused too.\n",
        );
        let detected = detect(text);
        assert_ne!(detected, "org", "a star list must not freeze as headlines");
        let formatted = format_as(text, &detected);
        assert!(
            formatted.contains("e.g. for smooth data.\n"),
            "list item stayed fused: {formatted:?}"
        );
        assert!(
            formatted.contains("Another item stays.\n"),
            "second item stayed fused: {formatted:?}"
        );
        assert!(check_as(&formatted, &detected).passed);
    }

    #[test]
    fn range_rewrite_splits_one_latex_line_and_keeps_the_section() {
        let mut params = plaintext(concat!(
            "\\section{Keep. This.}\n",
            "Dr. Smith showed the bound, e.g. for smooth data. See \\cite{smith2020}. Next sentence stays on the line.\n",
            "Tail stays. Fused too.\n",
        ));
        params.format = "latex".to_string();
        params.range = Some(LineRange { start: 2, end: 2 });
        let out = format(params);
        assert_eq!(
            out,
            concat!(
                "\\section{Keep. This.}\n",
                "Dr. Smith showed the bound, e.g. for smooth data.\n",
                "See \\cite{smith2020}.\n",
                "Next sentence stays on the line.\n",
                "Tail stays. Fused too.\n",
            )
        );
    }

    #[test]
    fn detect_plain_prose_and_an_rst_underline() {
        assert_eq!(detect("Dr. Smith went home. He was tired.\n"), "plaintext");
        assert_eq!(detect("A title\n=====\n\nBody text. Next.\n"), "rst");
        assert_eq!(detect("\\begin{document}\nHello. Next.\n"), "latex");
    }
}
