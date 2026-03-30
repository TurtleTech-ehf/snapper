use crate::parser::{FormatParser, Region, flush_prose};

/// Trivial parser: everything is prose, blank lines are preserved.
pub struct PlaintextParser;

impl FormatParser for PlaintextParser {
    fn parse(&self, input: &str) -> Vec<Region> {
        let mut regions = Vec::new();
        let mut current_prose = String::new();
        let mut pragma_off = false;

        for line in input.lines() {
            // Check for snapper:off/on pragmas
            if let Some(on) = super::check_pragma(line) {
                flush_prose(&mut current_prose, &mut regions);
                pragma_off = !on;
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            if pragma_off {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            if line.trim().is_empty() {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::BlankLines(format!("{line}\n")));
            } else {
                if !current_prose.is_empty() {
                    current_prose.push(' ');
                }
                current_prose.push_str(line.trim());
            }
        }

        flush_prose(&mut current_prose, &mut regions);
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
