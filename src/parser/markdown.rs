use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Line, SpannedRegion, flush_prose_spanned, iter_lines, push_prose_line,
};

static HEADING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6}\s+)(.*)$").unwrap());

static FENCED_CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(`{3,}|~{3,})").unwrap());

/// Capture the language token immediately after a fence marker.
/// `lang` is `[A-Za-z0-9_+.-]+`; anything past it (info string) is ignored.
static FENCED_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:`{3,}|~{3,})\s*([A-Za-z0-9_+.\-]+)").unwrap());

static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*(?:[-*+]|\d+[.)]) )(.*)$").unwrap());

/// Markdown blockquote prefix: optional indent plus one or more `> `.
/// Nested `> > text` keeps the full prefix so reflow can repeat it.
static QUOTE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*(?:> )+)(.*)$").unwrap());

/// Match a markdown table row: line whose trimmed form starts and ends with `|`.
/// Also matches separator rows like `|---|---|`.
static TABLE_ROW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\|.*\|\s*$").unwrap());

/// CommonMark setext underline: one or more `=` (level 1) or `-` (level 2),
/// optional leading indent up to three spaces, optional trailing spaces.
static SETEXT_UNDERLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ {0,3}(?:=+|-+)\s*$").unwrap());

pub struct MarkdownParser;

/// Close an open list item: flush accumulated prose and emit the trailing newline.
fn close_list_item(
    in_list_item: &mut bool,
    current_prose: &mut String,
    prose_span: &mut Option<ByteSpan>,
    list_term: &mut Option<ByteSpan>,
    input: &str,
    regions: &mut Vec<SpannedRegion>,
) {
    if *in_list_item {
        flush_prose_spanned(current_prose, prose_span, regions);
        if let Some(span) = list_term.take() {
            if !span.is_empty() {
                regions.push(SpannedRegion::structure(input, span));
            }
        }
        *in_list_item = false;
    }
}

fn starts_html_comment(line: &str) -> bool {
    line.trim_start().starts_with("<!--")
}

fn html_comment_closed(text: &str) -> bool {
    match text.find("<!--") {
        Some(i) => text[i + 4..].contains("-->"),
        None => text.contains("-->"),
    }
}

/// Two or more trailing spaces, or an unescaped trailing backslash.
/// Returns `(content_len, hard_at)` relative to `text`.
fn hard_break_rel(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    if !bytes.is_empty() && *bytes.last().unwrap() == b'\\' {
        let mut n = 0usize;
        let mut i = bytes.len();
        while i > 0 && bytes[i - 1] == b'\\' {
            n += 1;
            i -= 1;
        }
        if n % 2 == 1 {
            return Some((text.len() - 1, text.len() - 1));
        }
        return None;
    }
    let stripped = text.trim_end_matches(' ');
    if text.len() - stripped.len() >= 2 {
        return Some((stripped.len(), stripped.len()));
    }
    None
}

/// Append `line.text[piece_from..]` to the running prose buffer.
///
/// A hard break flushes prose and emits the break (spaces or `\`, plus the
/// line terminator) as Structure so splice copies those source bytes.
fn append_piece(
    prose: &mut String,
    prose_span: &mut Option<ByteSpan>,
    list_term: &mut Option<ByteSpan>,
    line: &Line<'_>,
    piece_from: usize,
    join_space: bool,
    include_term_if_soft: bool,
    input: &str,
    regions: &mut Vec<SpannedRegion>,
) {
    let piece = &line.text[piece_from..];
    if let Some((content_end, hard_at)) = hard_break_rel(piece) {
        let raw = &piece[..content_end];
        let trimmed = raw.trim_start();
        let left = raw.len() - trimmed.len();
        if !trimmed.is_empty() {
            if !prose.is_empty() && join_space {
                prose.push(' ');
            }
            prose.push_str(trimmed);
            let start = line.start + piece_from + left;
            let end = line.start + piece_from + content_end;
            match prose_span {
                None => *prose_span = Some(ByteSpan::new(start, end)),
                Some(s) => s.end = end,
            }
        }
        flush_prose_spanned(prose, prose_span, regions);
        let hard = ByteSpan::new(line.start + piece_from + hard_at, line.end);
        if !hard.is_empty() {
            regions.push(SpannedRegion::structure(input, hard));
        }
        *list_term = None;
        return;
    }
    if piece_from == 0 {
        push_prose_line(prose, prose_span, line, join_space, include_term_if_soft);
        *list_term = if include_term_if_soft {
            None
        } else {
            Some(line.terminator_span())
        };
        return;
    }
    if !piece.is_empty() {
        if !prose.is_empty() && join_space {
            prose.push(' ');
        }
        prose.push_str(piece);
        let start = line.start + piece_from;
        let end = line.start + line.text.len();
        match prose_span {
            None => *prose_span = Some(ByteSpan::new(start, end)),
            Some(s) => s.end = end,
        }
    }
    *list_term = Some(line.terminator_span());
}

