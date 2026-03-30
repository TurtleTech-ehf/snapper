use crate::parser::{FormatParser, Region, flush_prose};

pub struct RstParser;

impl FormatParser for RstParser {
    fn parse(&self, input: &str) -> Vec<Region> {
        parse_line_based(input)
    }
}

/// Line-based RST parser. Handles directives, literal blocks, sections,
/// field lists, comments, and tables as structure regions.
fn parse_line_based(input: &str) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut current_prose = String::new();
    let mut in_literal_block = false;
    let mut literal_indent: usize = 0;
    let mut in_directive = false;
    let mut directive_indent: usize = 0;
    let mut pragma_off = false;

    let lines: Vec<&str> = input.lines().collect();
    let total = lines.len();
    let mut i = 0;

    while i < total {
        let line = lines[i];

        // Pragma check
        if let Some(on) = super::check_pragma(line) {
            flush_prose(&mut current_prose, &mut regions);
            pragma_off = !on;
            regions.push(Region::Structure(format!("{line}\n")));
            i += 1;
            continue;
        }

        if pragma_off {
            flush_prose(&mut current_prose, &mut regions);
            regions.push(Region::Structure(format!("{line}\n")));
            i += 1;
            continue;
        }

        // Inside literal block
        if in_literal_block {
            let leading = line.len() - line.trim_start().len();
            if line.trim().is_empty() || leading >= literal_indent {
                regions.push(Region::Structure(format!("{line}\n")));
                i += 1;
                continue;
            }
            in_literal_block = false;
        }

        // Inside directive body
        if in_directive {
            let leading = line.len() - line.trim_start().len();
            if line.trim().is_empty() || leading >= directive_indent {
                regions.push(Region::Structure(format!("{line}\n")));
                i += 1;
                continue;
            }
            in_directive = false;
        }

        // Blank line
        if line.trim().is_empty() {
            flush_prose(&mut current_prose, &mut regions);
            regions.push(Region::BlankLines(format!("{line}\n")));
            i += 1;
            continue;
        }

        // RST directive (.. something::)
        let trimmed = line.trim_start();
        if trimmed.starts_with(".. ") && trimmed.contains("::") {
            flush_prose(&mut current_prose, &mut regions);
            regions.push(Region::Structure(format!("{line}\n")));
            let leading = line.len() - trimmed.len();
            directive_indent = leading + 3;
            in_directive = true;
            i += 1;
            continue;
        }

        // RST comment (.. without directive)
        if trimmed.starts_with(".. ") && !trimmed.contains("::") {
            flush_prose(&mut current_prose, &mut regions);
            regions.push(Region::Structure(format!("{line}\n")));
            i += 1;
            continue;
        }

        // Section underline
        if is_underline(line) {
            flush_prose(&mut current_prose, &mut regions);
            regions.push(Region::Structure(format!("{line}\n")));
            i += 1;
            continue;
        }

        // Section title (next line is underline)
        if i + 1 < total && is_underline(lines[i + 1]) {
            flush_prose(&mut current_prose, &mut regions);
            regions.push(Region::Structure(format!("{line}\n")));
            i += 1;
            continue;
        }

        // Field list (:field: value)
        if trimmed.starts_with(':') && trimmed.len() > 2 {
            if let Some(colon_pos) = trimmed[1..].find(':') {
                if colon_pos > 0 && colon_pos < trimmed.len() - 2 {
                    flush_prose(&mut current_prose, &mut regions);
                    regions.push(Region::Structure(format!("{line}\n")));
                    i += 1;
                    continue;
                }
            }
        }

        // Literal block intro (line ending with ::)
        if trimmed.ends_with("::") {
            flush_prose(&mut current_prose, &mut regions);
            regions.push(Region::Structure(format!("{line}\n")));
            // Find indent of next non-blank line
            let mut j = i + 1;
            while j < total && lines[j].trim().is_empty() {
                j += 1;
            }
            if j < total {
                let next_indent = lines[j].len() - lines[j].trim_start().len();
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
            flush_prose(&mut current_prose, &mut regions);
            regions.push(Region::Structure(format!("{line}\n")));
            i += 1;
            continue;
        }

        // Regular prose
        if !current_prose.is_empty() {
            current_prose.push(' ');
        }
        current_prose.push_str(trimmed);
        i += 1;
    }

    flush_prose(&mut current_prose, &mut regions);
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
