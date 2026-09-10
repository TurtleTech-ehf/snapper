use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Region, RegionOrigin, SpannedRegion, flush_prose_spanned, iter_lines,
    join_prose_gap, push_prose_line,
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

/// org-element / org-mode display math (`$$` or `\[`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayMathDelim {
    Bracket,
    Dollars,
}

/// Greater-block kind. Quote/verse/center contain paragraphs;
/// example/export/comment and other names stay literal Structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GreaterKind {
    Container,
    Opaque,
}

#[derive(Debug, Clone)]
struct OpenGreater {
    name: String,
    kind: GreaterKind,
}

pub struct OrgParser;

impl OrgParser {
    /// First token after `#+BEGIN_` / `#+END_`, uppercased (org-element NAME).
    fn block_directive_name(line: &str, prefix: &str) -> Option<String> {
        let trimmed = line.trim_start();
        let upper = trimmed.to_ascii_uppercase();
        if !upper.starts_with(prefix) {
            return None;
        }
        let name = trimmed[prefix.len()..].split_whitespace().next()?;
        if name.is_empty() {
            return None;
        }
        Some(name.to_ascii_uppercase())
    }

    fn block_begin_name(line: &str) -> Option<String> {
        Self::block_directive_name(line, "#+BEGIN_")
    }

    fn block_end_name(line: &str) -> Option<String> {
        Self::block_directive_name(line, "#+END_")
    }