/// True when `line` is a CommonMark setext underline (`===` or `---`).
fn is_setext_underline(line: &str) -> bool {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return false;
    }
    SETEXT_UNDERLINE_RE.is_match(trimmed)
}

/// True when `line` may be the text of a setext heading (non-empty, not an ATX
/// marker line, not a table row, not a list item, not a fence opener).
fn is_setext_title_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if HEADING_RE.is_match(line) {
        return false;
    }
    if TABLE_ROW_RE.is_match(line) {
        return false;
    }
    if LIST_ITEM_RE.is_match(line) || QUOTE_RE.is_match(line) {
        return false;
    }
    if FENCED_CODE_RE.is_match(line.trim_start()) {
        return false;
    }
    true
}

impl FormatParser for MarkdownParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let mut regions: Vec<SpannedRegion> = Vec::new();
        let mut current_prose = String::new();
        let mut prose_span: Option<ByteSpan> = None;
        let mut in_fenced_code = false;
        let mut fence_marker = String::new();
        let mut code_header = ByteSpan::default();
        let mut code_body_start = 0usize;
        let mut code_lang: Option<String> = None;
        let mut in_frontmatter = false;
        let mut frontmatter_fence = String::new();
        let mut in_list_item = false;
        let mut list_term: Option<ByteSpan> = None;
        let mut pragma_off = false;

        let lines = iter_lines(input);
        let total = lines.len();
        let mut i = 0;

        while i < total {
            let line: &Line<'_> = &lines[i];
            let line_text = line.text;
            let line_number = i + 1;

            // Check for snapper:off/on pragmas. Inside a fenced code block,
            // the markdown parser does NOT short-circuit on pragmas; the
            // code-block reflow handles them per-language (the markers
            // `#`, `//`, `--`, `;` are all valid pragma prefixes inside
            // their respective languages).
            if !in_fenced_code {
                if let Some(on) = super::check_pragma(line_text) {
                    close_list_item(
                        &mut in_list_item,
                        &mut current_prose,
                        &mut prose_span,
                        &mut list_term,
                        input,
                        &mut regions,
                    );
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    pragma_off = !on;
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }

                if pragma_off {
                    close_list_item(
                        &mut in_list_item,
                        &mut current_prose,
                        &mut prose_span,
                        &mut list_term,
                        input,
                        &mut regions,
                    );
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
            }

            // Front matter detection (only at start of file)
            if line_number == 1 && (line_text.trim() == "---" || line_text.trim() == "+++") {
                in_frontmatter = true;
                frontmatter_fence = line_text.trim().to_string();
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if in_frontmatter {
                if line_text.trim() == frontmatter_fence {
                    in_frontmatter = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // Inside fenced code block
            if in_fenced_code {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let mut closed = false;
                if let Some(caps) = FENCED_CODE_RE.captures(line_text.trim_start()) {
                    let marker = caps.get(1).unwrap().as_str();
                    if marker.chars().next() == fence_marker.chars().next()
                        && marker.len() >= fence_marker.len()
                    {
                        closed = true;
                    }
                }
                if closed {
                    in_fenced_code = false;
                    regions.push(SpannedRegion::code(
                        input,
                        code_lang.take(),
                        code_header,
                        ByteSpan::new(code_body_start, line.start),
                        line.span(),
                    ));
                }
                i += 1;
                continue;
            }

            // Fenced code block start
            if let Some(caps) = FENCED_CODE_RE.captures(line_text.trim_start()) {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                fence_marker = caps.get(1).unwrap().as_str().to_string();
                in_fenced_code = true;
                code_lang = FENCED_LANG_RE
                    .captures(line_text.trim_start())
                    .map(|c| c.get(1).unwrap().as_str().to_string());
                code_header = line.span();
                code_body_start = line.end;
                i += 1;
                continue;
            }

            // Blank line
            if line_text.trim().is_empty() {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::blank(input, line.span()));
                i += 1;
                continue;
            }

            // Heading — keep the entire ATX line as Structure.
            // Splitting into Structure("### ") + Prose(title) let the sentence
            // reflow engine break titles after "1." or mid-phrase, producing
            // orphan headings like:
            //   ### 1.
            //   `cargo binstall` (preferred binary install)
            // CommonMark ATX headings are single-line; do not reflow them.
            if HEADING_RE.is_match(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // Setext heading: title line + underline of `=` or `-`.
            // Without this, title text is Prose and the underline is glued on
            // (or mid-title periods reflow), collapsing the heading.
            if i + 1 < total
                && is_setext_title_line(line_text)
                && is_setext_underline(lines[i + 1].text)
            {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                regions.push(SpannedRegion::structure(input, lines[i + 1].span()));
                i += 2;
                continue;
            }

            // Table row (pipe-delimited)
            if TABLE_ROW_RE.is_match(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // HTML comment block (not a snapper pragma — those are handled above).
            if starts_html_comment(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if html_comment_closed(line_text) {
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
                let start = line.start;
                i += 1;
                while i < total {
                    let done = lines[i].text.contains("-->");
                    i += 1;
                    if done {
                        break;
                    }
                }
                let end = lines
                    .get(i.saturating_sub(1))
                    .map(|l| l.end)
                    .unwrap_or(input.len());
                regions.push(SpannedRegion::structure(input, ByteSpan::new(start, end)));
                continue;
            }

            // Blockquote: emit the full `> ` / `> > ` prefix as Structure.
            // Checked before list items so nested `> >` is not flattened.
            // Each source quote line is its own item so splice ranges stay
            // contiguous. A hard break is Structure; the next line supplies
            // its own `>` (no pre-emitted resume marker).
            if let Some(caps) = QUOTE_RE.captures(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let text = caps.get(2).unwrap().as_str();
                if text.trim().is_empty() {
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
                if HEADING_RE.is_match(text)
                    || TABLE_ROW_RE.is_match(text)
                    || FENCED_CODE_RE.is_match(text.trim_start())
                {
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
                let marker_span = ByteSpan::new(line.start, line.start + marker.len());
                regions.push(SpannedRegion::structure(input, marker_span));
                in_list_item = true;
                append_piece(
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    line,
                    marker.len(),
                    false,
                    false,
                    input,
                    &mut regions,
                );
                i += 1;
                continue;
            }

            // List item: emit marker as Structure, start accumulating text as prose.
            // Continuation lines are appended until a block boundary.
            if let Some(caps) = LIST_ITEM_RE.captures(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let marker_span = ByteSpan::new(line.start, line.start + marker.len());
                regions.push(SpannedRegion::structure(input, marker_span));
                in_list_item = true;
                append_piece(
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    line,
                    marker.len(),
                    false,
                    false,
                    input,
                    &mut regions,
                );
                i += 1;
                continue;
            }

            // Regular prose (also serves as list-item continuation when in_list_item)
            if in_list_item {
                append_piece(
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    line,
                    0,
                    true,
                    false,
                    input,
                    &mut regions,
                );
            } else {
                append_piece(
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    line,
                    0,
                    true,
                    true,
                    input,
                    &mut regions,
                );
            }
            i += 1;
        }

        close_list_item(
            &mut in_list_item,
            &mut current_prose,
            &mut prose_span,
            &mut list_term,
            input,
            &mut regions,
        );
        flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
        // Unclosed fence at EOF: emit a code region with empty footer.
        if in_fenced_code {
            let eof = ByteSpan::new(input.len(), input.len());
            regions.push(SpannedRegion::code(
                input,
                code_lang.take(),
                code_header,
                ByteSpan::new(code_body_start, input.len()),
                eof,
            ));
        }
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Region;

    #[test]
    fn simple_prose() {
        let input = "Hello world. This is a test.\nAnother line here.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(
            regions,
            vec![Region::Prose(
                "Hello world. This is a test. Another line here.".to_string()
            )]
        );
    }

    #[test]
    fn fenced_code_preserved() {
        let input = "Some text.\n```python\nprint('hello')\n```\nMore text.";
        let regions = MarkdownParser.parse(input);
        assert!(matches!(&regions[0], Region::Prose(_)));
        // Code blocks now collapse into a single Region::Code carrying
        // header, body, and footer.
        match &regions[1] {
            Region::Code {
                lang,
                header,
                body,
                footer,
            } => {
                assert_eq!(lang.as_deref(), Some("python"));
                assert_eq!(header, "```python\n");
                assert_eq!(body, "print('hello')\n");
                assert_eq!(footer, "```\n");
            }
            other => panic!("expected Region::Code, got {other:?}"),
        }
        assert!(matches!(&regions[2], Region::Prose(_)));
    }

    #[test]
    fn frontmatter_preserved() {
        let input = "---\ntitle: Test\nauthor: Someone\n---\n\nSome text.";
        let regions = MarkdownParser.parse(input);
        // First 4 lines are structure (frontmatter)
        assert!(matches!(&regions[0], Region::Structure(_)));
        assert!(matches!(&regions[1], Region::Structure(_)));
        assert!(matches!(&regions[2], Region::Structure(_)));
        assert!(matches!(&regions[3], Region::Structure(_)));
    }

    #[test]
    fn table_preserved() {
        let input = "| Feature | Why |\n|---------|-----|\n| `Foo` | Bar |";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().all(|r| matches!(r, Region::Structure(_))),
            "all table rows should be Structure, got: {:?}",
            regions
        );
    }

    #[test]
    fn table_with_surrounding_prose() {
        let input = "Some text before.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nSome text after.";
        let regions = MarkdownParser.parse(input);
        // Should have: Prose, Blank, 3x Structure (table rows), Blank, Prose
        let prose_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Prose(_)))
            .count();
        let structure_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Structure(_)))
            .count();
        assert_eq!(prose_count, 2);
        assert_eq!(structure_count, 3);
    }

    #[test]
    fn wide_table_preserved_verbatim() {
        let input = "| Feature                         | Why excluded                                          | Follow-up article type     |\n|---------------------------------|-------------------------------------------------------|----------------------------|\n| `DraftValidation`               | LLM-assisted; needs API key, not production-reliable  | Step-by-Step Project       |";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions.len(), 3);
        assert!(regions.iter().all(|r| matches!(r, Region::Structure(_))));
        // Verify each line is preserved exactly (with trailing newline)
        for r in &regions {
            if let Region::Structure(s) = r {
                assert!(s.starts_with('|'));
                assert!(
                    s.ends_with('|') || s.ends_with("|\n"),
                    "table row must be the input slice: {s:?}"
                );
            }
        }
    }

    #[test]
    fn list_item_continuation_joined() {
        let input = "1. First line of item\ncontinuation text here.\nAnother sentence.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions[0], Region::Structure("1. ".to_string()));
        // All three lines should be joined into one Prose region
        assert_eq!(
            regions[1],
            Region::Prose(
                "First line of item continuation text here. Another sentence.".to_string()
            )
        );
        // No trailing newline in the source, so no terminator Structure.
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn list_item_continuation_stops_at_blank() {
        let input = "- Item one text.\ncontinuation.\n\nParagraph after.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Item one text. continuation.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert!(matches!(&regions[3], Region::BlankLines(_)));
        assert_eq!(regions[4], Region::Prose("Paragraph after.".to_string()));
    }

    #[test]
    fn list_item_continuation_stops_at_next_item() {
        let input = "- First item\ncontinuation.\n- Second item";
        let regions = MarkdownParser.parse(input);
        // First item
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("First item continuation.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        // Second item
        assert_eq!(regions[3], Region::Structure("- ".to_string()));
        assert_eq!(regions[4], Region::Prose("Second item".to_string()));
        assert_eq!(regions.len(), 5);
    }

    #[test]
    fn numbered_list_with_backtick_continuation() {
        // The exact bug from the user report
        let input = "1. **Quality gates:** `Thresholds(warning=0.1)`\nlets you express failure rates. Replaces binary assert.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions[0], Region::Structure("1. ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose(
                "**Quality gates:** `Thresholds(warning=0.1)` lets you express failure rates. Replaces binary assert.".to_string()
            )
        );
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn heading_is_structure_not_prose() {
        let input = "## My Heading";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Region::Structure("## My Heading".to_string()));
    }

    #[test]
    fn numbered_atx_heading_with_code_stays_one_line() {
        // Regression: rtrash README / snapper-25kc — snapper -i turned
        // `### 1. \`cargo binstall\` (preferred binary install)` into an orphan
        // `### 1.` plus a reflowed title paragraph.
        let input = "### 1. `cargo binstall` (preferred binary install)\n\nBody sentence one. Body sentence two.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            matches!(&regions[0], Region::Structure(s) if s == "### 1. `cargo binstall` (preferred binary install)\n"),
            "expected full ATX line as Structure, got: {:?}",
            regions[0]
        );
        // Title must not appear as Prose (would be sentence-reflowed).
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("cargo binstall"))),
            "heading title must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn atx_heading_levels_preserved_verbatim() {
        for hashes in 1..=6 {
            let marks = "#".repeat(hashes);
            let line = format!("{marks} Title with `code` and (parens)");
            let regions = MarkdownParser.parse(&line);
            assert_eq!(
                regions,
                vec![Region::Structure(line.clone())],
                "level {hashes}"
            );
        }
    }

    #[test]
    fn setext_heading_equals_is_structure() {
        let input = "Setext Title With Period. Still Title\n=====================================\n\nBody after setext.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            matches!(&regions[0], Region::Structure(s) if s == "Setext Title With Period. Still Title\n"),
            "setext title must be Structure, got: {:?}",
            regions[0]
        );
        assert!(
            matches!(&regions[1], Region::Structure(s) if s.starts_with('=')),
            "setext underline must be Structure, got: {:?}",
            regions[1]
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Still Title"))),
            "setext title must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn setext_heading_dashes_is_structure() {
        let input = "Secondary Setext Title\n----------------------\n\nParagraph text here.\n";
        let regions = MarkdownParser.parse(input);
        assert_eq!(
            regions[0],
            Region::Structure("Secondary Setext Title\n".to_string())
        );
        assert!(matches!(&regions[1], Region::Structure(s) if s.starts_with('-')));
    }

    #[test]
    fn multi_sentence_setext_title_stays_one_line() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "Setext Title With Period. Still Title\n=====================================\n\nBody after setext. Second body.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.starts_with(
                "Setext Title With Period. Still Title\n=====================================\n"
            ),
            "setext title+underline must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("Still Title =====") && !out.contains("Still Title\nStill"),
            "must not glue underline onto reflowed title:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn setext_after_prose_flushes_body() {
        let input = "Body sentence one. Body two.\n\nHeading Here\n============\n";
        let regions = MarkdownParser.parse(input);
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(p) => Some(p.as_str()),
                _ => None,
            })
            .collect();
        assert!(prose.iter().any(|p| p.contains("Body sentence one")));
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "Heading Here\n"))
        );
    }

    #[test]
    fn blockquote_marker_is_structure() {
        let input = "> One. Two.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions[0], Region::Structure("> ".to_string()));
        assert_eq!(regions[1], Region::Prose("One. Two.".to_string()));
        // No trailing newline in the source, so no terminator Structure.
        assert_eq!(regions.len(), 2);
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains('>'))),
            "quote marker must not leak into Prose: {regions:?}"
        );
    }

    #[test]
    fn blockquote_multiline_keeps_each_marker() {
        // Each source quote line is its own item so splice ranges stay
        // contiguous. Reflow of already-split quotes is identity.
        let regions = MarkdownParser.parse("> One.\n> Two.");
        assert_eq!(regions[0], Region::Structure("> ".to_string()));
        assert_eq!(regions[1], Region::Prose("One.".to_string()));
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions[3], Region::Structure("> ".to_string()));
        assert_eq!(regions[4], Region::Prose("Two.".to_string()));
    }

    #[test]
    fn list_and_quote_multi_sentence_hangs() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let dash = format_text("- One. Two.\n", &cfg).unwrap();
        assert_eq!(dash, "- One.\n  Two.\n");
        assert_eq!(format_text(&dash, &cfg).unwrap(), dash);

        let numbered = format_text("1. One. Two.\n", &cfg).unwrap();
        assert_eq!(numbered, "1. One.\n   Two.\n");
        assert_eq!(format_text(&numbered, &cfg).unwrap(), numbered);

        let quote = format_text("> One. Two.\n", &cfg).unwrap();
        assert_eq!(quote, "> One.\n> Two.\n");
        assert_eq!(format_text(&quote, &cfg).unwrap(), quote);
    }

    #[test]
    fn blockquote_keeps_marker_on_each_content_line() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        for input in ["> One. Two.\n", "> One.\n> Two.\n"] {
            let out = format_text(input, &cfg).unwrap();
            let quote_lines: Vec<_> = out.lines().filter(|l| !l.is_empty()).collect();
            assert_eq!(
                quote_lines,
                vec!["> One.", "> Two."],
                "each content line needs `>`, input {input:?}, got:\n{out}"
            );
            assert_eq!(format_text(&out, &cfg).unwrap(), out);
        }
    }

    #[test]
    fn nested_blockquote_keeps_full_prefix() {
        let input = "> > Nested one. Nested two.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions[0], Region::Structure("> > ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Nested one. Nested two.".to_string())
        );
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn nested_blockquote_reflow_repeats_prefix() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "> Quoted one. Quoted two.\n> > Nested one. Nested two.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            "> Quoted one.\n> Quoted two.\n> > Nested one.\n> > Nested two.\n"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn nested_list_stays_two_items_after_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "1. Parent one. Parent two.\n   - Child one. Child two.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            "1. Parent one.\n   Parent two.\n   - Child one.\n     Child two.\n"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let regions = MarkdownParser.parse(&out);
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
    }

    #[test]
    fn hard_break_two_spaces_not_joined_with_space() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "line  \ncontinued. Next sentence.\n";
        let regions = MarkdownParser.parse(input);
        let joined: String = regions
            .iter()
            .map(|r| match r {
                Region::Prose(p) | Region::Structure(p) | Region::BlankLines(p) => p.as_str(),
                Region::Code { .. } => "",
            })
            .collect();
        assert!(
            !joined.contains("line continued"),
            "two trailing spaces are a hard break, not a space join: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| match r {
                Region::Structure(s) => s.contains("  \n") || s.ends_with("  \n"),
                _ => false,
            }) || joined.contains("line  \n"),
            "hard-break spaces must survive classification: {regions:?}"
        );

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("line continued"),
            "must not collapse hard break to a space, got:\n{out}"
        );
        assert!(
            out.contains("line  \n") || out.contains("line  \r"),
            "two trailing spaces must remain, got:\n{out:?}"
        );
        assert!(out.contains("Next sentence."));
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn hard_break_backslash_not_joined_with_space() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "line\\\ncontinued. Next sentence.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("line continued") && !out.contains("line\\ continued"),
            "backslash hard break must not become a space, got:\n{out}"
        );
        assert!(
            out.contains("line\\\ncontinued"),
            "backslash hard break must remain, got:\n{out:?}"
        );
        assert!(out.contains("Next sentence."));
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn html_comment_multiline_is_structure() {
        let input = "Before sentence. After.\n<!--\nHidden. With a period.\nStill comment.\n-->\nMore. Text.";
        let regions = MarkdownParser.parse(input);
        let comment = regions.iter().find_map(|r| match r {
            Region::Structure(s) if s.contains("<!--") => Some(s.as_str()),
            _ => None,
        });
        let comment = comment.expect(&format!("comment must be Structure, got {regions:?}"));
        assert!(comment.contains("<!--"), "{comment}");
        assert!(comment.contains("Hidden. With a period."), "{comment}");
        assert!(comment.contains("Still comment."), "{comment}");
        assert!(comment.contains("-->"), "{comment}");
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Hidden") || p.contains("Still comment"))),
            "comment body must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn html_comment_multiline_passes_through_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "Before sentence. After.\n<!--\nHidden. With a period.\nStill comment.\n-->\nMore. Text.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("<!--\nHidden. With a period.\nStill comment.\n-->\n"),
            "multiline comment must pass through, got:\n{out}"
        );
        assert!(out.contains("Before sentence.\nAfter."));
        assert!(out.contains("More.\nText."));
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn html_comment_pragma_still_disables_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "Hello world. Goodbye world.\n<!-- snapper:off -->\nKeep this. Exactly here.\n<!-- snapper:on -->\nFinal thing. Last sentence.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(out.contains("Hello world.\nGoodbye world.\n"));
        assert!(
            out.contains("Keep this. Exactly here.\n"),
            "pragma-off body must stay untouched, got:\n{out}"
        );
        assert!(out.contains("Final thing.\nLast sentence."));
        assert!(out.contains("<!-- snapper:off -->"));
        assert!(out.contains("<!-- snapper:on -->"));
    }

    #[test]
    fn quote_hard_break_then_nonquote_has_no_stray_marker() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "> line  \nNext sentence.\n";
        let regions = MarkdownParser.parse(input);
        let after_break = regions
            .iter()
            .skip_while(|r| !matches!(r, Region::Structure(s) if s.ends_with("  \n")));
        assert!(
            !after_break
                .clone()
                .any(|r| matches!(r, Region::Structure(s) if is_quote_resume(s))),
            "must not emit `>` after a quote hard break into non-quote, got: {regions:?}"
        );

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("> \n") && !out.lines().any(|l| l == ">" || l.trim() == ">"),
            "stray empty quote line, got:\n{out:?}"
        );
        assert!(
            out.starts_with("> line  \nNext sentence."),
            "hard break then non-quote body, got:\n{out:?}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let still_quote = format_text("> line  \n> continued.\n", &cfg).unwrap();
        assert!(
            still_quote.starts_with("> line  \n> continued."),
            "in-quote hard break must still resume `>`, got:\n{still_quote:?}"
        );
    }

    fn is_quote_resume(s: &str) -> bool {
        !s.is_empty()
            && !s.contains('\n')
            && s.contains('>')
            && s.bytes().all(|b| b == b'>' || b == b' ')
    }

    #[test]
    fn quote_wrap_repeats_prefix_under_max_width() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "> One two three four five six seven eight.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            max_width: 20,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        let lines: Vec<_> = out.lines().filter(|l| !l.is_empty()).collect();
        assert!(
            lines.len() > 1,
            "sentence must wrap under max_width=20, got:\n{out}"
        );
        for line in &lines {
            assert!(
                line.starts_with("> "),
                "every wrap line keeps `>`, not a space hang, got:\n{out}"
            );
            assert!(
                line.chars().count() <= 20,
                "prefix counts toward max_width: {line:?} ({out})"
            );
        }
        assert!(
            !out.contains("\n  "),
            "must not hang quote wrap with spaces: {out:?}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }
}
