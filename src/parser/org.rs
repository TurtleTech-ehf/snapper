use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Region, RegionOrigin, SpannedRegion, flush_prose_spanned, iter_lines,
    push_prose_line,
};

static HEADLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\*+\s+(?:TODO\s+|DONE\s+|NEXT\s+|WAIT\s+)?)(.*)$").unwrap());

static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*(?:[-+]|\d+[.)]) )(.*)$").unwrap());

/// Matches LaTeX \begin{env} lines embedded in org prose.
static LATEX_BEGIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\\begin\{([^}]+)\}").unwrap());

/// Matches LaTeX \end{env} lines.
static LATEX_END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\\end\{([^}]+)\}").unwrap());

/// Matches org inline export snippets: @@backend:value@@
static EXPORT_SNIPPET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@@[a-zA-Z]+:[^@]*@@").unwrap());

pub struct OrgParser;

impl OrgParser {
    /// Check if a line starts a block (#+BEGIN_...)
    fn is_block_begin(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.to_ascii_uppercase().starts_with("#+BEGIN_")
    }

    /// Check if a line starts a source code block (#+BEGIN_SRC LANG ARGS...).
    /// Returns the language token if present, or `Some(None)` for a bare
    /// `#+BEGIN_SRC`. Returns `None` for non-src blocks.
    fn is_src_begin(line: &str) -> Option<Option<String>> {
        let trimmed = line.trim_start();
        let upper = trimmed.to_ascii_uppercase();
        if !upper.starts_with("#+BEGIN_SRC") {
            return None;
        }
        // Slice the original (case-preserving) tail past the directive.
        let rest = trimmed["#+BEGIN_SRC".len()..].trim_start();
        if rest.is_empty() {
            return Some(None);
        }
        // Language is the first whitespace-delimited token.
        let lang = rest.split_whitespace().next().map(|s| s.to_string());
        Some(lang)
    }

    /// Check if a line ends a block (#+END_...)
    fn is_block_end(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.to_ascii_uppercase().starts_with("#+END_")
    }

    /// Check if a line ends a source code block (#+END_SRC).
    fn is_src_end(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.to_ascii_uppercase().starts_with("#+END_SRC")
    }

    /// Check if a line starts a property drawer
    fn is_drawer_begin(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with(':') && trimmed.ends_with(':') && trimmed.len() > 2
    }

    /// Check if a line ends a drawer
    fn is_drawer_end(line: &str) -> bool {
        line.trim().eq_ignore_ascii_case(":END:")
    }

    /// Check if a line is a keyword/directive (#+KEYWORD:)
    fn is_keyword(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("#+") && !Self::is_block_begin(line) && !Self::is_block_end(line)
    }

    /// Check if a line is a comment (starts with #, but not #+)
    fn is_comment(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with('#') && !trimmed.starts_with("#+")
    }

    /// Check if a line is a table row
    fn is_table_row(line: &str) -> bool {
        line.trim_start().starts_with('|')
    }

    /// Check if a line starts a LaTeX environment (\begin{...})
    fn is_latex_begin(line: &str) -> Option<String> {
        LATEX_BEGIN_RE
            .captures(line)
            .map(|caps| caps.get(1).unwrap().as_str().to_string())
    }

    /// Check if a line ends a LaTeX environment (\end{...})
    fn is_latex_end(line: &str, env: &str) -> bool {
        LATEX_END_RE
            .captures(line)
            .is_some_and(|caps| caps.get(1).unwrap().as_str() == env)
    }

    /// Check if a line is a display math delimiter (\[ or \])
    fn is_display_math_open(line: &str) -> bool {
        line.trim() == r"\["
    }

    fn is_display_math_close(line: &str) -> bool {
        line.trim() == r"\]"
    }

    /// Check if a line is entirely an inline export snippet (@@backend:...@@)
    fn is_export_snippet_line(line: &str) -> bool {
        let trimmed = line.trim();
        EXPORT_SNIPPET_RE.is_match(trimmed) && trimmed.starts_with("@@")
    }
}

