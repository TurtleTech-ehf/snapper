use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{FormatParser, Region, flush_prose};

static HEADING_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6}\s+)(.*)$").unwrap());

static FENCED_CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(`{3,}|~{3,})").unwrap());

/// Capture the language token immediately after a fence marker.
/// `lang` is `[A-Za-z0-9_+.-]+`; anything past it (info string) is ignored.
static FENCED_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:`{3,}|~{3,})\s*([A-Za-z0-9_+.\-]+)").unwrap());

static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*(?:[-*+]|\d+[.)]) )(.*)$").unwrap());

/// Match a markdown table row: line whose trimmed form starts and ends with `|`.
/// Also matches separator rows like `|---|---|`.
static TABLE_ROW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\|.*\|\s*$").unwrap());

pub struct MarkdownParser;

/// Close an open list item: flush accumulated prose and emit the trailing newline.
fn close_list_item(in_list_item: &mut bool, current_prose: &mut String, regions: &mut Vec<Region>) {
    if *in_list_item {
        flush_prose(current_prose, regions);
        regions.push(Region::Structure("\n".to_string()));
        *in_list_item = false;
    }
}

impl FormatParser for MarkdownParser {
    fn parse(&self, input: &str) -> Vec<Region> {
        let mut regions: Vec<Region> = Vec::new();
        let mut current_prose = String::new();
        let mut in_fenced_code = false;
        let mut fence_marker = String::new();
        // Buffer for the running code block: header line, body lines, lang
        let mut code_header = String::new();
        let mut code_body = String::new();
        let mut code_lang: Option<String> = None;
        let mut in_frontmatter = false;
        let mut frontmatter_fence = String::new();
        let mut in_list_item = false;
        let mut line_number = 0;
        let mut pragma_off = false;

        for line in input.lines() {
            line_number += 1;

            // Check for snapper:off/on pragmas. Inside a fenced code block,
            // the markdown parser does NOT short-circuit on pragmas; the
            // code-block reflow handles them per-language (the markers
            // `#`, `//`, `--`, `;` are all valid pragma prefixes inside
            // their respective languages).
            if !in_fenced_code {
                if let Some(on) = super::check_pragma(line) {
                    close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
                    flush_prose(&mut current_prose, &mut regions);
                    pragma_off = !on;
                    regions.push(Region::Structure(format!("{line}\n")));
                    continue;
                }

                if pragma_off {
                    close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
                    flush_prose(&mut current_prose, &mut regions);
                    regions.push(Region::Structure(format!("{line}\n")));
                    continue;
                }
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
                close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
                flush_prose(&mut current_prose, &mut regions);
                let mut closed = false;
                if let Some(caps) = FENCED_CODE_RE.captures(line.trim_start()) {
                    let marker = caps.get(1).unwrap().as_str();
                    if marker.chars().next() == fence_marker.chars().next()
                        && marker.len() >= fence_marker.len()
                    {
                        closed = true;
                    }
                }
                if closed {
                    in_fenced_code = false;
                    regions.push(Region::Code {
                        lang: code_lang.take(),
                        header: std::mem::take(&mut code_header),
                        body: std::mem::take(&mut code_body),
                        footer: format!("{line}\n"),
                    });
                } else {
                    code_body.push_str(line);
                    code_body.push('\n');
                }
                continue;
            }

            // Fenced code block start
            if let Some(caps) = FENCED_CODE_RE.captures(line.trim_start()) {
                close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
                flush_prose(&mut current_prose, &mut regions);
                fence_marker = caps.get(1).unwrap().as_str().to_string();
                in_fenced_code = true;
                code_lang = FENCED_LANG_RE
                    .captures(line.trim_start())
                    .map(|c| c.get(1).unwrap().as_str().to_string());
                code_header = format!("{line}\n");
                code_body.clear();
                continue;
            }

            // Blank line
            if line.trim().is_empty() {
                close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::BlankLines(format!("{line}\n")));
                continue;
            }

            // Heading — keep the entire ATX line as Structure.
            // Splitting into Structure("### ") + Prose(title) let the sentence
            // reflow engine break titles after "1." or mid-phrase, producing
            // orphan headings like:
            //   ### 1.
            //   `cargo binstall` (preferred binary install)
            // CommonMark ATX headings are single-line; do not reflow them.
            if HEADING_RE.is_match(line) {
                close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Table row (pipe-delimited)
            if TABLE_ROW_RE.is_match(line) {
                close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // List item: emit marker as Structure, start accumulating text as prose.
            // Continuation lines are appended until a block boundary.
            if let Some(caps) = LIST_ITEM_RE.captures(line) {
                close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
                flush_prose(&mut current_prose, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let text = caps.get(2).unwrap().as_str();
                regions.push(Region::Structure(marker.to_string()));
                in_list_item = true;
                if !text.is_empty() {
                    current_prose.push_str(text);
                }
                continue;
            }

            // Regular prose (also serves as list-item continuation when in_list_item)
            if !current_prose.is_empty() {
                current_prose.push(' ');
            }
            current_prose.push_str(line.trim());
        }

        close_list_item(&mut in_list_item, &mut current_prose, &mut regions);
        flush_prose(&mut current_prose, &mut regions);
        // Unclosed fence at EOF: emit a code region with empty footer.
        if in_fenced_code {
            regions.push(Region::Code {
                lang: code_lang.take(),
                header: std::mem::take(&mut code_header),
                body: std::mem::take(&mut code_body),
                footer: String::new(),
            });
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
                assert!(s.ends_with("|\n"));
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
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions.len(), 3);
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
        assert_eq!(regions[5], Region::Structure("\n".to_string()));
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
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
    }

    #[test]
    fn heading_is_structure_not_prose() {
        let input = "## My Heading";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Region::Structure("## My Heading\n".to_string()));
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
                vec![Region::Structure(format!("{line}\n"))],
                "level {hashes}"
            );
        }
    }
}
