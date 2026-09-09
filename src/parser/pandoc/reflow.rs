//! Reflow Para/Plain in a pandoc AST. Other node kinds stay structure.
//!
//! Sentence (and optional wrap) breaks become `SoftBreak` inlines. The
//! writer (`--wrap=preserve`) turns those into source line breaks. Math
//! and inline Code are masked so their periods cannot end a sentence.

use pandoc_ast::{Block, Inline, Pandoc};

use super::ast::{format_math, is_math_only_para};
use crate::reflow::{ReflowConfig, wrap_sentence_plain};
use crate::sentence::SentenceSplitter;

/// Insert sentence/wrap SoftBreaks into every prose Para/Plain.
///
/// `Header`, `CodeBlock`, `Table`, and `LineBlock` are left untouched.
/// Lists, quotes, definition bodies, Div, and Figure keep their node
/// kind; nested Para/Plain still get sentence breaks.
pub fn reflow_pandoc(doc: &mut Pandoc, splitter: &dyn SentenceSplitter, config: &ReflowConfig) {
    reflow_blocks(&mut doc.blocks, splitter, config, false);
}

fn reflow_blocks(
    blocks: &mut [Block],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
    in_table: bool,
) {
    for block in blocks {
        match block {
            Block::Para(inlines) | Block::Plain(inlines) => {
                if in_table || is_math_only_para(inlines) {
                    continue;
                }
                *inlines = reflow_inlines(std::mem::take(inlines), splitter, config);
            }
            Block::Header(..) | Block::CodeBlock(..) | Block::RawBlock(..) | Block::Null => {}
            Block::LineBlock(_) | Block::HorizontalRule => {}
            Block::BlockQuote(inner) | Block::Div(_, inner) | Block::Figure(_, _, inner) => {
                reflow_blocks(inner, splitter, config, in_table);
            }
            Block::BulletList(items) | Block::OrderedList(_, items) => {
                for item in items {
                    reflow_blocks(item, splitter, config, in_table);
                }
            }
            Block::DefinitionList(defs) => {
                for (_term, definitions) in defs {
                    for def in definitions {
                        reflow_blocks(def, splitter, config, in_table);
                    }
                }
            }
            Block::Table(_attr, _caption, _cols, head, bodies, foot) => {
                let (_hattr, head_rows) = head;
                for row in head_rows {
                    reflow_table_row(row, splitter, config);
                }
                for body in bodies {
                    let (_battr, _rhc, intermediate, body_rows) = body;
                    for row in intermediate {
                        reflow_table_row(row, splitter, config);
                    }
                    for row in body_rows {
                        reflow_table_row(row, splitter, config);
                    }
                }
                let (_fattr, foot_rows) = foot;
                for row in foot_rows {
                    reflow_table_row(row, splitter, config);
                }
            }
        }
    }
}

fn reflow_table_row(
    _row: &mut pandoc_ast::Row,
    _splitter: &dyn SentenceSplitter,
    _config: &ReflowConfig,
) {
    // Table stays structure: cell Paras are not sentence-split.
}

fn reflow_inlines(
    inlines: Vec<Inline>,
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> Vec<Inline> {
    if inlines.is_empty() {
        return inlines;
    }
    let masked = mask_protected(&inlines);
    if masked.chars().all(|c| c.is_whitespace()) {
        return inlines;
    }
    let breaks = break_offsets(&masked, splitter, config);
    if breaks.is_empty() {
        return inlines;
    }
    let mut out = Vec::with_capacity(inlines.len() + breaks.len());
    let mut pos = 0usize;
    let mut bi = 0usize;
    insert_breaks(inlines, &mut pos, &breaks, &mut bi, &mut out);
    while bi < breaks.len() && breaks[bi] == pos {
        push_soft_break(&mut out);
        bi += 1;
    }
    out
}

/// Flatten like [`inlines_to_string`], but Math/Code become `X` of the same
/// width so periods inside them cannot end a sentence.
fn mask_protected(inlines: &[Inline]) -> String {
    let mut out = String::new();
    mask_into(inlines, &mut out);
    out
}

fn mask_into(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Str(s) => out.push_str(s),
            Inline::Space | Inline::SoftBreak => out.push(' '),
            Inline::LineBreak => out.push('\n'),
            Inline::Code(_, code) => {
                let n = 2 + code.len();
                out.extend(std::iter::repeat_n('X', n));
            }
            Inline::Math(ty, body) => {
                let n = format_math(ty, body).len();
                out.extend(std::iter::repeat_n('X', n));
            }
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
            | Inline::Image(_, c, _) => mask_into(c, out),
            Inline::RawInline(_, raw) => out.push_str(raw),
            Inline::Note(_) => {}
        }
    }
}