impl FormatParser for OrgParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let mut regions: Vec<SpannedRegion> = Vec::new();
        let mut current_prose = String::new();
        let mut prose_span: Option<ByteSpan> = None;
        let mut in_block = false;
        // Source block bookkeeping; `in_src_block` implies `in_block`.
        let mut in_src_block = false;
        let mut src_lang: Option<String> = None;
        let mut src_header = ByteSpan::default();
        let mut src_body_start = 0usize;
        let mut in_drawer = false;
        let mut in_latex_env: Option<String> = None;
        let mut in_display_math = false;
        let mut pragma_off = false;
        // Track list item context: indent level of the marker text.
        // Continuation lines indented at or beyond this level belong to the item.
        let mut list_item_indent: Option<usize> = None;

        for line in iter_lines(input) {
            let line_text = line.text;
            // Check for snapper:off/on pragmas. Inside a source block we
            // defer pragma handling to the code-block reflow so the
            // language's own comment marker controls the freeze.
            if !in_src_block {
                if let Some(on) = super::check_pragma(line_text) {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    pragma_off = !on;
                    regions.push(SpannedRegion::structure(input, line.span()));
                    continue;
                }

                // Inside pragma-off region: pass through unchanged
                if pragma_off {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(input, line.span()));
                    continue;
                }
            }

            // Inside a source block -- buffer body until #+END_SRC
            if in_src_block {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_src_end(line_text) {
                    in_src_block = false;
                    in_block = false;
                    regions.push(SpannedRegion::code(
                        input,
                        src_lang.take(),
                        src_header,
                        ByteSpan::new(src_body_start, line.start),
                        line.span(),
                    ));
                }
                continue;
            }

            // Inside a non-src block -- everything is structure
            if in_block {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_block_end(line_text) {
                    in_block = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Inside a drawer -- everything is structure
            if in_drawer {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_drawer_end(line_text) {
                    in_drawer = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Inside a LaTeX environment -- everything is structure
            if let Some(ref env) = in_latex_env {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let done = Self::is_latex_end(line_text, env);
                regions.push(SpannedRegion::structure(input, line.span()));
                if done {
                    in_latex_env = None;
                }
                continue;
            }

            // Inside display math \[...\] -- everything is structure
            if in_display_math {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_display_math_close(line_text) {
                    in_display_math = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Source block begin (#+BEGIN_SRC LANG ...)
            if let Some(lang) = Self::is_src_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_block = true;
                in_src_block = true;
                src_lang = lang;
                src_header = line.span();
                src_body_start = line.end;
                continue;
            }

            // Other #+BEGIN_ block: opaque structure
            if Self::is_block_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_block = true;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Drawer begin
            if Self::is_drawer_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_drawer = true;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // LaTeX environment begin (\begin{equation} etc.)
            if let Some(env) = Self::is_latex_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_latex_env = Some(env);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Display math open (\[)
            if Self::is_display_math_open(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_display_math = true;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Export snippet line (@@latex:...@@)
            if Self::is_export_snippet_line(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Blank line
            if line_text.trim().is_empty() {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                list_item_indent = None;
                regions.push(SpannedRegion::blank(input, line.span()));
                continue;
            }

            // Keyword/directive
            if Self::is_keyword(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Comment
            if Self::is_comment(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Table row
            if Self::is_table_row(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Bare file/http links on their own line -- treat as structure
            if line_text.trim_start().starts_with("file:")
                || line_text.trim_start().starts_with("http://")
                || line_text.trim_start().starts_with("https://")
            {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Headline: keep the entire line as Structure.
            // Splitting Structure(stars)+Prose(title) reflowed multi-sentence
            // titles and left continuation lines without stars (orphan body).
            // Org headlines are single-line; do not reflow them.
            if HEADLINE_RE.is_match(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // List item: marker is structure, rest is prose
            if let Some(caps) = LIST_ITEM_RE.captures(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let text = caps.get(2).unwrap().as_str();
                // Track indent for continuation detection: text starts at marker length
                list_item_indent = Some(marker.len());
                let marker_span = ByteSpan::new(line.start, line.start + marker.len());
                regions.push(SpannedRegion::structure(input, marker_span));
                if !text.is_empty() {
                    regions.push(SpannedRegion::prose(
                        text.to_string(),
                        ByteSpan::new(line.start + marker.len(), line.start + line_text.len()),
                    ));
                }
                let term = line.terminator_span();
                if !term.is_empty() {
                    regions.push(SpannedRegion::structure(input, term));
                }
                continue;
            }

            // List item continuation: indented line following a list item
            if let Some(indent) = list_item_indent {
                let leading = line_text.len() - line_text.trim_start().len();
                if leading >= indent && !line_text.trim().is_empty() {
                    // Append to the previous Prose region of the list item.
                    // The last three regions are Structure(marker), Prose(text), Structure(\n)
                    // We want to extend the Prose region.
                    let is_term = matches!(
                        regions.last(),
                        Some(SpannedRegion {
                            region: Region::Structure(s),
                            ..
                        }) if s == "\n"
                    );
                    if is_term {
                        regions.pop();
                        if let Some(prev) = regions.last_mut() {
                            if let Region::Prose(prose) = &mut prev.region {
                                prose.push(' ');
                                prose.push_str(line_text.trim());
                            }
                            if let Some(RegionOrigin::Whole(span)) = &mut prev.origin {
                                span.end = line.start + line_text.len();
                            }
                        }
                        let term = line.terminator_span();
                        if !term.is_empty() {
                            regions.push(SpannedRegion::structure(input, term));
                        }
                        continue;
                    }
                }
                // Not a continuation: leave list context
                list_item_indent = None;
            }

            // Regular prose line -- accumulate
            push_prose_line(&mut current_prose, &mut prose_span, &line, true, true);
        }

        // Flush remaining
        flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
        // Unclosed source block at EOF: still emit as Code with empty footer.
        if in_src_block {
            let eof = ByteSpan::new(input.len(), input.len());
            regions.push(SpannedRegion::code(
                input,
                src_lang.take(),
                src_header,
                ByteSpan::new(src_body_start, input.len()),
                eof,
            ));
        }

        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_prose() {
        let input = "Hello world. This is a test.\nAnother line here.";
        let regions = OrgParser.parse(input);
        assert_eq!(
            regions,
            vec![Region::Prose(
                "Hello world. This is a test. Another line here.".to_string()
            )]
        );
    }

    #[test]
    fn preserves_blocks() {
        let input = "Some prose.\n#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\nMore prose.";
        let regions = OrgParser.parse(input);
        assert_eq!(regions.len(), 3);
        assert!(matches!(&regions[0], Region::Prose(_)));
        match &regions[1] {
            Region::Code {
                lang,
                header,
                body,
                footer,
            } => {
                assert_eq!(lang.as_deref(), Some("python"));
                assert_eq!(header, "#+BEGIN_SRC python\n");
                assert_eq!(body, "print('hello')\n");
                assert_eq!(footer, "#+END_SRC\n");
            }
            other => panic!("expected Region::Code, got {other:?}"),
        }
        assert!(matches!(&regions[2], Region::Prose(_)));
    }

    #[test]
    fn preserves_keywords() {
        let input = "#+TITLE: My Document\n#+AUTHOR: Someone\n\nSome text here.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Structure(_)));
        assert!(matches!(&regions[1], Region::Structure(_)));
    }

    #[test]
    fn headline_is_structure_not_prose() {
        let input = "* TODO This is a headline";
        let regions = OrgParser.parse(input);
        assert_eq!(regions.len(), 1);
        assert_eq!(
            regions[0],
            Region::Structure("* TODO This is a headline".to_string())
        );
    }

    #[test]
    fn multi_sentence_headline_stays_one_line() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "** Multi sentence. Second sentence in title\nbody prose. Second body.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.lines()
                .any(|l| l == "** Multi sentence. Second sentence in title"),
            "headline must stay one line, got:\n{out}"
        );
        assert!(
            !out.contains("** Multi sentence.\nSecond"),
            "must not orphan second title sentence without stars:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn headline_trailing_angle_bracket_round_trips() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "* TODO R4 :: snapshot field is Box[T], not Vec[T]\nbody\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Vec[T]"),
            "trailing `>` must survive formatting, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn verbatim_inner_equals_does_not_orphan_closer() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        // The period after `note.` is inside the first span. Closing on the
        // inner `=` would emit a line that starts with `=` and leave the
        // document's markup unterminated.
        let input = "so =x = 1 -- note.= reflows while =s = \"x\"= does not.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "verbatim spans with inner `=` must stay one sentence, got:\n{out}"
        );
        assert!(
            !out.lines().any(|l| l.starts_with('=')),
            "must not orphan a closer onto its own line, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn bold_emphasis_with_period_does_not_become_headline() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "End of first. *Bold spans period. Continues* after.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        // Emphasis with an internal period must stay on one line; splitting
        // would leave a line starting with `*Bold` and a dangling closer.
        let bold_lines: Vec<_> = out
            .lines()
            .filter(|l| l.contains("*Bold") || l.contains("Continues*"))
            .collect();
        assert_eq!(
            bold_lines.len(),
            1,
            "bold emphasis must not split across lines, got:\n{out}"
        );
        assert!(bold_lines[0].contains("*Bold spans period. Continues*"));
        // Org headlines are stars + space; ensure we never introduce one.
        for line in out.lines() {
            let stars = line.chars().take_while(|c| *c == '*').count();
            if stars > 0 {
                let rest = &line[stars..];
                assert!(
                    !rest.starts_with(' ') || rest.trim().is_empty() || line.starts_with("* "),
                    "unexpected star-line: {line}"
                );
            }
        }
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn table_preserved() {
        let input = "| Name | Age |\n|------+-----|\n| Alice | 30 |";
        let regions = OrgParser.parse(input);
        assert!(regions.iter().all(|r| matches!(r, Region::Structure(_))));
    }

    #[test]
    fn list_item_split() {
        let input = "- First item text\n- Second item text";
        let regions = OrgParser.parse(input);
        // Each list item: Structure(marker) + Prose(text) + Structure(\n)
        // The last item has no trailing newline, so no final Structure(\n).
        assert_eq!(regions.len(), 5);
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(regions[1], Region::Prose("First item text".to_string()));
    }

    #[test]
    fn list_item_continuation() {
        let input = "- First sentence of item.\n  Continuation of the same item.\n- Second item";
        let regions = OrgParser.parse(input);
        // First item: Structure("- ") + Prose("First sentence of item. Continuation of the same item.") + Structure("\n")
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("First sentence of item. Continuation of the same item.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        // Second item: Structure("- ") + Prose("Second item") + Structure("\n")
        assert_eq!(regions[3], Region::Structure("- ".to_string()));
        assert_eq!(regions[4], Region::Prose("Second item".to_string()));
    }

    #[test]
    fn drawer_preserved() {
        let input = ":PROPERTIES:\n:ID: abc123\n:END:\nSome text.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Structure(_))); // :PROPERTIES:
        assert!(matches!(&regions[1], Region::Structure(_))); // :ID:
        assert!(matches!(&regions[2], Region::Structure(_))); // :END:
    }

    #[test]
    fn latex_environment_preserved() {
        let input = "Some text.\n\\begin{equation}\nx = 5\n\\end{equation}\nMore text.";
        let regions = OrgParser.parse(input);
        // Prose, Structure(\begin), Structure(x=5), Structure(\end), Prose
        assert!(matches!(&regions[0], Region::Prose(_)));
        assert!(matches!(&regions[1], Region::Structure(s) if s.contains("\\begin{equation}")));
        assert!(matches!(&regions[2], Region::Structure(s) if s.contains("x = 5")));
        assert!(matches!(&regions[3], Region::Structure(s) if s.contains("\\end{equation}")));
        assert!(matches!(&regions[4], Region::Prose(_)));
    }

    #[test]
    fn display_math_preserved() {
        let input = "Some text.\n\\[\nx = 5\n\\]\nMore text.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Prose(_)));
        assert!(matches!(&regions[1], Region::Structure(s) if s.contains("\\[")));
        assert!(matches!(&regions[2], Region::Structure(s) if s.contains("x = 5")));
        assert!(matches!(&regions[3], Region::Structure(s) if s.contains("\\]")));
        assert!(matches!(&regions[4], Region::Prose(_)));
    }

    #[test]
    fn export_snippet_preserved() {
        let input = "Text before.\n@@latex:\\newpage@@\nText after.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Prose(_)));
        assert!(matches!(&regions[1], Region::Structure(s) if s.contains("@@latex:")));
        assert!(matches!(&regions[2], Region::Prose(_)));
    }

    #[test]
    fn nested_latex_envs() {
        let input = "Prose.\n\\begin{align}\na &= b \\\\\nc &= d\n\\end{align}\nMore prose.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Prose(_)));
        // All lines inside align are structure
        let struct_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Structure(_)))
            .count();
        assert!(struct_count >= 4); // \begin, two content lines, \end
    }

    #[test]
    fn list_multi_sentence_hangs_and_rejoins() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "- One. Two.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, "- One.\n  Two.\n");
        let second = format_text(&out, &cfg).unwrap();
        assert_eq!(second, out, "format_text twice must equal once");

        let regions = OrgParser.parse(&out);
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(regions[1], Region::Prose("One. Two.".to_string()));
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions.len(), 3);
    }

    #[test]
    fn nested_list_stays_two_items_after_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "1. Parent one. Parent two.\n   - Child one. Child two.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            "1. Parent one.\n   Parent two.\n   - Child one.\n     Child two.\n"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let regions = OrgParser.parse(&out);
        assert_eq!(regions[0], Region::Structure("1. ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Parent one. Parent two.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions[3], Region::Structure("   - ".to_string()));
        assert_eq!(
            regions[4],
            Region::Prose("Child one. Child two.".to_string())
        );
        assert_eq!(regions[5], Region::Structure("\n".to_string()));
        assert_eq!(regions.len(), 6);
    }
}