    /// Check if a line starts a block (#+BEGIN_NAME).
    fn is_block_begin(line: &str) -> bool {
        Self::block_begin_name(line).is_some()
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

    /// Check if a line ends a block (#+END_NAME).
    fn is_block_end(line: &str) -> bool {
        Self::block_end_name(line).is_some()
    }

    /// Check if a line ends a source code block (`#+END_SRC` only).
    fn is_src_end(line: &str) -> bool {
        Self::block_end_name(line).as_deref() == Some("SRC")
    }

    /// org-element drawer: `:NAME:` with NAME = `[A-Za-z_-]+`. Not `:END:`.
    fn is_drawer_begin(line: &str) -> bool {
        let trimmed = line.trim();
        let Some(name) = trimmed.strip_prefix(':').and_then(|s| s.strip_suffix(':')) else {
            return false;
        };
        !name.is_empty()
            && !name.eq_ignore_ascii_case("END")
            && name
                .bytes()
                .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-'))
    }

    /// Check if a line ends a drawer
    fn is_drawer_end(line: &str) -> bool {
        line.trim().eq_ignore_ascii_case(":END:")
    }

    /// org-element fixed-width: `: ` payload or a lone `:`.
    fn is_fixed_width(line: &str) -> bool {
        let t = line.trim_start_matches([' ', '\t']);
        t == ":" || t.starts_with(": ")
    }

    /// org-element horizontal rule: five or more dashes.
    fn is_horizontal_rule(line: &str) -> bool {
        let t = line.trim();
        t.len() >= 5 && t.bytes().all(|b| b == b'-')
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

    /// org-element latex-fragment display math: `\[` / `\]` or `$$` / `$$`.
    fn display_math_open(line: &str) -> Option<DisplayMathDelim> {
        let t = line.trim();
        if t == r"\[" {
            Some(DisplayMathDelim::Bracket)
        } else if t.starts_with("$$") {
            Some(DisplayMathDelim::Dollars)
        } else {
            None
        }
    }

    /// A line that is only `$$` is an opener, not a one-line `$$...$$` block.
    fn display_math_is_single_line(line: &str, delim: DisplayMathDelim) -> bool {
        match delim {
            DisplayMathDelim::Bracket => false,
            DisplayMathDelim::Dollars => {
                let t = line.trim();
                t != "$$" && t.ends_with("$$")
            }
        }
    }

    fn is_display_math_close(line: &str, delim: DisplayMathDelim) -> bool {
        match delim {
            DisplayMathDelim::Bracket => line.trim() == r"\]",
            DisplayMathDelim::Dollars => line.trim_end().ends_with("$$"),
        }
    }

    /// Check if a line is entirely an inline export snippet (@@backend:...@@)
    fn is_export_snippet_line(line: &str) -> bool {
        let trimmed = line.trim();
        EXPORT_SNIPPET_RE.is_match(trimmed) && trimmed.starts_with("@@")
    }

    /// org-element quote-block / verse-block / center-block contain paragraphs.
    fn is_container_block_name(name: &str) -> bool {
        matches!(name, "QUOTE" | "VERSE" | "CENTER")
    }

    fn greater_kind(name: &str) -> GreaterKind {
        if Self::is_container_block_name(name) {
            GreaterKind::Container
        } else {
            GreaterKind::Opaque
        }
    }

    fn inside_opaque(stack: &[OpenGreater]) -> bool {
        stack.iter().any(|b| b.kind == GreaterKind::Opaque)
    }

    fn innermost_container(stack: &[OpenGreater]) -> Option<&str> {
        stack
            .iter()
            .rev()
            .find(|b| b.kind == GreaterKind::Container)
            .map(|b| b.name.as_str())
    }

    fn push_greater(stack: &mut Vec<OpenGreater>, name: String) {
        let kind = Self::greater_kind(&name);
        stack.push(OpenGreater { name, kind });
    }

    fn pop_matching_greater(stack: &mut Vec<OpenGreater>, end_name: &str) {
        if stack.last().is_some_and(|b| b.name == end_name) {
            stack.pop();
        }
    }
}

impl FormatParser for OrgParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let mut regions: Vec<SpannedRegion> = Vec::new();
        let mut current_prose = String::new();
        let mut prose_span: Option<ByteSpan> = None;
        // Open greater-element names. `#+END_NAME` pops only a matching top
        // (org-element / orgize); a mismatched closer stays structure.
        // Quote/verse/center are containers (inner Prose); other names are opaque.
        let mut block_stack: Vec<OpenGreater> = Vec::new();
        let mut in_src_block = false;
        let mut src_lang: Option<String> = None;
        let mut src_header = ByteSpan::default();
        let mut src_body_start = 0usize;
        let mut in_drawer = false;
        let mut in_latex_env: Option<String> = None;
        let mut in_display_math: Option<DisplayMathDelim> = None;
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

            // Inside an opaque greater block -- everything is structure.
            // Quote/verse/center are containers: fall through and parse inner
            // regions (Prose, SRC, nested opaque blocks).
            if Self::inside_opaque(&block_stack) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if let Some(end_name) = Self::block_end_name(line_text) {
                    Self::pop_matching_greater(&mut block_stack, &end_name);
                } else if let Some(begin_name) = Self::block_begin_name(line_text) {
                    Self::push_greater(&mut block_stack, begin_name);
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

            // Inside display math \[...\] / $$...$$ -- everything is structure
            if let Some(delim) = in_display_math {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_display_math_close(line_text, delim) {
                    in_display_math = None;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Source block begin (#+BEGIN_SRC LANG ...)
            if let Some(lang) = Self::is_src_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_src_block = true;
                src_lang = lang;
                src_header = line.span();
                src_body_start = line.end;
                continue;
            }

            // #+BEGIN_NAME: container open (quote/verse/center) or opaque.
            if let Some(name) = Self::block_begin_name(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                Self::push_greater(&mut block_stack, name);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // #+END_NAME: matching container closer, or mismatched Structure.
            if let Some(end_name) = Self::block_end_name(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                Self::pop_matching_greater(&mut block_stack, &end_name);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Drawer begin (`:NAME:` only; `:See also:` is not a name)
            if Self::is_drawer_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_drawer = true;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Fixed-width (`: text`) is Structure, not a drawer.
            if Self::is_fixed_width(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Horizontal rule (`-----`) is Structure and a paragraph boundary.
            if Self::is_horizontal_rule(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
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

            // Display math open (\[ or $$)
            if let Some(delim) = Self::display_math_open(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if !Self::display_math_is_single_line(line_text, delim) {
                    in_display_math = Some(delim);
                }
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
                                join_prose_gap(prose);
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

            // Regular prose line -- accumulate. Verse stays line-preserving.
            if Self::innermost_container(&block_stack) == Some("VERSE") {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                push_prose_line(&mut current_prose, &mut prose_span, &line, true, true);
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            } else {
                push_prose_line(&mut current_prose, &mut prose_span, &line, true, true);
            }
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
                "Hello world. This is a test.\nAnother line here.".to_string()
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
        // First item: Structure("- ") + Prose + Structure("\n")
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("First sentence of item.\nContinuation of the same item.".to_string())
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
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Some text."))),
            "text after :END: must stay Prose, got: {regions:?}"
        );
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

    fn org_cfg() -> crate::FormatConfig {
        crate::FormatConfig {
            format: crate::format::Format::Org,
            ..Default::default()
        }
        .without_safety_backstops()
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
    fn dollar_dollar_display_math_is_structure_not_prose() {
        let input = "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("This is a long sentence that must stay inside display math")
            )),
            "$$ body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This is a long sentence that must stay inside display math")
            )),
            "$$ body must not be Prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "$$")),
            "$$ delimiters must be Structure, got: {regions:?}"
        );
    }

    #[test]
    fn dollar_dollar_display_math_does_not_reflow_as_prose() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains(
                "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$"
            ),
            "$$ display math must stay a structure block, got:\n{out}"
        );
        assert!(
            !out.contains("$$ This is a long sentence"),
            "must not join $$ into surrounding prose, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn dollar_dollar_two_sentences_do_not_split() {
        use crate::format_text;

        let input =
            "$$\nFirst sentence. Second sentence that would split if this were prose.\n$$\n";
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(
                "$$\nFirst sentence. Second sentence that would split if this were prose.\n$$"
            ),
            "$$ display math must stay a structure block, got:\n{out}"
        );
        assert!(
            !out.contains("$$ First sentence"),
            "must not join $$ into surrounding prose, got:\n{out}"
        );
        assert!(
            !out.contains("First sentence.\nSecond sentence"),
            "$$ body must not split at sentence end, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn dollar_dollar_single_line_display_is_structure() {
        use crate::format_text;

        let input = "Before the math. More before.\n$$E = mc^2$$\nAfter the math. More after.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("$$E = mc^2$$"))),
            "single-line $$...$$ must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("E = mc^2"))),
            "single-line $$ body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("$$E = mc^2$$"),
            "single-line $$ must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("Before the math.\nMore before."),
            "surrounding prose must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn bracket_display_math_still_structure() {
        use crate::format_text;

        let input = "\\[\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n\\]\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long sentence that must stay inside display math")
            )),
            "\\[ body must stay Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("This is a long sentence that must stay inside display math")
            )),
            "\\[ body must not become Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(
                "\\[\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n\\]"
            ),
            "\\[ display math must stay a structure block, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn inline_single_dollar_math_is_still_prose() {
        use crate::format_text;

        let input = "See $x = 1$ here. Next sentence.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("$x = 1$"))),
            "inline $...$ must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("See $x = 1$ here.\nNext sentence."),
            "inline $...$ must not open display math, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
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
        assert_eq!(regions[1], Region::Prose("One.\nTwo.".to_string()));
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
            Region::Prose("Parent one.\nParent two.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions[3], Region::Structure("   - ".to_string()));
        assert_eq!(
            regions[4],
            Region::Prose("Child one.\nChild two.".to_string())
        );
        assert_eq!(regions[5], Region::Structure("\n".to_string()));
        assert_eq!(regions.len(), 6);
    }

    /// Quote containing example: `#+END_EXAMPLE` must not close the quote.
    fn quote_with_nested_example() -> &'static str {
        concat!(
            "#+BEGIN_QUOTE\n",
            "Quoted one. Quoted two.\n",
            "#+BEGIN_EXAMPLE\n",
            "foo. bar.\n",
            "#+END_EXAMPLE\n",
            "Still quoted. More quoted.\n",
            "#+END_QUOTE\n",
            "After. Next.\n",
        )
    }

    #[test]
    fn nested_example_does_not_close_quote_by_any_end() {
        let input = quote_with_nested_example();
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_QUOTE"))),
            "quote opener must stay Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("#+END_QUOTE"))),
            "matching #+END_QUOTE must stay Structure, not prose: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Quoted one.") && p.contains("Quoted two.")
            )),
            "quote body must be Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Still quoted") && p.contains("More quoted")
            )),
            "post-example quote body must stay Prose inside the quote: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("foo. bar."))),
            "EXAMPLE body must stay Structure: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("#+END_QUOTE") || p.contains("foo. bar.")
            )),
            "quote closer and EXAMPLE body must not become Prose: {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            prose
                .iter()
                .any(|p| p.contains("After.") && p.contains("Next.")),
            "prose after the quote must remain Prose, got: {regions:?}"
        );
    }

    #[test]
    fn nested_example_in_quote_closes_by_name_only() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = quote_with_nested_example();
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Quoted one.\nQuoted two."),
            "quoted sentences must reflow inside the quote fence, got:\n{out}"
        );
        assert!(
            out.contains("foo. bar."),
            "EXAMPLE body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("foo.\nbar."),
            "EXAMPLE must stay literal, got:\n{out}"
        );
        assert!(
            out.contains("Still quoted.\nMore quoted."),
            "text after nested EXAMPLE must reflow inside the quote, got:\n{out}"
        );
        assert!(
            out.contains("#+END_QUOTE\nAfter.\nNext.\n")
                || out.ends_with("#+END_QUOTE\nAfter.\nNext."),
            "real closer stays a fence; following prose reflows, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    /// Ticket fixture (Format::Org): quote inner is Prose.
    fn quote_inner_prose_fixture() -> &'static str {
        concat!(
            "#+BEGIN_QUOTE\n",
            "Quoted one. Quoted two.\n",
            "#+END_QUOTE\n",
        )
    }

    /// Ticket fixture (Format::Org): nested SRC inside quote.
    fn quote_nested_src_fixture() -> &'static str {
        concat!(
            "#+BEGIN_QUOTE\n",
            "Before.\n",
            "\n",
            "#+BEGIN_SRC python\n",
            "print(\"a. b\")\n",
            "#+END_SRC\n",
            "\n",
            "After quote. More.\n",
            "#+END_QUOTE\n",
        )
    }

    #[test]
    fn quote_block_inner_is_prose_not_structure() {
        let input = quote_inner_prose_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_QUOTE"))),
            "quote opener must stay Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("#+END_QUOTE"))),
            "quote closer must stay Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Quoted one.") && p.contains("Quoted two.")
            )),
            "quote inner must be Prose, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Quoted one."))),
            "quote inner must not be Structure, got: {regions:?}"
        );
    }

    #[test]
    fn quote_block_inner_prose_reflows() {
        use crate::format_text;

        let input = quote_inner_prose_fixture();
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("#+BEGIN_QUOTE\nQuoted one.\nQuoted two.\n#+END_QUOTE"),
            "quoted sentences must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn quote_block_nested_src_closes_only_src() {
        use crate::format_text;

        let input = quote_nested_src_fixture();
        let regions = OrgParser.parse(input);
        match regions.iter().find(|r| matches!(r, Region::Code { .. })) {
            Some(Region::Code {
                lang,
                body,
                header,
                footer,
            }) => {
                assert_eq!(lang.as_deref(), Some("python"));
                assert!(header.contains("#+BEGIN_SRC"));
                assert!(body.contains("print(\"a. b\")"));
                assert!(footer.contains("#+END_SRC"));
            }
            other => panic!("nested SRC must be Code, got {regions:?} (found {other:?})"),
        }
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("After quote.") && p.contains("More.")
            )),
            "quote prose after SRC must stay Prose, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("print"))),
            "SRC body must not leak as Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("print(\"a. b\")"),
            "SRC body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("a.\nb"),
            "SRC body must not split at the period, got:\n{out}"
        );
        assert!(
            out.contains("After quote.\nMore."),
            "quote prose after SRC must reflow, got:\n{out}"
        );
        assert!(
            out.contains("#+BEGIN_QUOTE") && out.contains("#+END_QUOTE"),
            "quote fences must remain, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn verse_block_inner_is_line_preserving_prose() {
        use crate::format_text;

        let input = "#+BEGIN_VERSE\nGreat clouds overhead\nTiny black birds rise and fall\n#+END_VERSE\nAfter. Next.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Great clouds overhead"))),
            "verse inner must be Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Great clouds overhead")
            )),
            "verse inner must not be Structure, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("Great clouds overhead\nTiny black birds rise and fall"),
            "verse lines must stay separate, got:\n{out}"
        );
        assert!(
            !out.contains("Great clouds overhead Tiny black birds"),
            "verse must not join lines with a space, got:\n{out}"
        );
        assert!(
            out.contains("#+END_VERSE\nAfter.\nNext."),
            "prose after verse must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn center_block_inner_prose_reflows() {
        use crate::format_text;

        let input = "#+BEGIN_CENTER\nCentered one. Centered two.\n#+END_CENTER\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Centered one.") && p.contains("Centered two.")
            )),
            "center inner must be Prose, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Centered one."))),
            "center inner must not be Structure, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("#+BEGIN_CENTER\nCentered one.\nCentered two.\n#+END_CENTER"),
            "center inner must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn example_export_comment_do_not_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        for (name, body) in [
            ("EXAMPLE", "foo. bar."),
            ("EXPORT", "<p>Hello. World.</p>"),
            ("COMMENT", "Secret one. Secret two."),
        ] {
            let input = format!("#+BEGIN_{name}\n{body}\n#+END_{name}\nAfter. Next.\n");
            let out = format_text(&input, &cfg).unwrap();
            assert!(
                out.contains(body),
                "{name} body must not reflow, got:\n{out}"
            );
            assert!(
                out.contains(&format!("#+END_{name}\nAfter.\nNext.")),
                "{name} closer is name-matched; following prose reflows, got:\n{out}"
            );
            assert_eq!(format_text(&out, &cfg).unwrap(), out);
        }
    }

    /// Ticket fixture (Format::Org): `:See also:` is not a drawer,
    /// `: text` is fixed-width, `-----` is a rule.
    fn drawer_fixed_width_rule_fixture() -> &'static str {
        concat!(
            ":See also:\n",
            "This is a note. Second sentence.\n",
            "After the note. More.\n",
            "\n",
            ": First sentence. Second sentence.\n",
            "\n",
            "End of section.\n",
            "-----\n",
            "Start of next. More.\n",
        )
    }

    #[test]
    fn see_also_is_not_a_drawer() {
        use crate::format_text;

        let input = drawer_fixed_width_rule_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This is a note.") && p.contains("Second sentence.")
            )),
            ":See also: must not swallow following prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a note.")
            )),
            "notes after :See also: must not be drawer Structure, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("This is a note.\nSecond sentence."),
            ":See also: must not swallow notes as a drawer, got:\n{out}"
        );
        assert!(
            out.contains("After the note.\nMore."),
            "prose after :See also: must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn fixed_width_colon_space_is_structure() {
        use crate::format_text;

        // Isolated from any `:NAME:` so origin/main cannot pass by swallowing.
        let input = ": First sentence. Second sentence.\nAfter fixed. More after.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(": First sentence. Second sentence.")
            )),
            "fixed-width : text must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("First sentence.")
            )),
            "fixed-width must not be Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("After fixed.") && p.contains("More after.")
            )),
            "prose after fixed-width must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(": First sentence. Second sentence."),
            "fixed-width must keep the colon and not wrap, got:\n{out}"
        );
        assert!(
            !out.contains("First sentence.\nSecond sentence."),
            "fixed-width must not split as prose, got:\n{out}"
        );
        assert!(
            out.contains("After fixed.\nMore after."),
            "prose after fixed-width must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn horizontal_rule_is_structure_boundary() {
        use crate::format_text;

        let input = "End of section.\n-----\nStart of next. More.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "-----")),
            "----- must be Structure (rule), got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("-----"))),
            "----- must not join surrounding prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("End of section.\n-----\nStart of next."),
            "----- must stay a rule boundary, got:\n{out}"
        );
        assert!(
            out.contains("Start of next.\nMore."),
            "prose after the rule must reflow independently, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn named_drawer_still_structure_until_end() {
        use crate::format_text;

        let input = ":LOGBOOK:\nCLOCK: [2026-01-01] First. Second.\n:END:\nAfter drawer. More.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(":LOGBOOK:"))),
            "named drawer opener must be Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("CLOCK:") && s.contains("First. Second.")
            )),
            "drawer body must stay Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First. Second."))),
            "drawer body must not become Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(":LOGBOOK:\nCLOCK: [2026-01-01] First. Second.\n:END:"),
            "real :NAME: drawer must not reflow, got:\n{out}"
        );
        assert!(
            out.contains("After drawer.\nMore."),
            "prose after :END: must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn gyoz_fixture_does_not_swallow_or_drop_colon() {
        use crate::format_text;

        let input = drawer_fixed_width_rule_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(": First sentence. Second sentence.")
            )),
            "fixed-width line in the ticket fixture must be Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "-----")),
            "rule in the ticket fixture must be Structure, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("This is a note.\nSecond sentence."),
            ":See also: must not swallow following prose, got:\n{out}"
        );
        assert!(
            out.contains("After the note.\nMore."),
            "sentences after the note must reflow, got:\n{out}"
        );
        assert!(
            out.lines()
                .any(|l| l == ": First sentence. Second sentence."),
            "fixed-width must keep the leading colon, got:\n{out}"
        );
        assert!(
            out.contains("End of section.\n-----\nStart of next.\nMore."),
            "rule is a boundary; following prose reflows, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }
}
