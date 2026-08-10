//! Line-level `--check` diagnostics: fused, wrap, and long.
//!
//! These kinds describe the source as written. They share the same sentence
//! splitter as format so abbreviations do not produce false fused hits.

use serde::Serialize;

use crate::format::Format;
use crate::sentence::SentenceSplitter;
use crate::{FormatConfig, format_text};

/// Default character threshold for the advisory `long` kind when `max_width`
/// is unset (0).
pub const DEFAULT_LONG_THRESHOLD: usize = 120;

/// Kind of a line-level check diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticKind {
    /// Splitter finds more than one sentence on a prose line.
    Fused,
    /// Mid-clause continuation: the previous prose line does not end a clause
    /// and this line starts with a lowercase non-connector word.
    Wrap,
    /// Advisory: prose line exceeds the width threshold and has a clause
    /// boundary where a break could go.
    Long,
}

impl DiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticKind::Fused => "fused",
            DiagnosticKind::Wrap => "wrap",
            DiagnosticKind::Long => "long",
        }
    }
}

/// One 1-indexed diagnostic on a source line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineDiagnostic {
    pub line: usize,
    pub kind: DiagnosticKind,
    pub excerpt: String,
}

/// Width used for `long`: `max_width` when set, otherwise the configured
/// default (120 if unset).
pub fn resolve_long_threshold(max_width: usize, configured: Option<usize>) -> usize {
    if max_width > 0 {
        max_width
    } else {
        configured.unwrap_or(DEFAULT_LONG_THRESHOLD)
    }
}

/// Identity check used by CLI `--check` and MCP `would_reformat`.
pub fn would_reformat(input: &str, config: &FormatConfig) -> anyhow::Result<bool> {
    let output = format_text(input, config)?;
    Ok(output != input)
}

/// Connector words that start an intentional semantic break, not a wrap.
const WRAP_CONNECTORS: &[&str] = &[
    "and", "but", "so", "or", "nor", "yet", "which", "that", "where", "who", "whose", "whom",
    "when", "while", "because", "although", "though", "unless", "until", "if", "as",
];

/// Collect fused / wrap / long diagnostics for `input`.
///
/// `long_threshold` is character count, already resolved by
/// [`resolve_long_threshold`].
pub fn collect_diagnostics(
    input: &str,
    format: Format,
    splitter: &dyn SentenceSplitter,
    long_threshold: usize,
) -> Vec<LineDiagnostic> {
    let lines: Vec<&str> = input.lines().collect();
    let prose = classify_prose_lines(&lines, format);
    let mut diagnostics = Vec::new();
    let mut prev_prose: Option<&str> = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            prev_prose = None;
            continue;
        }
        if !prose[idx] {
            prev_prose = None;
            continue;
        }

        let line_no = idx + 1;
        let excerpt = excerpt_of(line);

        if splitter.split(line.trim()).len() > 1 {
            diagnostics.push(LineDiagnostic {
                line: line_no,
                kind: DiagnosticKind::Fused,
                excerpt: excerpt.clone(),
            });
        }

        if let Some(prev) = prev_prose {
            if !ends_clause_or_quote(prev) {
                if let Some(word) = leading_lowercase_word(line) {
                    if !WRAP_CONNECTORS.contains(&word.as_str()) {
                        diagnostics.push(LineDiagnostic {
                            line: line_no,
                            kind: DiagnosticKind::Wrap,
                            excerpt: excerpt.clone(),
                        });
                    }
                }
            }
        }

        let width = line.chars().count();
        if width > long_threshold && has_clause_boundary_hint(line) {
            diagnostics.push(LineDiagnostic {
                line: line_no,
                kind: DiagnosticKind::Long,
                excerpt,
            });
        }

        prev_prose = Some(line);
    }

    diagnostics
}