fn break_offsets(text: &str, splitter: &dyn SentenceSplitter, config: &ReflowConfig) -> Vec<usize> {
    let sentences = splitter.split(text);
    if sentences.len() < 2 && config.max_width == 0 && !config.clause_breaks {
        return Vec::new();
    }
    let mut breaks = Vec::new();
    let mut cur = 0usize;
    let nsent = sentences.len();
    for (i, sent) in sentences.iter().enumerate() {
        let needle = sent.trim();
        if needle.is_empty() {
            continue;
        }
        let Some(rel) = text.get(cur..).and_then(|rest| rest.find(needle)) else {
            continue;
        };
        let start = cur + rel;
        let end = start + needle.len();
        if config.max_width > 0 || config.clause_breaks {
            let wrapped = wrap_sentence_plain(
                needle,
                config.max_width,
                config.clause_breaks,
                config.format,
            );
            let mut off = start;
            let parts: Vec<&str> = wrapped.split('\n').collect();
            for (j, part) in parts.iter().enumerate() {
                let p = part.trim_start();
                if !p.is_empty() {
                    if let Some(r) = text.get(off..end).and_then(|rest| rest.find(p)) {
                        off = off + r + p.len();
                    } else {
                        off = (off + p.len()).min(end);
                    }
                }
                if j + 1 < parts.len() && off > start && off < end {
                    breaks.push(off);
                }
            }
        }
        if i + 1 < nsent {
            breaks.push(end);
        }
        cur = end;
    }
    breaks.sort_unstable();
    breaks.dedup();
    breaks
}

fn insert_breaks(
    inlines: Vec<Inline>,
    pos: &mut usize,
    breaks: &[usize],
    bi: &mut usize,
    out: &mut Vec<Inline>,
) {
    for inline in inlines {
        flush_breaks_at(*pos, breaks, bi, out);
        match inline {
            Inline::Str(s) => emit_str(s, pos, breaks, bi, out),
            Inline::Space | Inline::SoftBreak => {
                let start = *pos;
                *pos += 1;
                if *bi < breaks.len() && (breaks[*bi] == start || breaks[*bi] == *pos) {
                    push_soft_break(out);
                    *bi += 1;
                    while *bi < breaks.len() && breaks[*bi] == *pos {
                        *bi += 1;
                    }
                } else {
                    out.push(Inline::Space);
                }
            }
            Inline::LineBreak => {
                *pos += 1;
                out.push(Inline::LineBreak);
            }
            Inline::Code(attr, code) => {
                let n = 2 + code.len();
                emit_protected(Inline::Code(attr, code), n, pos, breaks, bi, out);
            }
            Inline::Math(ty, body) => {
                let n = format_math(&ty, &body).len();
                emit_protected(Inline::Math(ty, body), n, pos, breaks, bi, out);
            }
            Inline::Emph(c) => emit_container(c, pos, breaks, bi, out, Inline::Emph),
            Inline::Strong(c) => emit_container(c, pos, breaks, bi, out, Inline::Strong),
            Inline::Underline(c) => emit_container(c, pos, breaks, bi, out, Inline::Underline),
            Inline::Strikeout(c) => emit_container(c, pos, breaks, bi, out, Inline::Strikeout),
            Inline::Superscript(c) => emit_container(c, pos, breaks, bi, out, Inline::Superscript),
            Inline::Subscript(c) => emit_container(c, pos, breaks, bi, out, Inline::Subscript),
            Inline::SmallCaps(c) => emit_container(c, pos, breaks, bi, out, Inline::SmallCaps),
            Inline::Quoted(q, c) => {
                let mut inner = Vec::new();
                insert_breaks(c, pos, breaks, bi, &mut inner);
                out.push(Inline::Quoted(q, inner));
            }
            Inline::Span(attr, c) => {
                let mut inner = Vec::new();
                insert_breaks(c, pos, breaks, bi, &mut inner);
                out.push(Inline::Span(attr, inner));
            }
            Inline::Cite(cite, c) => {
                let mut inner = Vec::new();
                insert_breaks(c, pos, breaks, bi, &mut inner);
                out.push(Inline::Cite(cite, inner));
            }
            Inline::Link(attr, c, target) => {
                let mut inner = Vec::new();
                insert_breaks(c, pos, breaks, bi, &mut inner);
                out.push(Inline::Link(attr, inner, target));
            }
            Inline::Image(attr, c, target) => {
                let mut inner = Vec::new();
                insert_breaks(c, pos, breaks, bi, &mut inner);
                out.push(Inline::Image(attr, inner, target));
            }
            Inline::RawInline(fmt, raw) => {
                emit_str_at(raw, pos, breaks, bi, out, |s| {
                    Inline::RawInline(fmt.clone(), s)
                });
            }
            Inline::Note(note) => out.push(Inline::Note(note)),
        }
    }
}

fn emit_container<F>(
    children: Vec<Inline>,
    pos: &mut usize,
    breaks: &[usize],
    bi: &mut usize,
    out: &mut Vec<Inline>,
    wrap: F,
) where
    F: FnOnce(Vec<Inline>) -> Inline,
{
    let mut inner = Vec::new();
    insert_breaks(children, pos, breaks, bi, &mut inner);
    out.push(wrap(inner));
}

