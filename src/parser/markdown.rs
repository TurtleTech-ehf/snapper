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

            // Blockquote: emit the full `> ` / `> > ` prefix as Structure.
            // Checked before list items so nested `> >` is not flattened.
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
                let marker_span = ByteSpan::new(line.start, line.start + marker.len());
                regions.push(SpannedRegion::structure(input, marker_span));
                in_list_item = true;
                list_term = Some(line.terminator_span());
                if !text.is_empty() {
                    current_prose.push_str(text);
                    prose_span = Some(ByteSpan::new(
                        line.start + marker.len(),
                        line.start + line_text.len(),
                    ));
                }
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
                let text = caps.get(2).unwrap().as_str();
                let marker_span = ByteSpan::new(line.start, line.start + marker.len());
                regions.push(SpannedRegion::structure(input, marker_span));
                in_list_item = true;
                list_term = Some(line.terminator_span());
                if !text.is_empty() {
                    current_prose.push_str(text);
                    prose_span = Some(ByteSpan::new(
                        line.start + marker.len(),
                        line.start + line_text.len(),
                    ));
                }
                i += 1;
                continue;
            }

            // Regular prose (also serves as list-item continuation when in_list_item)
            if in_list_item {
                push_prose_line(&mut current_prose, &mut prose_span, line, true, false);
                list_term = Some(line.terminator_span());
            } else {
                push_prose_line(&mut current_prose, &mut prose_span, line, true, true);
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
}