fn excerpt_of(line: &str) -> String {
    const MAX: usize = 200;
    let trimmed = line.trim();
    if trimmed.chars().count() <= MAX {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(MAX).collect();
    out.push_str("...");
    out
}

fn ends_clause_or_quote(line: &str) -> bool {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.ends_with('\u{2014}') || trimmed.ends_with("--") {
        return true;
    }
    matches!(
        trimmed.chars().last(),
        Some(
            '.' | '!'
                | '?'
                | ';'
                | ':'
                | ','
                | '"'
                | '\''
                | '\u{201d}'
                | '\u{2019}'
                | ')'
                | ']'
                | '}'
        )
    )
}

fn leading_lowercase_word(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let mut chars = trimmed.chars();
    let first = chars.next()?;
    if !first.is_lowercase() {
        return None;
    }
    let mut word = String::new();
    word.push(first);
    for c in chars {
        if c.is_alphabetic() || c == '\'' {
            word.push(c);
        } else {
            break;
        }
    }
    Some(word)
}

fn has_clause_boundary_hint(line: &str) -> bool {
    if line.contains('\u{2014}') || line.contains("--") {
        return true;
    }
    let mut i = 0;
    while i < line.len() {
        let c = line[i..].chars().next().unwrap();
        let len = c.len_utf8();
        if matches!(c, ',' | ';' | ':' | '.' | '!' | '?') {
            let rest = &line[i + len..];
            if rest.starts_with(|n: char| n.is_whitespace()) {
                return true;
            }
        }
        i += len;
    }
    false
}

fn classify_prose_lines(lines: &[&str], format: Format) -> Vec<bool> {
    let mut prose = vec![false; lines.len()];
    let mut in_code = false;
    let mut rst_code_indent: Option<usize> = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match format {
            Format::Plaintext => {
                prose[idx] = true;
            }
            Format::Markdown => {
                if is_md_fence(line) {
                    in_code = !in_code;
                    continue;
                }
                if in_code || is_md_structure(line) {
                    continue;
                }
                prose[idx] = true;
            }
            Format::Org => {
                if is_org_src_begin(line) {
                    in_code = true;
                    continue;
                }
                if is_org_src_end(line) {
                    in_code = false;
                    continue;
                }
                if in_code || is_org_structure(line) {
                    continue;
                }
                prose[idx] = true;
            }
            Format::Latex => {
                if let Some(name) = latex_begin_env(line) {
                    if is_latex_code_env(name) {
                        in_code = true;
                    }
                    continue;
                }
                if let Some(name) = latex_end_env(line) {
                    if is_latex_code_env(name) {
                        in_code = false;
                    }
                    continue;
                }
                if in_code || is_latex_structure(line) {
                    continue;
                }
                prose[idx] = true;
            }
            Format::Rst => {
                if is_rst_code_directive(line) {
                    rst_code_indent = Some(leading_spaces(line));
                    continue;
                }
                if let Some(indent) = rst_code_indent {
                    if leading_spaces(line) > indent {
                        continue;
                    }
                    rst_code_indent = None;
                }
                if is_rst_structure(line) {
                    continue;
                }
                prose[idx] = true;
            }
        }
    }
    prose
}

fn is_md_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn is_md_structure(line: &str) -> bool {
    let t = line.trim_start();
    let hashes = t.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && t.as_bytes().get(hashes) == Some(&b' ') {
        return true;
    }
    let trimmed = line.trim();
    if trimmed.starts_with('|') && trimmed.ends_with('|') {
        return true;
    }
    if trimmed == "---" || trimmed == "+++" {
        return true;
    }
    let end_trimmed = line.trim_end();
    let indent = line.len() - line.trim_start().len();
    let body = end_trimmed.trim_start();
    if indent <= 3
        && !body.is_empty()
        && (body.chars().all(|c| c == '=') || body.chars().all(|c| c == '-'))
    {
        return true;
    }
    trimmed.starts_with("<!--")
}

fn is_org_src_begin(line: &str) -> bool {
    let u = line.trim_start().to_ascii_uppercase();
    u.starts_with("#+BEGIN_SRC") || u.starts_with("#+BEGIN_EXAMPLE")
}

fn is_org_src_end(line: &str) -> bool {
    let u = line.trim_start().to_ascii_uppercase();
    u.starts_with("#+END_SRC") || u.starts_with("#+END_EXAMPLE")
}

