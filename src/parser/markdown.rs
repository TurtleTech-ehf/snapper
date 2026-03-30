use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{FormatParser, Region, flush_prose};

static HEADING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6}\s+)(.*)$").unwrap());

static FENCED_CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(`{3,}|~{3,})").unwrap());

static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*(?:[-*+]|\d+[.)]) )(.*)$").unwrap());

pub struct MarkdownParser;

impl FormatParser for MarkdownParser {
    fn parse(&self, input: &str) -> Vec<Region> {
        let mut regions: Vec<Region> = Vec::new();
        let mut current_prose = String::new();
        let mut in_fenced_code = false;
        let mut fence_marker = String::new();
        let mut in_frontmatter = false;
        let mut frontmatter_fence = String::new();
        let mut line_number = 0;
        let mut pragma_off = false;

        for line in input.lines() {
            line_number += 1;

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

            // Front matter detection (only at start of file)
            if line_number == 1 && (line.trim() == "---" || line.trim() == "+++") {
                in_frontmatter = true;
                frontmatter_fence = line.trim().to_string();
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            if in_frontmatter {
                if line.trim() == frontmatter_fence {
                    in_frontmatter = false;
                }
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Inside fenced code block
            if in_fenced_code {
                flush_prose(&mut current_prose, &mut regions);
                if let Some(caps) = FENCED_CODE_RE.captures(line.trim_start()) {
                    let marker = caps.get(1).unwrap().as_str();
                    if marker.chars().next() == fence_marker.chars().next()
                        && marker.len() >= fence_marker.len()
                    {
                        in_fenced_code = false;
                    }
                }
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Fenced code block start
            if let Some(caps) = FENCED_CODE_RE.captures(line.trim_start()) {
                flush_prose(&mut current_prose, &mut regions);
                fence_marker = caps.get(1).unwrap().as_str().to_string();
                in_fenced_code = true;
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Blank line
            if line.trim().is_empty() {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::BlankLines(format!("{line}\n")));
                continue;
            }

            // Heading
            if let Some(caps) = HEADING_RE.captures(line) {
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

            // List item
            if let Some(caps) = LIST_ITEM_RE.captures(line) {
                flush_prose(&mut current_prose, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let text = caps.get(2).unwrap().as_str();
                regions.push(Region::Structure(marker.to_string()));
                if !text.is_empty() {
                    regions.push(Region::Prose(text.to_string()));
                }
                regions.push(Region::Structure("\n".to_string()));
                continue;
            }

            // Regular prose
            if !current_prose.is_empty() {
                current_prose.push(' ');
            }
            current_prose.push_str(line.trim());
        }

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
        assert!(matches!(&regions[1], Region::Structure(_))); // ```python
        assert!(matches!(&regions[2], Region::Structure(_))); // code
        assert!(matches!(&regions[3], Region::Structure(_))); // ```
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
    fn heading_split() {
        let input = "## My Heading";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions.len(), 3);
        assert_eq!(regions[0], Region::Structure("## ".to_string()));
        assert_eq!(regions[1], Region::Prose("My Heading".to_string()));
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
    }
}
