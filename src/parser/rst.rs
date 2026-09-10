use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Line, Region, SpannedRegion, flush_prose_spanned, iter_lines,
    push_prose_line,
};

/// Match `.. code-block:: LANG` or `.. sourcecode:: LANG` (or `.. code:: LANG`).
static CODE_DIRECTIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*\.\.\s+(?:code-block|sourcecode|code)::\s*([A-Za-z0-9_+.\-]+)?\s*$").unwrap()
});

pub struct RstParser;

impl FormatParser for RstParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        parse_line_based(input)
    }
}

/// Line-based RST parser. Handles directives, literal blocks, sections,
/// field lists, comments, tables, definition lists, and block-quote
/// hang spaces as structure regions.
fn parse_line_based(input: &str) -> Vec<SpannedRegion> {
    let mut regions = Vec::new();
    let mut current_prose = String::new();
    let mut prose_span: Option<ByteSpan> = None;
    let mut in_literal_block = false;
    let mut literal_indent: usize = 0;
    let mut in_directive = false;
    let mut directive_indent: usize = 0;
    let mut in_definition = false;
    let mut definition_indent: usize = 0;
    // Comment body: indented lines after `..` / `.. text` stay Structure.
    let mut in_comment = false;
    let mut comment_indent: usize = 0;
    // Hang column of the current list item (`- ` → 2) or block quote.
    // Continuation paragraphs after a blank stay in the item when
    // indented this far.
    let mut list_hang: Option<usize> = None;
    let mut pragma_off = false;

    // Code-block directive bookkeeping. Mutually exclusive with `in_directive`.
    let mut in_code_block = false;
    let mut code_indent: usize = 0;
    let mut code_lang: Option<String> = None;
    let mut code_header = ByteSpan::default();
    let mut code_body_start = 0usize;
    let mut code_body_end = 0usize;
    let mut code_footer_start: Option<usize> = None;

    let lines = iter_lines(input);
    let total = lines.len();
    let mut i = 0;

    while i < total {
        let line = &lines[i];
        let line_text = line.text;

        // Pragma check; inside a code-block directive the per-language
        // reflow path handles pragmas instead.
        if !in_code_block {
            if let Some(on) = super::check_pragma(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                pragma_off = !on;
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if pragma_off {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
        }

        // Inside an rst code-block directive body.
        // The body consists of lines indented past `code_indent`, plus
        // interior blank lines. The block ends at a non-blank line whose
        // indent drops below `code_indent`.
        if in_code_block {
            let leading = line_text.len() - line_text.trim_start().len();
            if line_text.trim().is_empty() {
                // Could be interior blank or end-of-block; buffer and look ahead.
                if code_footer_start.is_none() {
                    code_footer_start = Some(line.start);
                }
                i += 1;
                continue;
            }
            if leading >= code_indent {
                // A later body line claims any buffered interior blanks.
                code_footer_start = None;
                code_body_end = line.end;
                i += 1;
                continue;
            }
            // Less-indented non-blank line: close the code block.
            in_code_block = false;
            let footer = match code_footer_start.take() {
                Some(fs) => ByteSpan::new(fs, line.start),
                None => ByteSpan::new(line.start, line.start),
            };
            let body_end = if code_body_end > code_body_start {
                code_body_end
            } else {
                footer.start
            };
            regions.push(SpannedRegion::code(
                input,
                code_lang.take(),
                code_header,
                ByteSpan::new(code_body_start, body_end),
                footer,
            ));
            // Fall through to reprocess this line as normal.
        }

        // Inside literal block
        if in_literal_block {
            let leading = line_text.len() - line_text.trim_start().len();
            if line_text.trim().is_empty() || leading >= literal_indent {
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
            in_literal_block = false;
        }

        // Inside directive body
        if in_directive {
            let leading = line_text.len() - line_text.trim_start().len();
            if line_text.trim().is_empty() || leading >= directive_indent {
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
            in_directive = false;
        }

        // Inside comment body (indented lines after `..` / `.. text`).
        if in_comment {
            let leading = line_text.len() - line_text.trim_start().len();
            if line_text.trim().is_empty() || leading >= comment_indent {
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
            in_comment = false;
        }

        // Blank line
        if line_text.trim().is_empty() {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::blank(input, line.span()));
            i += 1;
            continue;
        }

        // Inside a definition-list body (indented lines after a flush term).
        // Blanks already fell through above so they stay BlankLines.
        if in_definition {
            let leading = line_text.len() - line_text.trim_start().len();
            if leading >= definition_indent {
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
            in_definition = false;
        }

        // RST code-block directive (.. code-block:: LANG)
        if let Some(caps) = CODE_DIRECTIVE_RE.captures(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            code_lang = caps.get(1).map(|m| m.as_str().to_string());
            code_header = line.span();
            // Docutils accepts a two-space body; +3 is convention only.
            let leading = line_text.len() - line_text.trim_start().len();
            code_indent = leading + 2;
            code_body_start = line.end;
            code_body_end = line.end;
            code_footer_start = None;
            in_code_block = true;
            i += 1;
            continue;
        }

        // RST directive (.. something::)
        let trimmed = line_text.trim_start();
        if trimmed.starts_with(".. ") && trimmed.contains("::") {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            let leading = line_text.len() - trimmed.len();
            // Docutils accepts a two-space body; +3 is convention only.
            directive_indent = leading + 2;
            in_directive = true;
            i += 1;
            continue;
        }

        // RST comment (`..` or `.. text` without `::`). Bare `..` is a
        // comment opener; following indented lines are the comment body.
        if is_rst_comment_opener(trimmed) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            let leading = line_text.len() - trimmed.len();
            // Any indent past the opener is body (docutils).
            comment_indent = leading + 1;
            in_comment = true;
            i += 1;
            continue;
        }

        // Section underline
        if is_underline(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            i += 1;
            continue;
        }

        // Section title (next line is underline)
        if i + 1 < total && is_underline(lines[i + 1].text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            i += 1;
            continue;
        }

        // Field list (:field: value)
        if trimmed.starts_with(':') && trimmed.len() > 2 {
            if let Some(colon_pos) = trimmed[1..].find(':') {
                if colon_pos > 0 && colon_pos < trimmed.len() - 2 {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
            }
        }

        // Literal block intro (line ending with ::)
        if trimmed.ends_with("::") {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            // Find indent of next non-blank line
            let mut j = i + 1;
            while j < total && lines[j].text.trim().is_empty() {
                j += 1;
            }
            if j < total {
                let next = lines[j].text;
                let next_indent = next.len() - next.trim_start().len();
                if next_indent > 0 {
                    literal_indent = next_indent;
                    in_literal_block = true;
                }
            }
            i += 1;
            continue;
        }

        // List item: marker is Structure so `1. First` does not split
        // after `1.`, and each item is its own region so adjacent
        // bullets are not glued onto one line.
        if let Some(marker_len) = rst_list_marker_len(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            list_hang = Some(marker_len);
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(line.start, line.start + marker_len),
            ));
            if line_text.len() > marker_len {
                current_prose.push_str(line_text[marker_len..].trim());
                prose_span = Some(ByteSpan::new(line.start + marker_len, line.end));
            }
            i += 1;
            continue;
        }

        // Grid table rows (`| cell |` / `+---+---+`)
        if trimmed.starts_with('|') || trimmed.starts_with('+') {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            i += 1;
            continue;
        }

        // Simple table: `=====  =====` borders plus the rows they wrap.
        // `is_underline` rejects those borders (interior spaces), so without
        // this the header/body rows fall through to prose and get joined.
        if is_simple_table_border(line_text) {
            if let Some(end) = simple_table_end(&lines, i) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                for row in &lines[i..=end] {
                    regions.push(SpannedRegion::structure(input, row.span()));
                }
                i = end + 1;
                continue;
            }
        }

        // Definition list: flush term plus an immediately indented definition.
        // Without this both lines are Prose and splice joins them.
        if is_definition_term(&lines, i) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            let next = lines[i + 1].text;
            definition_indent = next.len() - next.trim_start().len();
            in_definition = true;
            i += 1;
            continue;
        }

        // List continuation: after a list item, a later line indented
        // to the hang (two spaces after `- `) is still the item.
        // A blank before it is a new paragraph: hang spaces stay
        // Structure so splice does not outdent them. Compact wrap
        // (no blank) is the same paragraph; join into the open Prose
        // so a reflow hang reparses as the source list item.
        if let Some(hang) = list_hang {
            let leading = line_text.len() - line_text.trim_start().len();
            if leading >= hang {
                let after_blank = regions
                    .last()
                    .is_some_and(|r| matches!(r.region, Region::BlankLines(_)));
                if after_blank {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(
                        input,
                        ByteSpan::new(line.start, line.start + leading),
                    ));
                    if line_text.len() > leading {
                        current_prose.push_str(line_text[leading..].trim());
                        prose_span = Some(ByteSpan::new(line.start + leading, line.end));
                    }
                } else if line_text.len() > leading {
                    if !current_prose.is_empty() {
                        current_prose.push(' ');
                    }
                    current_prose.push_str(line_text[leading..].trim());
                    match prose_span.as_mut() {
                        Some(span) => span.end = line.end,
                        None => {
                            prose_span = Some(ByteSpan::new(line.start + leading, line.end));
                        }
                    }
                }
                i += 1;
                continue;
            }
            list_hang = None;
        }

        // Block quote: indented prose that is not a list, directive,
        // comment, definition, or literal. Hang spaces stay Structure
        // so splice does not outdent them (GitHub #58). Compact wrap
        // (no blank) joins via list_hang so SemBr still applies.
        let leading = line_text.len() - line_text.trim_start().len();
        if leading > 0 {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            list_hang = Some(leading);
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(line.start, line.start + leading),
            ));
            if line_text.len() > leading {
                current_prose.push_str(line_text[leading..].trim());
                prose_span = Some(ByteSpan::new(line.start + leading, line.end));
            }
            i += 1;
            continue;
        }

        // Regular prose
        push_prose_line(&mut current_prose, &mut prose_span, line, true, true);
        i += 1;
    }

    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
    if in_code_block {
        let footer = match code_footer_start {
            Some(fs) => ByteSpan::new(fs, input.len()),
            None => ByteSpan::new(input.len(), input.len()),
        };
        let body_end = if code_body_end > code_body_start {
            code_body_end
        } else {
            footer.start
        };
        regions.push(SpannedRegion::code(
            input,
            code_lang.take(),
            code_header,
            ByteSpan::new(code_body_start, body_end),
            footer,
        ));
    }
    regions
}

