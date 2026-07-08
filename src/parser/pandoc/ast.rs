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

use pandoc_ast::{Block, Cell, Inline, MathType, Pandoc, Row, TableBody, TableFoot, TableHead};

use crate::parser::Region;

/// Map a pandoc document to snapper regions for reflow.
///
/// | Pandoc node | Snapper treatment |
/// |-------------|-------------------|
/// | `Para` / `Plain` of ordinary text | [`Region::Prose`] (reflow) |
/// | `Para` / `Plain` with `Math` / inline `Code` | split: prose runs + structure islands |
/// | display-math-only `Para` | [`Region::Structure`] |
/// | `Header` | [`Region::Structure`] |
/// | `CodeBlock` | [`Region::Code`] (fenced from Attr) |
/// | `Table` | [`Region::Structure`] (pipe table from cells) |
/// | `BulletList` / `OrderedList` | marker structure + item blocks |
/// | `BlockQuote` | `>` markers + nested blocks |
pub fn regions_from_pandoc(doc: &Pandoc) -> Vec<Region> {
    let mut regions = Vec::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        if i > 0 {
            push_block_separator(&mut regions);
        }
        extract_block(block, &mut regions);
    }
    regions
}

/// Blank line between top-level blocks (pandoc does not emit blank nodes).
fn push_block_separator(regions: &mut Vec<Region>) {
    if regions.is_empty() {
        return;
    }
    if matches!(regions.last(), Some(Region::BlankLines(_))) {
        return;
    }
    regions.push(Region::BlankLines("\n".to_string()));
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
            // Pandoc's CodeBlock is authoritative (fenced, minted, lstlisting, …).
            // Emit a coherent Region::Code: lang from Attr; minimal fence framing
            // for reflow output (not source-glyph fidelity, not a native re-parse).
            let lang = code_lang_from_attr(attr);
            let body = if code.ends_with('\n') {
                code.clone()
            } else if code.is_empty() {
                String::new()
            } else {
                format!("{code}\n")
            };
            let (header, footer) = code_fence_frame(lang.as_deref());
            regions.push(Region::Code {
                lang,
                header,
                body,
                footer,
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
            for (i, b) in blocks.iter().enumerate() {
                if i > 0 {
                    push_block_separator(regions);
                }
                extract_quoted_block(b, regions);
            }
        }
        Block::BulletList(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    // single newline between items (not a full blank)
                    ensure_trailing_newline(regions);
                }
                extract_list_item("- ", item, regions);
            }
        }
        Block::OrderedList(_, items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    ensure_trailing_newline(regions);
                }
                let marker = format!("{}. ", i + 1);
                extract_list_item(&marker, item, regions);
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
        Block::Table(_attr, _caption, _cols, head, bodies, foot) => {
            let rendered = render_table_structure(head, bodies, foot);
            if !rendered.is_empty() {
                regions.push(Region::Structure(rendered));
            }
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
    // Prefer first non-empty class; pandoc also stores language= on keyvals for lstlisting.
    if let Some(c) = classes.first().cloned().filter(|c| !c.is_empty()) {
        return Some(c);
    }
    for (k, v) in _kvs {
        if k.eq_ignore_ascii_case("language") && !v.is_empty() {
            return Some(v.clone());
        }
    }
    None
}

/// Minimal fence framing so reflow emits a coherent code unit (header/body/footer).
fn code_fence_frame(lang: Option<&str>) -> (String, String) {
    let header = match lang {
        Some(l) if !l.is_empty() => format!("```{l}\n"),
        _ => "```\n".to_string(),
    };
    (header, "```\n".to_string())
}

fn ensure_trailing_newline(regions: &mut Vec<Region>) {
    match regions.last() {
        Some(Region::Structure(s) | Region::Prose(s) | Region::BlankLines(s))
            if s.ends_with('\n') => {}
        Some(Region::Code { footer, .. }) if footer.ends_with('\n') => {}
        Some(_) => regions.push(Region::Structure("\n".to_string())),
        None => {}
    }
}

fn extract_quoted_block(block: &Block, regions: &mut Vec<Region>) {
    match block {
        Block::Para(inlines) | Block::Plain(inlines) => {
            regions.push(Region::Structure("> ".to_string()));
            extract_para_inlines(inlines, regions);
            ensure_trailing_newline(regions);
        }
        other => {
            regions.push(Region::Structure("> ".to_string()));
            extract_block(other, regions);
        }
    }
}

fn extract_list_item(marker: &str, item: &[Block], regions: &mut Vec<Region>) {
    regions.push(Region::Structure(marker.to_string()));
    for (i, block) in item.iter().enumerate() {
        if i == 0 {
            match block {
                Block::Para(inlines) | Block::Plain(inlines) => {
                    extract_para_inlines(inlines, regions);
                    ensure_trailing_newline(regions);
                }
                other => extract_block(other, regions),
            }
        } else {
            push_block_separator(regions);
            extract_block(block, regions);
        }
    }
}

