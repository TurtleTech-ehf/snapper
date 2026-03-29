use std::path::Path;
use std::process::Command;

use pandoc_ast::{Block, Inline, Pandoc};

use crate::parser::{FormatParser, Region};

/// Parser that uses pandoc as a backend for universal format support.
/// Requires pandoc binary on PATH.
pub struct PandocParser {
    /// Pandoc input format (e.g. "latex", "markdown", "org", "rst", "typst")
    input_format: String,
}

impl PandocParser {
    pub fn new(format: &str) -> Self {
        Self {
            input_format: format.to_string(),
        }
    }

    /// Detect pandoc input format from file extension.
    pub fn format_for_path(path: &Path) -> Option<String> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("org") => Some("org".to_string()),
            Some("tex" | "latex" | "ltx") => Some("latex".to_string()),
            Some("md" | "markdown" | "mkd" | "mdx") => Some("markdown".to_string()),
            Some("rst" | "rest") => Some("rst".to_string()),
            Some("typ") => Some("typst".to_string()),
            Some("adoc" | "asciidoc") => Some("asciidoc".to_string()),
            Some("html" | "htm") => Some("html".to_string()),
            Some("docx") => Some("docx".to_string()),
            Some("txt") => Some("markdown".to_string()),
            _ => None,
        }
    }
}

/// Check if pandoc is available on PATH.
pub fn pandoc_available() -> bool {
    Command::new("pandoc")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

impl FormatParser for PandocParser {
    fn parse(&self, input: &str) -> Vec<Region> {
        // Run pandoc to get JSON AST
        let output = Command::new("pandoc")
            .args(["-f", &self.input_format, "-t", "json"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(input.as_bytes()).ok();
                }
                child.wait_with_output()
            });

        let output = match output {
            Ok(o) if o.status.success() => o,
            _ => {
                // Pandoc failed; treat everything as prose
                return vec![Region::Prose(input.to_string())];
            }
        };

        let json = match String::from_utf8(output.stdout) {
            Ok(j) => j,
            Err(_) => return vec![Region::Prose(input.to_string())],
        };

        // Deserialize pandoc AST
        let doc: Pandoc = match serde_json::from_str(&json) {
            Ok(d) => d,
            Err(_) => return vec![Region::Prose(input.to_string())],
        };

        // Walk the AST and extract regions
        let mut regions = Vec::new();
        for block in &doc.blocks {
            extract_block(block, &mut regions);
        }

        // Handle pragma regions -- pandoc strips comments, so pragmas
        // won't work through pandoc. This is a known limitation.

        regions
    }
}

fn extract_block(block: &Block, regions: &mut Vec<Region>) {
    match block {
        Block::Para(inlines) | Block::Plain(inlines) => {
            let text = extract_inlines(inlines);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                regions.push(Region::Prose(trimmed.to_string()));
            }
        }
        Block::Header(_, _, inlines) => {
            let text = extract_inlines(inlines);
            if !text.trim().is_empty() {
                regions.push(Region::Structure(format!("{}\n", text.trim())));
            }
        }
        Block::CodeBlock(_, code) => {
            regions.push(Region::Structure(code.clone()));
        }
        Block::RawBlock(_, raw) => {
            regions.push(Region::Structure(raw.clone()));
        }
        Block::BlockQuote(blocks) => {
            for b in blocks {
                extract_block(b, regions);
            }
        }
        Block::BulletList(items) => {
            for item in items {
                for b in item {
                    extract_block(b, regions);
                }
            }
        }
        Block::OrderedList(_, items) => {
            for item in items {
                for b in item {
                    extract_block(b, regions);
                }
            }
        }
        Block::DefinitionList(defs) => {
            for (term, definitions) in defs {
                let term_text = extract_inlines(term);
                if !term_text.trim().is_empty() {
                    regions.push(Region::Structure(format!("{}\n", term_text.trim())));
                }
                for def in definitions {
                    for b in def {
                        extract_block(b, regions);
                    }
                }
            }
        }
        Block::Table(..) => {
            // Tables pass through as structure
            regions.push(Region::Structure("[table]\n".to_string()));
        }
        Block::HorizontalRule => {
            regions.push(Region::Structure("---\n".to_string()));
        }
        Block::Div(_, blocks) => {
            for b in blocks {
                extract_block(b, regions);
            }
        }
        Block::Figure(_, _, blocks) => {
            for b in blocks {
                extract_block(b, regions);
            }
        }
        Block::Null => {}
        Block::LineBlock(lines) => {
            // Poetry / preformatted lines -- treat as structure
            for line in lines {
                let text = extract_inlines(line);
                regions.push(Region::Structure(format!("{}\n", text)));
            }
        }
    }
}

fn extract_inlines(inlines: &[Inline]) -> String {
    let mut result = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(s) => result.push_str(s),
            Inline::Space => result.push(' '),
            Inline::SoftBreak => result.push(' '),
            Inline::LineBreak => result.push('\n'),
            Inline::Code(_, code) => {
                result.push('`');
                result.push_str(code);
                result.push('`');
            }
            Inline::Math(_, math) => {
                result.push('$');
                result.push_str(math);
                result.push('$');
            }
            Inline::Emph(children)
            | Inline::Strong(children)
            | Inline::Underline(children)
            | Inline::Strikeout(children)
            | Inline::Superscript(children)
            | Inline::Subscript(children)
            | Inline::SmallCaps(children)
            | Inline::Quoted(_, children)
            | Inline::Span(_, children) => {
                result.push_str(&extract_inlines(children));
            }
            Inline::Cite(_, children) => {
                result.push_str(&extract_inlines(children));
            }
            Inline::Link(_, children, _) => {
                result.push_str(&extract_inlines(children));
            }
            Inline::Image(_, children, _) => {
                result.push_str(&extract_inlines(children));
            }
            Inline::RawInline(_, raw) => {
                result.push_str(raw);
            }
            Inline::Note(_) => {
                // Footnotes -- skip inline, could recurse
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pandoc_availability_check() {
        // Just verify the function doesn't panic
        let _ = pandoc_available();
    }

    #[test]
    fn pandoc_parser_fallback_on_missing() {
        // If pandoc isn't available, should return input as prose
        let parser = PandocParser::new("nonexistent-format");
        let regions = parser.parse("Hello world.");
        assert!(!regions.is_empty());
    }

    #[test]
    fn pandoc_format_detection() {
        assert_eq!(
            PandocParser::format_for_path(Path::new("paper.typ")),
            Some("typst".to_string())
        );
        assert_eq!(
            PandocParser::format_for_path(Path::new("doc.adoc")),
            Some("asciidoc".to_string())
        );
        assert_eq!(PandocParser::format_for_path(Path::new("file.xyz")), None);
    }

    #[test]
    #[ignore] // Requires pandoc on PATH
    fn pandoc_parses_markdown() {
        if !pandoc_available() {
            return;
        }
        let parser = PandocParser::new("markdown");
        let regions = parser
            .parse("Hello world. Second sentence.\n\n```python\nprint('hi')\n```\n\nMore text.");
        let prose_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Prose(_)))
            .count();
        let structure_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Structure(_)))
            .count();
        assert!(
            prose_count >= 2,
            "Expected prose regions, got {prose_count}"
        );
        assert!(
            structure_count >= 1,
            "Expected structure regions, got {structure_count}"
        );
    }
}
