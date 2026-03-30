use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{FormatParser, Region, flush_prose};

static HEADLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\*+\s+(?:TODO\s+|DONE\s+|NEXT\s+|WAIT\s+)?)(.*)$").unwrap());

static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*(?:[-+]|\d+[.)]) )(.*)$").unwrap());

pub struct OrgParser;

impl OrgParser {
    /// Check if a line starts a block (#+BEGIN_...)
    fn is_block_begin(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.to_ascii_uppercase().starts_with("#+BEGIN_")
    }

    /// Check if a line ends a block (#+END_...)
    fn is_block_end(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.to_ascii_uppercase().starts_with("#+END_")
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
}

impl FormatParser for OrgParser {
    fn parse(&self, input: &str) -> Vec<Region> {
        let mut regions: Vec<Region> = Vec::new();
        let mut current_prose = String::new();
        let mut in_block = false;
        let mut in_drawer = false;
        let mut pragma_off = false;
        // Track list item context: indent level of the marker text.
        // Continuation lines indented at or beyond this level belong to the item.
        let mut list_item_indent: Option<usize> = None;

        for line in input.lines() {
            // Check for snapper:off/on pragmas
            if let Some(on) = super::check_pragma(line) {
                flush_prose(&mut current_prose, &mut regions);
                pragma_off = !on;
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Inside pragma-off region: pass through unchanged
            if pragma_off {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Inside a block -- everything is structure
            if in_block {
                flush_prose(&mut current_prose, &mut regions);
                if Self::is_block_end(line) {
                    in_block = false;
                }
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Inside a drawer -- everything is structure
            if in_drawer {
                flush_prose(&mut current_prose, &mut regions);
                if Self::is_drawer_end(line) {
                    in_drawer = false;
                }
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Block begin
            if Self::is_block_begin(line) {
                flush_prose(&mut current_prose, &mut regions);
                in_block = true;
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Drawer begin
            if Self::is_drawer_begin(line) {
                flush_prose(&mut current_prose, &mut regions);
                in_drawer = true;
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Blank line
            if line.trim().is_empty() {
                flush_prose(&mut current_prose, &mut regions);
                list_item_indent = None;
                regions.push(Region::BlankLines(format!("{line}\n")));
                continue;
            }

            // Keyword/directive
            if Self::is_keyword(line) {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Comment
            if Self::is_comment(line) {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Table row
            if Self::is_table_row(line) {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Bare file/http links on their own line -- treat as structure
            if line.trim_start().starts_with("file:")
                || line.trim_start().starts_with("http://")
                || line.trim_start().starts_with("https://")
            {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Headline: stars + optional keyword are structure, rest is prose
            if let Some(caps) = HEADLINE_RE.captures(line) {
                flush_prose(&mut current_prose, &mut regions);
                let prefix = caps.get(1).unwrap().as_str();
                let text = caps.get(2).unwrap().as_str();
                regions.push(Region::Structure(prefix.to_string()));
                if !text.is_empty() {
                    regions.push(Region::Prose(text.to_string()));
                }
                regions.push(Region::Structure("\n".to_string()));
                continue;
            }

            // List item: marker is structure, rest is prose
            if let Some(caps) = LIST_ITEM_RE.captures(line) {
                flush_prose(&mut current_prose, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let text = caps.get(2).unwrap().as_str();
                // Track indent for continuation detection: text starts at marker length
                list_item_indent = Some(marker.len());
                regions.push(Region::Structure(marker.to_string()));
                if !text.is_empty() {
                    regions.push(Region::Prose(text.to_string()));
                }
                regions.push(Region::Structure("\n".to_string()));
                continue;
            }

            // List item continuation: indented line following a list item
            if let Some(indent) = list_item_indent {
                let leading = line.len() - line.trim_start().len();
                if leading >= indent && !line.trim().is_empty() {
                    // Append to the previous Prose region of the list item.
                    // The last three regions are Structure(marker), Prose(text), Structure(\n)
                    // We want to extend the Prose region.
                    if let Some(Region::Structure(s)) = regions.last() {
                        if s == "\n" {
                            regions.pop(); // remove the \n
                            if let Some(Region::Prose(prose)) = regions.last_mut() {
                                prose.push(' ');
                                prose.push_str(line.trim());
                            }
                            regions.push(Region::Structure("\n".to_string()));
                            continue;
                        }
                    }
                }
                // Not a continuation: leave list context
                list_item_indent = None;
            }

            // Regular prose line -- accumulate
            if !current_prose.is_empty() {
                current_prose.push(' ');
            }
            current_prose.push_str(line.trim());
        }

        // Flush remaining
        flush_prose(&mut current_prose, &mut regions);

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
        assert_eq!(regions.len(), 5);
        assert!(matches!(&regions[0], Region::Prose(_)));
        assert!(matches!(&regions[1], Region::Structure(_))); // BEGIN
        assert!(matches!(&regions[2], Region::Structure(_))); // code
        assert!(matches!(&regions[3], Region::Structure(_))); // END
        assert!(matches!(&regions[4], Region::Prose(_)));
    }

    #[test]
    fn preserves_keywords() {
        let input = "#+TITLE: My Document\n#+AUTHOR: Someone\n\nSome text here.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Structure(_)));
        assert!(matches!(&regions[1], Region::Structure(_)));
    }

    #[test]
    fn headline_split() {
        let input = "* TODO This is a headline";
        let regions = OrgParser.parse(input);
        assert_eq!(regions.len(), 3);
        assert_eq!(regions[0], Region::Structure("* TODO ".to_string()));
        assert_eq!(regions[1], Region::Prose("This is a headline".to_string()));
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
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
        assert_eq!(regions.len(), 6);
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
}
