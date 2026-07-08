//! Apply snapper to a document **after** pandoc has parsed it.
//!
//! Pipeline for the pandoc path:
//! 1. Pandoc reads the source (CLI `pandoc -t json` or in-process FFI) → AST.
//! 2. This module walks **pandoc block/inline node kinds** and marks which
//!    pieces snapper may reflow (`Para` / `Plain` → prose) vs must leave alone
//!    (`Header`, `CodeBlock`, `Table`, …).
//! 3. Snapper’s reflow runs only on prose regions.
//!
//! There is no second pass of native markdown/org line heuristics, and no
//! inventing source markup (e.g. ATX `###`) from the AST — structure truth is
//! the node kind, not a reconstructed source line.

use pandoc_ast::{Block, Inline, Pandoc};

use crate::parser::Region;

/// Map a pandoc document to snapper regions for reflow.
///
/// | Pandoc node | Snapper treatment |
/// |-------------|-------------------|
/// | `Para` / `Plain` | [`Region::Prose`] (snapper reflows) |
/// | `Header` | [`Region::Structure`] (never reflow — it is a Header) |
/// | `CodeBlock` | [`Region::Code`] |
/// | `Table` | [`Region::Structure`] |
/// | lists / quotes / divs | recurse into child blocks |
pub fn regions_from_pandoc(doc: &Pandoc) -> Vec<Region> {
    let mut regions = Vec::new();
    for block in &doc.blocks {
        extract_block(block, &mut regions);
    }
    regions
}

/// Deserialize pandoc JSON AST, then [`regions_from_pandoc`].
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
        Block::Header(_level, _attr, inlines) => {
            // Header is non-prose because pandoc classified it as Header — not
            // because we rebuilt an ATX source line. Title text (incl. "1. …")
            // must not enter Prose or snapper would sentence-split it.
            let text = extract_inlines(inlines);
            let title = text.trim();
            if !title.is_empty() {
                regions.push(Region::Structure(format!("{title}\n")));
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

        let has_heading_structure = regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("Title")));
        assert!(
            has_heading_structure,
            "Header node must be Structure (not Prose): {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Title"))),
            "Header title must not be Prose: {regions:?}"
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

    /// After pandoc parse, a Header whose title looks like "1. code …" must
    /// not be Prose — snapper would reflow it. We do not invent ATX markup.
    #[test]
    fn ast_numbered_header_is_structure_not_prose() {
        let json = include_str!("../../../tests/fixtures/pandoc_ast/numbered_heading.json");
        let regions = regions_from_pandoc_json(json).expect("numbered_heading.json");

        let heading = regions.iter().find_map(|r| match r {
            Region::Structure(s) if s.contains("cargo binstall") => Some(s.as_str()),
            _ => None,
        });
        let heading = heading.expect(&format!("expected Structure from Header node, got {regions:?}"));
        assert!(
            heading.contains("1.") && heading.contains("cargo binstall"),
            "Header title text preserved as structure payload: {heading:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("cargo binstall"))),
            "Header title must not be Prose (would be sentence-reflowed): {regions:?}"
        );
        // Prose body still reflowable.
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Hello"))),
            "Para nodes remain Prose: {regions:?}"
        );
    }
}