fn emit_protected(
    inline: Inline,
    width: usize,
    pos: &mut usize,
    breaks: &[usize],
    bi: &mut usize,
    out: &mut Vec<Inline>,
) {
    let start = *pos;
    let end = start + width;
    while *bi < breaks.len() && breaks[*bi] > start && breaks[*bi] < end {
        *bi += 1;
    }
    flush_breaks_at(start, breaks, bi, out);
    out.push(inline);
    *pos = end;
    flush_breaks_at(end, breaks, bi, out);
}

fn emit_str(s: String, pos: &mut usize, breaks: &[usize], bi: &mut usize, out: &mut Vec<Inline>) {
    emit_str_at(s, pos, breaks, bi, out, Inline::Str);
}

fn emit_str_at<F>(
    s: String,
    pos: &mut usize,
    breaks: &[usize],
    bi: &mut usize,
    out: &mut Vec<Inline>,
    wrap: F,
) where
    F: Fn(String) -> Inline,
{
    let start = *pos;
    let len = s.len();
    let end = start + len;
    let mut last = 0usize;
    while *bi < breaks.len() && breaks[*bi] < end {
        let at = breaks[*bi];
        if at < start {
            *bi += 1;
            continue;
        }
        let rel = at - start;
        if rel > last {
            if let Some(chunk) = s.get(last..rel) {
                if !chunk.is_empty() {
                    out.push(wrap(chunk.to_string()));
                }
            }
        }
        push_soft_break(out);
        *bi += 1;
        last = rel;
        while last < len && s.as_bytes()[last].is_ascii_whitespace() {
            last += 1;
        }
    }
    if last < len {
        out.push(wrap(s[last..].to_string()));
    }
    *pos = end;
}

fn flush_breaks_at(at: usize, breaks: &[usize], bi: &mut usize, out: &mut Vec<Inline>) {
    while *bi < breaks.len() && breaks[*bi] == at {
        push_soft_break(out);
        *bi += 1;
    }
}

fn push_soft_break(out: &mut Vec<Inline>) {
    if matches!(out.last(), Some(Inline::Space | Inline::SoftBreak)) {
        out.pop();
    }
    if !matches!(out.last(), Some(Inline::SoftBreak)) {
        out.push(Inline::SoftBreak);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Format;
    use crate::sentence::unicode::UnicodeSentenceSplitter;
    use pandoc_ast::Block;

    fn flat(inlines: &[Inline]) -> String {
        format!("{inlines:?}")
    }

    fn cfg() -> ReflowConfig<'static> {
        ReflowConfig {
            format: Format::Markdown,
            ..ReflowConfig::default()
        }
    }

    fn para_from(text: &str) -> Pandoc {
        let json = regions_json_para(text);
        serde_json::from_str(&json).expect("tiny pandoc")
    }

    fn regions_json_para(text: &str) -> String {
        // Minimal pandoc JSON: one Para of a single Str.
        format!(
            r#"{{"pandoc-api-version":[1,23,1],"meta":{{}},"blocks":[{{"t":"Para","c":[{{"t":"Str","c":{}}}]}}]}}"#,
            serde_json::to_string(text).unwrap()
        )
    }

    #[test]
    fn reflow_inserts_softbreak_between_sentences() {
        let mut doc = para_from("Hello world. Second sentence.");
        let splitter = UnicodeSentenceSplitter::new();
        reflow_pandoc(&mut doc, &splitter, &cfg());
        match &doc.blocks[0] {
            Block::Para(inlines) => {
                assert!(
                    inlines.iter().any(|i| matches!(i, Inline::SoftBreak)),
                    "expected SoftBreak in {inlines:?}"
                );
                let flat = flat(inlines);
                assert!(flat.contains("Hello world."));
                assert!(flat.contains("Second sentence."));
            }
            other => panic!("expected Para, got {other:?}"),
        }
    }

    #[test]
    fn reflow_leaves_header_and_codeblock() {
        let json = include_str!("../../../tests/fixtures/pandoc_ast/mixed_minimal.json");
        let mut doc: Pandoc = serde_json::from_str(json).expect("fixture");
        let before: Vec<String> = doc
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Header(l, _, ins) => Some(format!("H{l}:{}", flat(ins))),
                Block::CodeBlock(_, c) => Some(format!("C:{c}")),
                _ => None,
            })
            .collect();
        let splitter = UnicodeSentenceSplitter::new();
        reflow_pandoc(&mut doc, &splitter, &cfg());
        let after: Vec<String> = doc
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Header(l, _, ins) => Some(format!("H{l}:{}", flat(ins))),
                Block::CodeBlock(_, c) => Some(format!("C:{c}")),
                _ => None,
            })
            .collect();
        assert_eq!(before, after);
        assert!(
            doc.blocks.iter().any(|b| matches!(b, Block::Table(..))),
            "table node must remain"
        );
    }
}
