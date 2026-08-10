//! Line-level `--check` diagnostics: fused, wrap, and long.
//!
//! These kinds describe the source as written. They share the same sentence
//! splitter as format so abbreviations do not produce false fused hits.

use serde::Serialize;

use crate::format::Format;
use crate::parser::source_line_payloads;
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
    let payloads = source_line_payloads(input, format);
    debug_assert_eq!(
        payloads.len(),
        lines.len(),
        "parser line map must cover every source line (got {} payloads for {} lines)",
        payloads.len(),
        lines.len()
    );
    let mut diagnostics = Vec::new();
    let mut prev_prose: Option<String> = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            prev_prose = None;
            continue;
        }
        let Some(payload) = payloads.get(idx).and_then(|p| p.as_deref()) else {
            prev_prose = None;
            continue;
        };

        let line_no = idx + 1;
        let excerpt = excerpt_of(line);

        if splitter.split(payload.trim()).len() > 1 {
            diagnostics.push(LineDiagnostic {
                line: line_no,
                kind: DiagnosticKind::Fused,
                excerpt: excerpt.clone(),
            });
        }

        if let Some(prev) = prev_prose.as_deref() {
            if !ends_clause_or_quote(prev) {
                if let Some(word) = leading_lowercase_word(payload) {
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

        let width = payload.chars().count();
        if width > long_threshold && has_clause_boundary_hint(payload) {
            diagnostics.push(LineDiagnostic {
                line: line_no,
                kind: DiagnosticKind::Long,
                excerpt,
            });
        }

        prev_prose = Some(payload.to_string());
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

    fn assert_no_kind_on(
        found: &[LineDiagnostic],
        kind: DiagnosticKind,
        lines: &[usize],
        msg: &str,
    ) {
        for line in lines {
            assert!(
                found.iter().all(|d| !(d.line == *line && d.kind == kind)),
                "{msg}: line {line} has {kind:?} in {found:?}"
            );
        }
    }

    #[test]
    fn org_quote_comment_drawer_are_not_prose() {
        let input = concat!(
            "#+BEGIN_QUOTE\n",
            "Quoted hello. Quoted world.\n",
            "#+END_QUOTE\n",
            "# Comment hello. Comment world.\n",
            ":PROPERTIES:\n",
            ":ID: drawer-value-hello. Drawer world with extra padding so a comma, stays structure.\n",
            ":END:\n",
            "\n",
            "Real prose. Second sentence.\n",
        );
        let splitter = UnicodeSentenceSplitter::new();
        let found = collect_diagnostics(input, Format::Org, &splitter, DEFAULT_LONG_THRESHOLD);
        assert_no_kind_on(
            &found,
            DiagnosticKind::Fused,
            &[2, 4, 6],
            "org quote/comment/drawer must not be fused",
        );
        assert!(
            found
                .iter()
                .any(|d| d.line == 9 && d.kind == DiagnosticKind::Fused),
            "real org prose should still be fused, got {found:?}"
        );
    }

    #[test]
    fn markdown_front_matter_and_setext_are_not_prose() {
        let input = concat!(
            "---\n",
            "title: Hello. World in front matter.\n",
            "---\n",
            "\n",
            "Setext Title. Still Title\n",
            "=========================\n",
            "\n",
            "Body one. Body two.\n",
        );
        let splitter = UnicodeSentenceSplitter::new();
        let found = collect_diagnostics(input, Format::Markdown, &splitter, DEFAULT_LONG_THRESHOLD);
        assert_no_kind_on(
            &found,
            DiagnosticKind::Fused,
            &[2, 5],
            "front matter and setext title must not be fused",
        );
        assert!(
            found
                .iter()
                .any(|d| d.line == 8 && d.kind == DiagnosticKind::Fused),
            "markdown body should still be fused, got {found:?}"
        );
    }

    #[test]
    fn latex_preamble_and_equation_are_not_prose() {
        let input = concat!(
            "\\documentclass{article}\n",
            "\\usepackage{amsmath}\n",
            "\\begin{document}\n",
            "\\begin{equation}\n",
            "E = mc^2 + a very long expression, with commas, that exceeds one hundred twenty characters easily xxxxxxxxxxxxxxxxx\n",
            "\\end{equation}\n",
            "Body one. Body two.\n",
            "\\end{document}\n",
        );
        let splitter = UnicodeSentenceSplitter::new();
        let found = collect_diagnostics(input, Format::Latex, &splitter, DEFAULT_LONG_THRESHOLD);
        assert_no_kind_on(
            &found,
            DiagnosticKind::Fused,
            &[1, 2, 3, 4, 5, 6, 8],
            "latex preamble and equation must not be fused",
        );
        assert!(
            found
                .iter()
                .all(|d| !(d.line == 5 && d.kind == DiagnosticKind::Long)),
            "equation body must not be long, got {found:?}"
        );
        assert!(
            found
                .iter()
                .any(|d| d.line == 7 && d.kind == DiagnosticKind::Fused),
            "latex body should still be fused, got {found:?}"
        );
    }

    #[test]
    fn rst_title_and_note_body_are_not_prose() {
        let input = concat!(
            "Title Here. With Period.\n",
            "========================\n",
            "\n",
            ".. note::\n",
            "\n",
            "   This is a note. With two sentences.\n",
            "\n",
            "Body one. Body two.\n",
        );
        let splitter = UnicodeSentenceSplitter::new();
        let found = collect_diagnostics(input, Format::Rst, &splitter, DEFAULT_LONG_THRESHOLD);
        assert_no_kind_on(
            &found,
            DiagnosticKind::Fused,
            &[1, 4, 6],
            "rst title and note body must not be fused",
        );
        assert!(
            found
                .iter()
                .any(|d| d.line == 8 && d.kind == DiagnosticKind::Fused),
            "rst body should still be fused, got {found:?}"
        );
    }

    #[test]
    fn snapper_off_region_is_not_prose() {
        let input = concat!(
            "Hello world. This is a test.\n",
            "snapper:off\n",
            "Do not. Touch this.\n",
            "snapper:on\n",
            "After one. After two.\n",
        );
        let splitter = UnicodeSentenceSplitter::new();
        let found =
            collect_diagnostics(input, Format::Plaintext, &splitter, DEFAULT_LONG_THRESHOLD);
        assert_no_kind_on(
            &found,
            DiagnosticKind::Fused,
            &[2, 3, 4],
            "snapper:off body must not be fused",
        );
        assert!(
            found
                .iter()
                .any(|d| d.line == 1 && d.kind == DiagnosticKind::Fused),
            "prose before snapper:off should be fused, got {found:?}"
        );
        assert!(
            found
                .iter()
                .any(|d| d.line == 5 && d.kind == DiagnosticKind::Fused),
            "prose after snapper:on should be fused, got {found:?}"
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

    fn no_fused(input: &str, format: Format) -> Vec<LineDiagnostic> {
        let splitter = UnicodeSentenceSplitter::new();
        collect_diagnostics(input, format, &splitter, DEFAULT_LONG_THRESHOLD)
    }

    #[test]
    fn numbered_list_payload_is_item_text() {
        use crate::parser::source_line_payloads;
        let md = source_line_payloads("1. Hello world.\n", Format::Markdown);
        assert_eq!(md[0].as_deref(), Some("Hello world."));
        let org = source_line_payloads("1. Hello world.\n", Format::Org);
        assert_eq!(org[0].as_deref(), Some("Hello world."));
        let tex = source_line_payloads(
            "\\begin{document}\nSee Fig. 1. % TODO cite\n\\end{document}\n",
            Format::Latex,
        );
        assert_eq!(
            tex[1].as_deref().map(str::trim),
            Some("See Fig. 1."),
            "mid-line % prefix is the prose payload; trailing space is splice gap"
        );
    }

    #[test]
    fn numbered_list_item_is_not_fused_markdown() {
        let input = "1. Hello world.\n";
        let found = no_fused(input, Format::Markdown);
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Fused),
            "numbered list body is one sentence; marker is not fused, got {found:?}"
        );
        let config = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        assert!(
            !would_reformat(input, &config).unwrap(),
            "1. Hello world. must be identity under markdown"
        );
    }

    #[test]
    fn numbered_list_item_is_not_fused_org() {
        let input = "1. Hello world.\n";
        let found = no_fused(input, Format::Org);
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Fused),
            "org numbered list body is one sentence; marker is not fused, got {found:?}"
        );
        let config = FormatConfig {
            format: Format::Org,
            ..Default::default()
        };
        assert!(
            !would_reformat(input, &config).unwrap(),
            "1. Hello world. must be identity under org"
        );
    }

    #[test]
    fn latex_mid_line_comment_is_not_fused() {
        let input = "See Fig. 1. % TODO cite\n";
        let found = no_fused(input, Format::Latex);
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Fused),
            "latex comment is structure; Fig. 1. is one sentence, got {found:?}"
        );
        let config = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        };
        assert!(
            !would_reformat(input, &config).unwrap(),
            "See Fig. 1. % TODO cite must be identity under latex"
        );
    }

    #[test]
    fn latex_body_mid_line_comment_is_not_fused() {
        let input = "\\begin{document}\nSee Fig. 1. % TODO cite\n\\end{document}\n";
        let found = no_fused(input, Format::Latex);
        assert!(
            found.iter().all(|d| d.kind != DiagnosticKind::Fused),
            "body-line comment must not fuse Fig. 1. with TODO, got {found:?}"
        );
        let config = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        };
        assert!(
            !would_reformat(input, &config).unwrap(),
            "document with See Fig. 1. % TODO cite must be identity, got {}",
            crate::format_text(input, &config).unwrap()
        );
    }

    #[test]
    fn parser_line_map_covers_every_source_line() {
        use crate::parser::source_line_payloads;
        let cases = [
            (
                Format::Org,
                "#+BEGIN_QUOTE\nQuoted hello. Quoted world.\n#+END_QUOTE\n# Comment.\n:PROPERTIES:\n:ID: x\n:END:\n\nReal. Two.\n",
            ),
            (
                Format::Markdown,
                "---\ntitle: Hello. World.\n---\n\nSetext Title. Still\n===================\n\nBody. Two.\n",
            ),
            (
                Format::Latex,
                "\\documentclass{article}\n\\begin{document}\n\\begin{equation}\nE=mc^2\n\\end{equation}\nBody. Two.\n\\end{document}\n",
            ),
            (
                Format::Rst,
                "Title Here. With Period.\n========================\n\n.. note::\n\n   Note. Two.\n\nBody. Two.\n",
            ),
            (
                Format::Plaintext,
                "Hello. World.\nsnapper:off\nDo not. Touch.\nsnapper:on\nAfter. Two.\n",
            ),
        ];
        for (fmt, input) in cases {
            let kinds = source_line_payloads(input, fmt);
            assert_eq!(
                kinds.len(),
                input.lines().count(),
                "line map length mismatch for {fmt:?}"
            );
        }
    }

    #[test]
    fn resolve_long_threshold_prefers_max_width() {
        assert_eq!(resolve_long_threshold(80, Some(200)), 80);
        assert_eq!(resolve_long_threshold(0, Some(200)), 200);
        assert_eq!(resolve_long_threshold(0, None), DEFAULT_LONG_THRESHOLD);
    }
}