/// Pipe-table structure from pandoc table AST (cells only; non-prose).
fn render_table_structure(head: &TableHead, bodies: &[TableBody], foot: &TableFoot) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let (_hattr, head_rows) = head;
    for row in head_rows {
        rows.push(row_cells(row));
    }
    let head_count = head_rows.len();
    for body in bodies {
        let (_battr, _rhc, intermediate, body_rows) = body;
        for row in intermediate {
            rows.push(row_cells(row));
        }
        for row in body_rows {
            rows.push(row_cells(row));
        }
    }
    let (_fattr, foot_rows) = foot;
    for row in foot_rows {
        rows.push(row_cells(row));
    }
    if rows.is_empty() {
        return String::new();
    }
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if ncols == 0 {
        return String::new();
    }
    for r in &mut rows {
        while r.len() < ncols {
            r.push(String::new());
        }
    }
    // Separator after header rows (or after first row if no header).
    let sep_after = if head_count > 0 { head_count - 1 } else { 0 };
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        out.push_str("| ");
        out.push_str(
            &row.iter()
                .map(|c| c.replace('|', "\\|"))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
        if i == sep_after {
            out.push('|');
            for _ in 0..ncols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out
}

fn row_cells(row: &Row) -> Vec<String> {
    let (_attr, cells) = row;
    cells.iter().map(cell_text).collect()
}

fn cell_text(cell: &Cell) -> String {
    let (_attr, _align, _rs, _cs, blocks) = cell;
    let mut parts = Vec::new();
    for b in blocks {
        match b {
            Block::Plain(inlines) | Block::Para(inlines) => {
                parts.push(extract_inlines(inlines));
            }
            Block::CodeBlock(_, code) => parts.push(code.clone()),
            Block::RawBlock(_, raw) => parts.push(raw.clone()),
            _ => {}
        }
    }
    parts.join(" ").trim().to_string()
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
                // trim_trailing: sentence splitter also trims; put spaces on the
                // structure island so "word $math$ word" survives reflow.
                let need_lead = !prose.is_empty() && !prose.ends_with('\n');
                flush_prose_buf(prose, regions, true);
                let mut s = String::new();
                if matches!(ty, MathType::DisplayMath) {
                    if need_lead {
                        // display math on its own lines
                    }
                    s.push_str(&format_math(ty, body));
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                } else {
                    if need_lead {
                        s.push(' ');
                    }
                    s.push_str(&format_math(ty, body));
                    s.push(' ');
                }
                regions.push(Region::Structure(s));
            }
            Inline::Code(_, code) => {
                let need_lead = !prose.is_empty() && !prose.ends_with('\n');
                flush_prose_buf(prose, regions, true);
                let mut s = String::new();
                if need_lead {
                    s.push(' ');
                }
                s.push('`');
                s.push_str(code);
                s.push_str("` ");
                regions.push(Region::Structure(s));
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
            Region::Code {
                lang,
                header,
                body,
                footer,
            } => Some((lang.clone(), header.clone(), body.clone(), footer.clone())),
            _ => None,
        });
        let (lang, header, body, footer) = code.expect("CodeBlock must become Region::Code");
        assert_eq!(lang.as_deref(), Some("python"));
        assert!(
            header.starts_with("```") && header.contains("python"),
            "fence header from Attr lang: {header:?}"
        );
        assert!(footer.starts_with("```"), "fence footer: {footer:?}");
        assert!(body.contains("print(1)"), "code body: {body}");

        let has_table = regions.iter().any(|r| {
            matches!(r, Region::Structure(s) if s.contains('|') && (s.contains('a') || s.contains("---")))
        });
        assert!(has_table, "Table must be pipe Structure from cells: {regions:?}");

        for r in &regions {
            if let Region::Prose(s) = r {
                assert!(
                    !s.contains("print(1)") && !s.lines().any(|l| l.trim().starts_with('|')),
                    "structure leaked into prose: {s}"
                );
            }
        }
    }

    #[test]
    fn ast_table_list_quote_from_structure_fixture() {
        let json = include_str!("../../../tests/fixtures/pandoc_ast/structure_blocks.json");
        let regions = regions_from_pandoc_json(json).expect("structure_blocks.json");

        let table = regions.iter().find_map(|r| match r {
            Region::Structure(s) if s.contains('|') && s.contains("---") => Some(s.as_str()),
            _ => None,
        });
        let table = table.expect(&format!("pipe table structure: {regions:?}"));
        assert!(table.contains('a') && table.contains('b') && table.contains('1'));
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains('|') && p.contains("---"))),
            "table not Prose: {regions:?}"
        );

        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "-" || s.starts_with("- "))),
            "bullet marker: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.starts_with("> "))),
            "blockquote marker: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Intro") || p.contains("End"))),
            "body prose: {regions:?}"
        );
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

        let code = regions.iter().find(|r| {
            matches!(r, Region::Code { body, .. } if body.contains("print"))
        });
        match code {
            Some(Region::Code {
                lang,
                header,
                body,
                footer,
            }) => {
                assert_eq!(lang.as_deref(), Some("python"));
                assert!(header.contains("```") && header.contains("python"), "{header}");
                assert!(footer.contains("```"), "{footer}");
                assert!(body.contains("print(1.0)") && body.contains("x = 2."));
            }
            other => panic!("CodeBlock → Region::Code with fence: {other:?} / {regions:?}"),
        }
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