/// True when `trimmed` is an RST comment opener, not a `.. name::` directive.
/// Bare `..` (no trailing space) is a valid opener; so is `.. text`.
pub(crate) fn is_rst_comment_opener(trimmed: &str) -> bool {
    if trimmed.contains("::") {
        return false;
    }
    trimmed == ".." || trimmed.starts_with(".. ") || trimmed.starts_with("..\t")
}

/// Comment openers pandoc's RST reader drops (zero blocks). Hyperlink
/// targets (`.. _name:`) and footnotes (`.. [1]`) start with `..` but
/// survive the reader, so they are not a drop.
pub(crate) fn is_rst_dropped_comment_opener(trimmed: &str) -> bool {
    if !is_rst_comment_opener(trimmed) {
        return false;
    }
    let after = trimmed.strip_prefix("..").unwrap_or("").trim_start();
    if after.is_empty() {
        return true;
    }
    !after.starts_with('_') && !after.starts_with('[')
}

/// Source has an RST `..` comment that `pandoc -f rst` would delete.
pub(crate) fn source_has_dropped_rst_comments(input: &str) -> bool {
    input
        .lines()
        .any(|line| is_rst_dropped_comment_opener(line.trim_start()))
}

/// Byte length of a compact RST list opener on `line`, including the
/// trailing space: `* `, `- `, `+ ` (not a `+--+` table rule), `#. `,
/// or `1.` / `1)`.
fn rst_list_marker_len(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    let t = &line[indent..];
    let rest = if t.starts_with("* ") || t.starts_with("- ") {
        2
    } else if t.starts_with("#. ") {
        3
    } else if let Some(after) = t.strip_prefix("+ ") {
        if after.starts_with('-') || after.starts_with('+') {
            return None;
        }
        2
    } else {
        let bytes = t.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 || !matches!(bytes.get(i), Some(b'.') | Some(b')')) {
            return None;
        }
        if matches!(bytes.get(i + 1), Some(b' ')) {
            i + 2
        } else if i + 1 == t.len() {
            i + 1
        } else {
            return None;
        }
    };
    Some(indent + rest)
}

