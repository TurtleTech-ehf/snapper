//! Apply snapper to a document **after** pandoc has parsed it.
//!
//! Pipeline for the pandoc path:
//! 1. Pandoc reads the source (CLI `pandoc -t json` or in-process FFI) → AST.
//! 2. This module walks **pandoc block/inline node kinds** and marks which
//!    pieces snapper may reflow vs must leave alone.
//! 3. Snapper’s reflow runs only on prose regions.
//!
//! Math and code are never reflowed as prose:
//! - `CodeBlock` → [`Region::Code`]
//! - `Para`/`Plain` that are **only** display/inline math → structure
//! - Mixed paragraphs: split into prose runs and structure islands for
//!   `Math` / inline `Code` so periods inside math cannot end a sentence.
//!
//! Structure truth is the node kind, not source regex after a successful parse.

use pandoc_ast::{Block, Inline, MathType, Pandoc};

use crate::parser::Region;

/// Map a pandoc document to snapper regions for reflow.
///
/// | Pandoc node | Snapper treatment |
/// |-------------|-------------------|
/// | `Para` / `Plain` of ordinary text | [`Region::Prose`] (reflow) |
/// | `Para` / `Plain` with `Math` / inline `Code` | split: prose runs + structure islands |
/// | display-math-only `Para` | [`Region::Structure`] |
/// | `Header` | [`Region::Structure`] |
/// | `CodeBlock` | [`Region::Code`] |
/// | `Table` | [`Region::Structure`] |
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
            extract_para_inlines(inlines, regions);
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

/// True when the para is only math (and ignorable space/breaks) — display or
/// bare math blocks that must not go through the sentence reflower.
fn is_math_only_para(inlines: &[Inline]) -> bool {
    let mut saw_math = false;
    for inline in inlines {
        match inline {
            Inline::Math(..) => saw_math = true,
            Inline::Space | Inline::SoftBreak | Inline::LineBreak => {}
            _ => return false,
        }
    }
    saw_math
}

fn format_math(ty: &MathType, body: &str) -> String {
    match ty {
        MathType::DisplayMath => {
            // Keep payload as structure; delimiters are not source-faithful, only non-prose.
            let b = body.trim();
            if b.is_empty() {
                "$$\n".to_string()
            } else if b.contains('\n') {
                format!("$$\n{b}\n$$\n")
            } else {
                format!("$${b}$$\n")
            }
        }
        MathType::InlineMath => format!("${body}$"),
    }
}

fn flush_prose_buf(prose: &mut String, regions: &mut Vec<Region>, trim_trailing: bool) {
    // Drop all-whitespace buffers. When flushing mid-paragraph before a math
    // island, keep a trailing space so "word $math$" does not become "word$math$".
    if prose.chars().all(|c| c.is_whitespace()) {
        prose.clear();
        return;
    }
    let s = if trim_trailing {
        prose.trim_end()
    } else {
        prose.as_str()
    };
    if !s.is_empty() {
        regions.push(Region::Prose(s.to_string()));
    }
    prose.clear();
}

/// Split a pandoc paragraph into reflowable prose runs and non-prose islands
/// (`Math`, inline `Code`) so periods inside math/code never end a sentence.
fn extract_para_inlines(inlines: &[Inline], regions: &mut Vec<Region>) {
    if inlines.is_empty() {
        return;
    }

    if is_math_only_para(inlines) {
        let mut out = String::new();
        for inline in inlines {
            if let Inline::Math(ty, body) = inline {
                out.push_str(&format_math(ty, body));
            }
        }
        if !out.is_empty() {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            regions.push(Region::Structure(out));
        }
        return;
    }

    let mut prose = String::new();
    walk_para_inlines(inlines, regions, &mut prose);
    flush_prose_buf(&mut prose, regions, true);
}

