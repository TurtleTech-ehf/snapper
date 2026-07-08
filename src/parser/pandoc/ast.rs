//! Pure pandoc AST → [`Region`] mapping.
//!
//! Structure decisions come from pandoc block/inline node kinds only.
//! No line/regex heuristics on the original source.

use pandoc_ast::{Block, Inline, Pandoc};

use crate::parser::Region;

/// Classify a deserialized pandoc document into snapper regions.
///
/// Primary mapping (authoritative for the AST-backed path):
/// - `Para` / `Plain` → [`Region::Prose`]
/// - `Header` → [`Region::Structure`]
/// - `CodeBlock` → [`Region::Code`]
/// - `Table` → [`Region::Structure`] (non-prose)
/// - lists, quotes, divs recurse; rules, raw blocks, line blocks are structure
pub fn regions_from_pandoc(doc: &Pandoc) -> Vec<Region> {
    let mut regions = Vec::new();
    for block in &doc.blocks {
        extract_block(block, &mut regions);
    }
    regions
}

/// Deserialize pandoc JSON and classify. Used by both FFI and CLI backends
/// after they obtain JSON from an in-process or external pandoc.
pub fn regions_from_pandoc_json(json: &str) -> Result<Vec<Region>, String> {
    let doc: Pandoc = serde_json::from_str(json)
        .map_err(|e| format!("failed to deserialize pandoc AST JSON: {e}"))?;
    Ok(regions_from_pandoc(&doc))
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
        Block::Header(level, _attr, inlines) => {
            // Entire heading is one Structure region — same contract as the
            // native ATX fix (snapper-25kc): never Structure(prefix)+Prose(title),
            // so titles like "1. `cargo binstall` …" are not sentence-reflowed.
            // Reconstruct ATX markers from the pandoc Header level so the
            // emitted line remains an obvious single non-prose heading.
            let text = extract_inlines(inlines);
            let title = text.trim();
            if !title.is_empty() {
                let n = (*level).clamp(1, 6) as usize;
                let marks = "#".repeat(n);
                regions.push(Region::Structure(format!("{marks} {title}\n")));
            }
        }
        Block::CodeBlock(attr, code) => {
            let lang = code_lang_from_attr(attr);
            let body = if code.ends_with('\n') {
                code.clone()
            } else if code.is_empty() {
                String::new()
            } else {
                format!("{code}\n")
            };
            regions.push(Region::Code {
                lang,
                header: String::new(),
                body,
                footer: String::new(),
            });
        }
        Block::RawBlock(_, raw) => {
            let s = if raw.ends_with('\n') {
                raw.clone()
            } else {
                format!("{raw}\n")
            };
            regions.push(Region::Structure(s));
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
            for line in lines {
                let text = extract_inlines(line);
                regions.push(Region::Structure(format!("{text}\n")));
            }
        }
    }
}

/// Pandoc `Attr` is `(id, classes, keyvals)`; first class is usually the language.
fn code_lang_from_attr(attr: &pandoc_ast::Attr) -> Option<String> {
    let (_id, classes, _kvs) = attr;
    classes.first().cloned().filter(|c| !c.is_empty())
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
            Inline::Note(_) => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real pandoc-generated JSON (Header, Para, CodeBlock, Table, Para).
    fn mixed_doc_json() -> &'static str {
        include_str!("../../../tests/fixtures/pandoc_ast/mixed_minimal.json")
    }

    #[test]
    fn ast_classifies_prose_heading_code_table_from_node_kinds() {
        let regions = regions_from_pandoc_json(mixed_doc_json()).expect("valid fixture JSON");

        let prose: Vec<_> = regions
            .iter()
            .filter(|r| matches!(r, Region::Prose(_)))
            .collect();
        assert!(
            prose.len() >= 1,
            "expected ≥1 prose region from Para nodes, got {regions:?}"
        );
        assert!(
            prose
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Hello"))),
            "expected paragraph prose, got {prose:?}"
        );

        let has_heading_structure = regions.iter().any(|r| {
            matches!(r, Region::Structure(s) if s.starts_with('#') && s.contains("Title"))
        });
        assert!(
            has_heading_structure,
            "Header must be single ATX-style Structure, not Prose: {regions:?}"
        );

        let code = regions.iter().find_map(|r| match r {
            Region::Code { lang, body, .. } => Some((lang.clone(), body.clone())),
            _ => None,
        });
        let (lang, body) = code.expect("CodeBlock must become Region::Code");
        assert_eq!(lang.as_deref(), Some("python"));
        assert!(body.contains("print(1)"), "code body: {body}");

        let has_table = regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("[table]")));
        assert!(has_table, "Table must be non-prose Structure: {regions:?}");

        for r in &regions {
            if let Region::Prose(s) = r {
                assert!(
                    !s.contains("[table]") && !s.contains("print(1)"),
                    "structure leaked into prose: {s}"
                );
            }
        }
    }

    #[test]
    fn ast_rejects_invalid_json_explicitly() {
        let err = regions_from_pandoc_json("not-json").unwrap_err();
        assert!(
            err.contains("deserialize") || err.contains("failed"),
            "unexpected error: {err}"
        );
    }

    /// Pandoc Header with a numbered title that used to break native ATX reflow.
    /// Contract: one Structure line with reconstructed ATX marks; title never Prose.
    #[test]
    fn ast_numbered_header_is_single_structure_not_prose() {
        let json = include_str!("../../../tests/fixtures/pandoc_ast/numbered_heading.json");
        let regions = regions_from_pandoc_json(json).expect("numbered_heading.json");

        let heading = regions.iter().find_map(|r| match r {
            Region::Structure(s) if s.contains("cargo binstall") => Some(s.as_str()),
            _ => None,
        });
        let heading = heading.expect(&format!("expected Structure heading, got {regions:?}"));
        assert!(
            heading.starts_with("### "),
            "level-3 Header must reconstruct ###, got: {heading:?}"
        );
        assert!(
            heading.contains("1.") && heading.contains("cargo binstall"),
            "full title in one Structure line: {heading:?}"
        );
        assert!(
            heading.ends_with('\n') && !heading[..heading.len() - 1].contains('\n'),
            "single-line Structure heading: {heading:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("cargo binstall"))),
            "heading title must not be Prose: {regions:?}"
        );
        // Must not look like the old bug: orphan "### 1." as its own structure line.
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "### 1." || s.trim() == "### 1")),
            "orphan ### 1. structure forbidden: {regions:?}"
        );
    }
}
