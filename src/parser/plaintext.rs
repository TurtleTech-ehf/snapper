use crate::parser::{
    ByteSpan, FormatParser, SpannedRegion, flush_prose_spanned, iter_lines, push_prose_line,
};

/// Trivial parser: everything is prose, blank lines are preserved.
pub struct PlaintextParser;

impl FormatParser for PlaintextParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let mut regions = Vec::new();
        let mut current_prose = String::new();
        let mut prose_span: Option<ByteSpan> = None;
        let mut pragma_off = false;

        for line in iter_lines(input) {
            // Check for snapper:off/on pragmas
            if let Some(on) = super::check_pragma(line.text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                pragma_off = !on;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            if pragma_off {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            if line.text.trim().is_empty() {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::blank(input, line.span()));
            } else {
                push_prose_line(&mut current_prose, &mut prose_span, &line, true, true);
            }
        }

        flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Region;

    #[test]
    fn simple_paragraph() {
        let input = "Hello world. This is a test.\nAnother line here.";
        let regions = PlaintextParser.parse(input);
        assert_eq!(
            regions,
            vec![Region::Prose(
                "Hello world. This is a test. Another line here.".to_string()
            )]
        );
    }

    #[test]
    fn two_paragraphs() {
        let input = "First paragraph.\n\nSecond paragraph.";
        let regions = PlaintextParser.parse(input);
        assert_eq!(
            regions,
            vec![
                Region::Prose("First paragraph.".to_string()),
                Region::BlankLines("\n".to_string()),
                Region::Prose("Second paragraph.".to_string()),
            ]
        );
    }

    #[test]
    fn empty_input() {
        let regions = PlaintextParser.parse("");
        assert!(regions.is_empty());
    }
}
