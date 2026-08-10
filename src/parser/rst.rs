use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, SpannedRegion, flush_prose_spanned, iter_lines, push_prose_line,
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
/// field lists, comments, and tables as structure regions.
fn parse_line_based(input: &str) -> Vec<SpannedRegion> {
    let mut regions = Vec::new();
    let mut current_prose = String::new();
    let mut prose_span: Option<ByteSpan> = None;
    let mut in_literal_block = false;
    let mut literal_indent: usize = 0;
    let mut in_directive = false;
    let mut directive_indent: usize = 0;
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

        // Blank line
        if line_text.trim().is_empty() {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::blank(input, line.span()));
            i += 1;
            continue;
        }

        // RST code-block directive (.. code-block:: LANG)
        if let Some(caps) = CODE_DIRECTIVE_RE.captures(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            code_lang = caps.get(1).map(|m| m.as_str().to_string());
            code_header = line.span();
            // Body indent: directive_indent + 3 spaces is the rst convention;
            // be liberal and accept any deeper indent of the first body line.
            let leading = line_text.len() - line_text.trim_start().len();
            code_indent = leading + 3;
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
            directive_indent = leading + 3;
            in_directive = true;
            i += 1;
            continue;
        }

        // RST comment (.. without directive)
        if trimmed.starts_with(".. ") && !trimmed.contains("::") {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
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

        // Grid/simple table rows
        if trimmed.starts_with('|') || trimmed.starts_with('+') {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
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

/// Check if a line is a section underline (2+ repeated punctuation chars).
fn is_underline(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 2 {
        return false;
    }
    let first = trimmed.as_bytes()[0];
    matches!(first, b'=' | b'-' | b'~' | b'^' | b'"' | b'#' | b'*' | b'+')
        && trimmed.bytes().all(|b| b == first)
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
}