/// Check if a line is a section underline (2+ repeated punctuation chars).
/// Includes `' . _ < >` in addition to the common `= - ~ ^ " # * +` set.
fn is_underline(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 2 {
        return false;
    }
    let first = trimmed.as_bytes()[0];
    matches!(
        first,
        b'=' | b'-' | b'~' | b'^' | b'"' | b'#' | b'*' | b'+' | b'\'' | b'.' | b'_' | b'<' | b'>'
    ) && trimmed.bytes().all(|b| b == first)
}

/// RST simple-table border: `=` column groups separated by spaces
/// (`=====  =====`). A solid `=====` is a section underline, not a table.
fn is_simple_table_border(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 3 {
        return false;
    }
    let mut groups = 0u32;
    let mut in_eq = false;
    let mut saw_space_between = false;
    for b in t.bytes() {
        match b {
            b'=' => {
                if !in_eq {
                    groups += 1;
                    in_eq = true;
                }
            }
            b' ' => {
                if in_eq {
                    saw_space_between = true;
                }
                in_eq = false;
            }
            _ => return false,
        }
    }
    saw_space_between && groups >= 2
}

/// True when `lines[i]` is a definition-list term: the next physical line
/// is non-blank and indented further than this one. RST forbids a blank
/// between term and definition.
fn is_definition_term(lines: &[Line<'_>], i: usize) -> bool {
    let next = match lines.get(i + 1) {
        Some(line) => line.text,
        None => return false,
    };
    if next.trim().is_empty() {
        return false;
    }
    let line = lines[i].text;
    let indent = line.len() - line.trim_start().len();
    let next_indent = next.len() - next.trim_start().len();
    next_indent > indent
}

/// Last line of a simple table starting at `start`, if a later `=` border
/// closes it before a blank line.
fn simple_table_end(lines: &[Line<'_>], start: usize) -> Option<usize> {
    let mut last_border = start;
    for (j, line) in lines.iter().enumerate().skip(start + 1) {
        if line.text.trim().is_empty() {
            break;
        }
        if is_simple_table_border(line.text) {
            last_border = j;
        }
    }
    (last_border > start).then_some(last_border)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Region;

    #[test]
    fn simple_prose() {
        let input = "Hello world. This is a test.\nAnother line here.";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Hello world.")))
        );
    }

    #[test]
    fn directive_preserved() {
        let input = "Some prose.\n\n.. code-block:: python\n\n   print('hello')\n\nMore prose.";
        let regions = RstParser.parse(input);
        let prose_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Prose(_)))
            .count();
        assert_eq!(prose_count, 2);
        // The code block surfaces as Region::Code with lang=python.
        let code = regions.iter().find_map(|r| match r {
            Region::Code { lang, body, .. } => Some((lang.clone(), body.clone())),
            _ => None,
        });
        let (lang, body) = code.expect("expected one Region::Code");
        assert_eq!(lang.as_deref(), Some("python"));
        assert!(body.contains("print('hello')"));
    }

    #[test]
    fn section_title_preserved() {
        let input = "My Title\n========\n\nSome text here.";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("My Title")))
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("====")))
        );
    }

    #[test]
    fn apostrophe_section_adornment_is_structure() {
        let input = "Input\n'''''\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Input"))),
            "title must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("'''''"))),
            "apostrophe underline must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(
                |r| matches!(r, Region::Prose(s) if s.contains("Input") || s.contains("'''''"))
            ),
            "apostrophe section must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_section_adornments_are_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        for (adornment, n) in [('\'', 5), ('\'', 2), ('.', 5), ('_', 5), ('<', 5), ('>', 5)] {
            let rule = adornment.to_string().repeat(n);
            let input = format!("Input\n{rule}\n");
            let out = format_text(&input, &cfg).unwrap();
            assert_eq!(
                out, input,
                "section adornment {adornment:?} x{n} must stay two lines, got:\n{out}"
            );
            assert!(
                !out.contains(&format!("Input {rule}")),
                "must not glue {adornment:?} adornment onto title, got:\n{out}"
            );
        }
    }

    #[test]
    fn literal_block_preserved() {
        let input = "Example::\n\n   some code\n   more code\n\nBack to prose.";
        let regions = RstParser.parse(input);
        let structure_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Structure(_)))
            .count();
        assert!(structure_count >= 3);
    }

    #[test]
    fn field_list_preserved() {
        let input = ":Author: Someone\n:Date: 2026\n\nParagraph text.";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Author")))
        );
    }

    #[test]
    fn adjacent_bullet_items_stay_separate_regions() {
        let input = "Features:\n\n* First item\n* Second item\n* Third item\n";
        let regions = RstParser.parse(input);
        let bullets: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) if s.contains("item") => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            bullets.len(),
            3,
            "each bullet must be its own Prose region, got {regions:?}"
        );
    }

    #[test]
    fn simple_table_rows_are_structure() {
        let input = "=====  =====\nName   Value\n=====  =====\nA      B\n=====  =====\n";
        let regions = RstParser.parse(input);
        let structure: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Structure(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            structure.iter().any(|s| s.contains("Name")),
            "header row must be Structure, got {regions:?}"
        );
        assert!(
            structure.iter().any(|s| s.contains("A")),
            "body row must be Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Name") || s.contains("A"))),
            "table rows must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_simple_table_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "=====  =====\nName   Value\n=====  =====\nA      B\n=====  =====\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input, "simple table must stay identity, got:\n{out}");
    }

    #[test]
    fn definition_list_term_and_body_are_structure() {
        let input = "Term\n   Definition sentence.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Term"))),
            "term must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Definition"))),
            "definition must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(r, Region::Prose(s) if s.contains("Term") || s.contains("Definition"))
            }),
            "definition list must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_definition_list_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "Term\n   Definition sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "definition list must stay identity, got:\n{out}"
        );
        assert!(
            out.starts_with("Term\n"),
            "term must stay at column 0, got:\n{out}"
        );
        assert!(
            out.contains("\n   Definition sentence."),
            "definition must keep indent, got:\n{out}"
        );
    }

    #[test]
    fn adjacent_rst_lists_are_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        for input in [
            "Features:\n\n* First item\n* Second item\n* Third item\n",
            "Features:\n\n- First item\n- Second item\n",
            "Features:\n\n+ First item\n+ Second item\n",
            "Features:\n\n1. First item\n2. Second item\n",
            "Features:\n\n#. First item\n#. Second item\n",
        ] {
            let out = format_text(input, &cfg).unwrap();
            assert_eq!(out, input, "list must stay compact, got:\n{out}");
        }
    }

    #[test]
    fn list_continuation_paragraph_keeps_hang_structure() {
        let input = "- First sentence.\n\n  Second sentence.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "- ")),
            "marker must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| { matches!(r, Region::Prose(s) if s.contains("First sentence.")) }),
            "item text must be Prose, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "continuation hang must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| { matches!(r, Region::Prose(s) if s.contains("Second sentence.")) }),
            "continuation text must be Prose, got {regions:?}"
        );
    }

    #[test]
    fn compact_list_hang_joins_into_one_prose_region() {
        let input = "* One.\n  Two.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "* ")),
            "marker must be Structure, got {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            ["One. Two."],
            "compact hang must join into one Prose, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "compact hang must not be Structure, got {regions:?}"
        );
    }

    #[test]
    fn starred_quote_list_item_is_idempotent_and_oracle() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = "* a*'*'. A.";
        let out = format_text(input, &cfg).expect("format_text");
        let twice = format_text(&out, &cfg).expect("second pass");
        assert_eq!(
            out, twice,
            "not idempotent:\n first={out:?}\n second={twice:?}"
        );
        assert_eq!(
            out, "* a*'*'.\n  A.",
            "list wrap must hang at marker width, got:\n{out}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn reporter_list_continuation_paragraph_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "- First sentence.\n\n  Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "list continuation paragraph must stay identity, got:\n{out}"
        );
        assert!(
            out.contains("\n  Second sentence."),
            "continuation must keep two-space hang, got:\n{out}"
        );
    }

    #[test]
    fn reporter_enumerated_item_second_sentence_hangs_at_marker_width() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "#. First sentence. Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, "#. First sentence.\n   Second sentence.\n",
            "second sentence must hang at `#. ` width, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung enumerated item must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );

        // Bullet hang from #51 stays.
        let bullet = "* First sentence. Second sentence.\n";
        let bullet_out = format_text(bullet, &cfg).unwrap();
        assert_eq!(
            bullet_out, "* First sentence.\n  Second sentence.\n",
            "bullet hang must stay, got:\n{bullet_out}"
        );
        let bullet_twice = format_text(&bullet_out, &cfg).unwrap();
        assert_eq!(
            bullet_out, bullet_twice,
            "hung bullet must be identity, got:\n{bullet_twice}"
        );
        let blank_hang = "- First sentence.\n\n  Second sentence.\n";
        let blank_out = format_text(blank_hang, &cfg).unwrap();
        assert_eq!(
            blank_out, blank_hang,
            "list continuation hang from #51 must stay, got:\n{blank_out}"
        );
    }

    #[test]
    fn comment_opener_and_body_are_structure() {
        let input = "..\n   First sentence.\n   Second sentence.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "..")),
            "bare .. must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("First sentence."))),
            "comment body must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Second sentence."))),
            "comment body must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(
                    r,
                    Region::Prose(s)
                        if s.contains("First sentence.") || s.contains("Second sentence.")
                )
            }),
            "comment body must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_comment_body_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "..\n   First sentence.\n   Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "reporter comment body must stay identity under format, got:\n{out}"
        );
        assert!(
            out.contains("\n   First sentence."),
            "first comment line must keep indent, got:\n{out}"
        );
        assert!(
            out.contains("\n   Second sentence."),
            "second comment line must keep indent, got:\n{out}"
        );
    }

    #[test]
    fn bare_dotdot_is_comment_opener() {
        assert!(is_rst_comment_opener(".."));
        assert!(is_rst_comment_opener(".. This is a comment."));
        assert!(is_rst_comment_opener("..\tThis is a comment."));
        assert!(!is_rst_comment_opener(".. note::"));
        assert!(!is_rst_comment_opener("..."));
        assert!(!is_rst_comment_opener("Hello"));
    }

    #[test]
    fn dropped_comment_excludes_targets_and_footnotes() {
        assert!(is_rst_dropped_comment_opener(".."));
        assert!(is_rst_dropped_comment_opener(".. This is a comment."));
        assert!(source_has_dropped_rst_comments(
            "..\n   First sentence.\n   Second sentence.\n"
        ));
        assert!(!is_rst_dropped_comment_opener(".. _label:"));
        assert!(!is_rst_dropped_comment_opener(".. [1]"));
        assert!(!is_rst_dropped_comment_opener(".. note::"));
        assert!(!source_has_dropped_rst_comments(
            "Hello world. Second sentence.\n\n.. _label:\n\n.. note::\n   Body.\n"
        ));
    }

    #[test]
    fn recognized_comment_line_keeps_indented_body() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = ".. This is a comment.\n   First sentence.\n   Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "recognized comment line must keep indented body identity, got:\n{out}"
        );
        assert!(
            out.contains("\n   First sentence."),
            "first comment body line must keep indent, got:\n{out}"
        );
        assert!(
            out.contains("\n   Second sentence."),
            "second comment body line must keep indent, got:\n{out}"
        );
    }

    #[test]
    fn two_space_directive_option_and_body_are_structure() {
        let input = ".. note::\n  :class: test\n\n  Body.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(".. note::"))),
            "directive opener must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(":class: test"))),
            "two-space option must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Body."))),
            "two-space body must be Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Body."))),
            "two-space body must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_two_space_directive_body_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = ".. note::\n  :class: test\n\n  Body.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "reporter two-space directive body must stay identity under format, got:\n{out}"
        );
        assert!(
            out.contains("\n  :class: test"),
            "option must keep two-space indent, got:\n{out}"
        );
        assert!(
            out.contains("\n  Body."),
            "body must keep two-space indent, got:\n{out}"
        );
    }

    #[test]
    fn two_space_code_block_body_is_code() {
        let input = ".. code-block:: python\n\n  print(\"hello\")\n";
        let regions = RstParser.parse(input);
        let code = regions.iter().find_map(|r| match r {
            Region::Code { lang, body, .. } => Some((lang.clone(), body.clone())),
            _ => None,
        });
        let (lang, body) = code.expect("expected one Region::Code");
        assert_eq!(lang.as_deref(), Some("python"));
        assert!(
            body.contains("print(\"hello\")"),
            "two-space body must stay in the code region, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("print"))),
            "two-space code-block body must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_two_space_code_block_body_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = ".. code-block:: python\n\n  print(\"hello\")\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "reporter two-space code-block body must stay identity under format, got:\n{out}"
        );
        assert!(
            out.contains("\n  print(\"hello\")"),
            "body must keep two-space indent, got:\n{out}"
        );
    }

    #[test]
    fn block_quote_hang_is_structure() {
        let input = "Before.\n\n   First sentence.\n   Second sentence.\n\nAfter.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
            "quote hang must be Structure, got {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            prose.iter().any(|s| s.contains("Before.")),
            "surrounding paragraph must stay Prose, got {regions:?}"
        );
        assert!(
            prose.iter().any(|s| s.contains("After.")),
            "trailing paragraph must stay Prose, got {regions:?}"
        );
        assert!(
            prose
                .iter()
                .any(|s| s.contains("First sentence.") && s.contains("Second sentence.")),
            "compact quote lines must join into one Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_block_quote_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "Before.\n\n   First sentence.\n   Second sentence.\n\nAfter.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "reporter block quote must stay identity under format, got:\n{out}"
        );
        assert!(
            out.contains("\n   First sentence."),
            "first quote line must keep three-space indent, got:\n{out}"
        );
        assert!(
            out.contains("\n   Second sentence."),
            "second quote line must keep three-space indent, got:\n{out}"
        );
    }

    #[test]
    fn surrounding_unquoted_paragraphs_still_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            "Before. More before.\n\n",
            "   First sentence.\n   Second sentence.\n\n",
            "After. More after.\n"
        );
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Before.\nMore before."),
            "leading unquoted paragraph must still reflow, got:\n{out}"
        );
        assert!(
            out.contains("\n   First sentence.\n   Second sentence.\n"),
            "quoted sentences must keep indent, got:\n{out}"
        );
        assert!(
            out.contains("After.\nMore after."),
            "trailing unquoted paragraph must still reflow, got:\n{out}"
        );
    }

    #[test]
    fn compact_block_quote_reflows_with_hang() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "Before.\n\n   First sentence. Second sentence.\n\nAfter.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, "Before.\n\n   First sentence.\n   Second sentence.\n\nAfter.\n",
            "quoted sentences must reflow with hang, got:\n{out}"
        );
    }

    #[test]
    fn rst_role_closer_keeps_two_sentence_lines() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "The task is in :file:`README.md`.\nUse this section for questions.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input, "role closer must stay two lines, got:\n{out}");
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let literal = "The task is in ``README.md``.\nUse this section for questions.\n";
        let out = format_text(literal, &cfg).unwrap();
        assert_eq!(
            out, literal,
            "double-backtick literal must stay split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }
}