fn is_org_structure(line: &str) -> bool {
    let t = line.trim_start();
    let stars = t.bytes().take_while(|&b| b == b'*').count();
    if stars > 0 && t.as_bytes().get(stars) == Some(&b' ') {
        return true;
    }
    if t.starts_with("#+") {
        return true;
    }
    let trimmed = line.trim();
    if trimmed.starts_with(':') && trimmed.ends_with(':') && trimmed.len() > 2 {
        return true;
    }
    trimmed.starts_with('|')
}

fn latex_begin_env(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix("\\begin{")?;
    rest.split('}').next()
}

fn latex_end_env(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix("\\end{")?;
    rest.split('}').next()
}

fn is_latex_code_env(name: &str) -> bool {
    matches!(name, "verbatim" | "lstlisting" | "minted")
}

fn is_latex_structure(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with('%') {
        return true;
    }
    t.starts_with("\\documentclass")
        || t.starts_with("\\usepackage")
        || t.starts_with("\\section")
        || t.starts_with("\\subsection")
        || t.starts_with("\\subsubsection")
        || t.starts_with("\\chapter")
        || t.starts_with("\\part")
        || t.starts_with("\\paragraph")
        || t.starts_with("\\title")
        || t.starts_with("\\author")
        || t.starts_with("\\maketitle")
}

fn is_rst_code_directive(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with(".. code-block::")
        || t.starts_with(".. sourcecode::")
        || t.starts_with(".. code::")
}

fn is_rst_structure(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with(".. ") {
        return true;
    }
    let body = line.trim();
    !body.is_empty()
        && (body.chars().all(|c| c == '=')
            || body.chars().all(|c| c == '-')
            || body.chars().all(|c| c == '~')
            || body.chars().all(|c| c == '`'))
}

fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|&b| b == b' ').count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::unicode::UnicodeSentenceSplitter;

    fn diags(input: &str) -> Vec<LineDiagnostic> {
        let splitter = UnicodeSentenceSplitter::new();
        collect_diagnostics(input, Format::Plaintext, &splitter, DEFAULT_LONG_THRESHOLD)
    }

    fn kinds_on(diags: &[LineDiagnostic], line: usize) -> Vec<DiagnosticKind> {
        diags
            .iter()
            .filter(|d| d.line == line)
            .map(|d| d.kind)
            .collect()
    }

    #[test]
    fn fused_two_sentences_on_one_line() {
        let found = diags("Hello world. This is a test.\n");
        assert!(
            found
                .iter()
                .any(|d| d.line == 1 && d.kind == DiagnosticKind::Fused),
            "expected fused on line 1, got {found:?}"
        );
        assert!(
            found
                .iter()
                .any(|d| d.kind == DiagnosticKind::Fused && d.excerpt.contains("Hello world")),
            "fused excerpt should carry the source line, got {found:?}"
        );
    }

    #[test]
    fn fused_abbreviation_is_not_a_sentence_break() {
        let found = diags("See Fig. 3 for details.\n");
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Fused),
            "Fig. must not produce fused, got {found:?}"
        );
    }

    #[test]
    fn wrap_mid_clause_continuation() {
        let found = diags("The experiment ran for several\nweeks using the usual protocol.\n");
        assert!(
            found
                .iter()
                .any(|d| d.line == 2 && d.kind == DiagnosticKind::Wrap),
            "expected wrap on the continuation line, got {found:?}"
        );
        assert!(
            found
                .iter()
                .any(|d| d.kind == DiagnosticKind::Wrap && d.excerpt.contains("weeks")),
            "wrap excerpt should be the continuation line, got {found:?}"
        );
    }

    #[test]
    fn wrap_skips_connector_and() {
        let found = diags("The experiment ran for several weeks\nand used the usual protocol.\n");
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Wrap),
            "connector-led and is not wrap, got {found:?}"
        );
    }

    #[test]
    fn wrap_skips_connector_which() {
        let found = diags("The results were significant\nwhich surprised the team.\n");
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Wrap),
            "connector-led which is not wrap, got {found:?}"
        );
    }

    #[test]
    fn wrap_skips_after_comma() {
        let found = diags("The experiment ran for several weeks,\nusing the usual protocol.\n");
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Wrap),
            "a comma-ended previous line is a clause break, not wrap, got {found:?}"
        );
    }

    #[test]
    fn wrap_skips_after_em_dash() {
        let found =
            diags("The experiment ran for several weeks \u{2014}\nusing the usual protocol.\n");
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Wrap),
            "an em-dash-ended previous line is not wrap, got {found:?}"
        );
    }

    #[test]
    fn wrap_skips_after_closing_quote() {
        let found = diags("He said \"yes\"\nwithout any pause.\n");
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Wrap),
            "a closing-quote-ended previous line is not wrap, got {found:?}"
        );
    }

    #[test]
    fn wrap_skips_uppercase_start() {
        let found = diags(
            "The experiment ran for several weeks\nUsing a different protocol is possible.\n",
        );
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Wrap),
            "uppercase start is not a mid-clause wrap, got {found:?}"
        );
    }

    #[test]
    fn long_advisory_needs_clause_boundary() {
        let long_with_comma = format!(
            "The quick brown fox jumps over the lazy dog, then continues running across a very long meadow without pausing for breath at all today.\n"
        );
        assert!(
            long_with_comma.trim_end().chars().count() > DEFAULT_LONG_THRESHOLD,
            "fixture must exceed the default long threshold"
        );
        let found = diags(&long_with_comma);
        assert!(
            found
                .iter()
                .any(|d| d.line == 1 && d.kind == DiagnosticKind::Long),
            "long line with a comma should be long, got {found:?}"
        );

        let no_hint = format!("{}\n", "A".repeat(DEFAULT_LONG_THRESHOLD + 10));
        let found = diags(&no_hint);
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Long),
            "a long line with no clause-boundary hint is not long, got {found:?}"
        );
    }

    #[test]
    fn long_uses_resolved_threshold() {
        let splitter = UnicodeSentenceSplitter::new();
        let line = "Short clause, still short.\n";
        let found = collect_diagnostics(line, Format::Plaintext, &splitter, 5);
        assert!(
            found.iter().any(|d| d.kind == DiagnosticKind::Long),
            "threshold 5 must flag a comma-bearing line, got {found:?}"
        );
    }

    #[test]
    fn fused_and_long_can_share_a_line() {
        let line = "Hello world. This is a test that goes on and on, with extra words to exceed the default long threshold of one hundred twenty characters easily.\n";
        assert!(line.trim_end().chars().count() > DEFAULT_LONG_THRESHOLD);
        let found = diags(line);
        let kinds = kinds_on(&found, 1);
        assert!(
            kinds.contains(&DiagnosticKind::Fused),
            "expected fused, got {found:?}"
        );
        assert!(
            kinds.contains(&DiagnosticKind::Long),
            "expected long, got {found:?}"
        );
    }

    #[test]
    fn structure_and_code_are_not_prose() {
        let md = "# Title. Still a heading.\n\n```\nHello. World.\n```\n\nBody sentence.\n";
        let splitter = UnicodeSentenceSplitter::new();
        let found = collect_diagnostics(md, Format::Markdown, &splitter, DEFAULT_LONG_THRESHOLD);
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Fused),
            "headings and fenced code must not produce fused, got {found:?}"
        );
    }

    #[test]
    fn would_reformat_matches_format_identity() {
        let config = FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        };
        assert!(would_reformat("Hello world. This is a test.\n", &config).unwrap());
        assert!(!would_reformat("Hello world.\nThis is a test.\n", &config).unwrap());
    }

    #[test]
    fn resolve_long_threshold_prefers_max_width() {
        assert_eq!(resolve_long_threshold(80, Some(200)), 80);
        assert_eq!(resolve_long_threshold(0, Some(200)), 200);
        assert_eq!(resolve_long_threshold(0, None), DEFAULT_LONG_THRESHOLD);
    }
}