/// Recursively walk inlines: `Math` / `Code` → structure islands; text → prose.
fn walk_para_inlines(inlines: &[Inline], regions: &mut Vec<Region>, prose: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Math(ty, body) => {
                flush_prose_buf(prose, regions, false);
                let mut s = format_math(ty, body);
                if matches!(ty, MathType::DisplayMath) {
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                } else {
                    // Trailing space so reflow can join "…$math$ word" without
                    // relying on a leading space that sentence-split trims.
                    s.push(' ');
                }
                regions.push(Region::Structure(s));
            }
            Inline::Code(_, code) => {
                flush_prose_buf(prose, regions, false);
                regions.push(Region::Structure(format!("`{code}` ")));
            }
            Inline::Str(s) => prose.push_str(s),
            Inline::Space => prose.push(' '),
            Inline::SoftBreak => prose.push(' '),
            Inline::LineBreak => prose.push('\n'),
            Inline::Emph(children)
            | Inline::Strong(children)
            | Inline::Underline(children)
            | Inline::Strikeout(children)
            | Inline::Superscript(children)
            | Inline::Subscript(children)
            | Inline::SmallCaps(children)
            | Inline::Quoted(_, children)
            | Inline::Span(_, children)
            | Inline::Cite(_, children)
            | Inline::Link(_, children, _)
            | Inline::Image(_, children, _) => {
                walk_para_inlines(children, regions, prose);
            }
            Inline::RawInline(_, raw) => prose.push_str(raw),
            Inline::Note(_) => {}
        }
    }
}

/// Flatten inlines for non-prose contexts (headers, list terms) — not reflowed.
fn extract_inlines(inlines: &[Inline]) -> String {
    let mut result = String::new();
    flatten_inlines(inlines, &mut result);
    result
}

fn flatten_inlines(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Str(s) => out.push_str(s),
            Inline::Space => out.push(' '),
            Inline::SoftBreak => out.push(' '),
            Inline::LineBreak => out.push('\n'),
            Inline::Code(_, code) => {
                out.push('`');
                out.push_str(code);
                out.push('`');
            }
            Inline::Math(ty, body) => out.push_str(&format_math(ty, body)),
            Inline::Emph(c)
            | Inline::Strong(c)
            | Inline::Underline(c)
            | Inline::Strikeout(c)
            | Inline::Superscript(c)
            | Inline::Subscript(c)
            | Inline::SmallCaps(c)
            | Inline::Quoted(_, c)
            | Inline::Span(_, c)
            | Inline::Cite(_, c)
            | Inline::Link(_, c, _)
            | Inline::Image(_, c, _) => flatten_inlines(c, out),
            Inline::RawInline(_, raw) => out.push_str(raw),
            Inline::Note(_) => {}
        }
    }
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
        let heading =
            heading.expect(&format!("expected Structure from Header node, got {regions:?}"));
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
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Hello"))),
            "Para nodes remain Prose: {regions:?}"
        );
    }

    #[test]
    fn ast_codeblock_and_display_math_not_prose() {
        let json = include_str!("../../../tests/fixtures/pandoc_ast/math_code_md.json");
        let regions = regions_from_pandoc_json(json).expect("math_code_md.json");

        assert!(
            regions.iter().any(|r| matches!(r, Region::Code { body, .. } if body.contains("print"))),
            "CodeBlock → Code: {regions:?}"
        );
        // Display-math-only Para → Structure containing math body, not Prose.
        assert!(
            regions.iter().any(|r| {
                matches!(r, Region::Structure(s) if s.contains("1.5") || s.contains("x = 1"))
            }),
            "display math must be Structure: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(r, Region::Prose(p) if p.contains("1.5") && p.contains("y = 2"))
            }),
            "display math body must not be Prose: {regions:?}"
        );
        // Inline math island: mc^2. period not in Prose.
        assert!(
            regions.iter().any(|r| {
                matches!(r, Region::Structure(s) if s.contains("mc^2") || s.contains("E = mc"))
            }),
            "inline math → Structure island: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("mc^2"))),
            "inline math payload must not sit in Prose: {regions:?}"
        );
        // Ordinary multi-sentence prose still Prose.
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First sentence"))),
            "plain prose Para: {regions:?}"
        );
    }

    #[test]
    fn ast_latex_equation_and_minted_not_prose() {
        let json = include_str!("../../../tests/fixtures/pandoc_ast/math_code_tex.json");
        let regions = regions_from_pandoc_json(json).expect("math_code_tex.json");

        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("mc^2") || s.contains("E = mc"))),
            "equation DisplayMath → Structure: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("mc^2"))),
            "equation not Prose: {regions:?}"
        );
        assert!(
            regions.iter().filter(|r| matches!(r, Region::Code { .. })).count() >= 1,
            "minted/lstlisting/verbatim as CodeBlock: {regions:?}"
        );
        for r in &regions {
            if let Region::Code { body, .. } = r {
                // Code bodies must not be reflowed as prose regions.
                assert!(
                    !matches!(r, Region::Prose(_)),
                    "code body present: {body}"
                );
            }
        }
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Hello") || p.contains("End"))),
            "body prose remains: {regions:?}"
        );
    }
}
