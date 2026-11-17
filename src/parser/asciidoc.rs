//! Native AsciiDoc parser.
//!
//! Paragraphs, list bodies, definition lists, and admonition bodies are
//! prose. Headings, attributes, block titles, comments, macros, and
//! delimited listings stay structure. A `[source]` listing or a backtick
//! fence is a code region so `[code.<lang>]` can reflow its comments.
//! Example, sidebar, quote, and open blocks reflow their interiors.
//!
//! An indented line that does not continue a list is a literal paragraph
//! and stays structure. List and admonition continuations hang at the
//! marker, with the marker in its own structure slice, so the oracle can
//! join the indented line back into the item.

use crate::parser::{
    ByteSpan, FormatParser, Line, ProseBuf, SpannedRegion, flush_prose_spanned, iter_lines,
    join_prose_gap, push_prose_line,
};

pub struct AsciiDocParser;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Delim {
    Listing,
    Literal,
    Example,
    Sidebar,
    Quote,
    Passthrough,
    Comment,
    Open,
    Table,
    Fence,
}

#[derive(Clone, Debug)]
enum Style {
    Run { ch: char, min: usize },
    Backtick { min: usize },
    Exact(&'static str),
    Table,
}

#[derive(Clone, Debug)]
enum OpenKind {
    /// Delimited block whose interior is not prose.
    Frozen,
    /// Example, sidebar, quote, or open block.
    Prose,
    /// Source listing or backtick fence. Lines are held until the closer.
    Code { lang: Option<String> },
}

struct OpenBlock {
    style: Style,
    kind: OpenKind,
    /// Opener line, including its terminator. Set for every block.
    header: ByteSpan,
    /// Index of the opener in the line list. Code lines are emitted from here
    /// when the block never closes.
    start_index: usize,
}

#[derive(Clone, Copy)]
struct Hang {
    width: usize,
    empty_body: bool,
}

impl FormatParser for AsciiDocParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let lines = iter_lines(input);
        let mut regions = Vec::new();
        let mut prose = ProseBuf::default();
        let mut prose_span: Option<ByteSpan> = None;
        let mut term: Option<ByteSpan> = None;
        let mut hang: Option<Hang> = None;
        let mut stack: Vec<OpenBlock> = Vec::new();
        let mut pending_source: Option<Option<String>> = None;
        let mut pending_para = false;
        let mut pragma_off = false;
        let mut literal = false;
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if let Some(top) = stack.last() {
                if matches!(top.kind, OpenKind::Frozen | OpenKind::Code { .. }) {
                    if closes(line.text, &top.style) {
                        close_block(input, &lines, &mut stack, i, &mut regions);
                    } else if matches!(top.kind, OpenKind::Frozen) {
                        regions.push(SpannedRegion::structure(input, line.span()));
                    }
                    i += 1;
                    continue;
                }
                if closes(line.text, &top.style) {
                    flush_item(
                        input,
                        &mut prose,
                        &mut prose_span,
                        &mut term,
                        &mut hang,
                        &mut regions,
                    );
                    regions.push(SpannedRegion::structure(input, line.span()));
                    stack.pop();
                    literal = false;
                    i += 1;
                    continue;
                }
            }

            if line.text.trim().is_empty() {
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                pending_para = false;
                regions.push(SpannedRegion::blank(input, line.span()));
                literal = false;
                i += 1;
                continue;
            }

            if let Some(on) = adoc_pragma(line.text) {
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                pragma_off = !on;
                regions.push(SpannedRegion::structure(input, line.span()));
                literal = false;
                i += 1;
                continue;
            }
            if pragma_off {
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if literal {
                if leading_ws(line.text) > 0 {
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
                literal = false;
            }

            if prose.is_empty()
                && !pending_para
                && i + 1 < lines.len()
                && is_setext_title(line.text)
                && is_setext_underline(lines[i + 1].text)
                && !closes_open(lines[i + 1].text, &stack)
            {
                regions.push(SpannedRegion::structure(input, line.span()));
                regions.push(SpannedRegion::structure(input, lines[i + 1].span()));
                pending_source = None;
                i += 2;
                continue;
            }

            if let Some(delim) = delimiter(line.text) {
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                pending_para = false;
                open_block(
                    input,
                    &line,
                    i,
                    delim,
                    &mut pending_source,
                    &mut stack,
                    &mut regions,
                );
                literal = false;
                i += 1;
                continue;
            }

            if plus_continuation(line.text) {
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if pending_para && !is_structure_line(line.text) {
                pending_para = false;
                pending_source = None;
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                while i < lines.len() {
                    let body = lines[i];
                    if body.text.trim().is_empty()
                        || delimiter(body.text).is_some()
                        || plus_continuation(body.text)
                    {
                        break;
                    }
                    regions.push(SpannedRegion::structure(input, body.span()));
                    i += 1;
                }
                continue;
            }

            if is_structure_line(line.text) {
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                if source_lang(line.text).is_some() {
                    pending_source = source_lang(line.text);
                } else if !line.text.trim().is_empty() {
                    pending_source = None;
                }
                pending_para = paragraph_style(line.text);
                regions.push(SpannedRegion::structure(input, line.span()));
                literal = false;
                i += 1;
                continue;
            }
            pending_source = None;
            pending_para = false;

            if let Some(marker_len) = item_marker_len(line.text) {
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                let marker = ByteSpan::new(line.start, line.start + marker_len);
                regions.push(SpannedRegion::structure(input, marker));
                if let Some(at) = hard_break_at(line.text) {
                    if at >= marker_len {
                        push_prose_until(&mut prose, &mut prose_span, &line, marker_len, at, false);
                        flush_prose_spanned(&mut prose, &mut prose_span, &mut regions);
                        regions.push(SpannedRegion::structure(
                            input,
                            ByteSpan::new(line.start + at, line.end),
                        ));
                        hang = Some(Hang {
                            width: marker_len,
                            empty_body: line.text[marker_len..at].trim().is_empty(),
                        });
                        term = None;
                        literal = false;
                        i += 1;
                        continue;
                    }
                }
                let body = line.text[marker_len..].trim();
                if body.is_empty() {
                    if !line.terminator_span().is_empty() {
                        regions.push(SpannedRegion::structure(input, line.terminator_span()));
                    }
                    hang = Some(Hang {
                        width: marker_len,
                        empty_body: true,
                    });
                } else {
                    hang = Some(Hang {
                        width: marker_len,
                        empty_body: false,
                    });
                    push_hang_body(
                        &mut prose,
                        &mut prose_span,
                        &mut term,
                        &line,
                        marker_len,
                        false,
                    );
                }
                literal = false;
                i += 1;
                continue;
            }

            if let Some(active) = hang {
                let indent = leading_ws(line.text);
                if indent >= active.width && indent < line.text.len() {
                    if let Some(at) = hard_break_at(line.text) {
                        if at >= indent {
                            if active.empty_body {
                                let marker = ByteSpan::new(line.start, line.start + indent);
                                regions.push(SpannedRegion::structure(input, marker));
                                hang = Some(Hang {
                                    width: indent,
                                    empty_body: false,
                                });
                                push_prose_until(
                                    &mut prose,
                                    &mut prose_span,
                                    &line,
                                    indent,
                                    at,
                                    false,
                                );
                            } else {
                                push_prose_until(&mut prose, &mut prose_span, &line, 0, at, true);
                            }
                            flush_prose_spanned(&mut prose, &mut prose_span, &mut regions);
                            regions.push(SpannedRegion::structure(
                                input,
                                ByteSpan::new(line.start + at, line.end),
                            ));
                            term = None;
                            i += 1;
                            continue;
                        }
                    }
                    if active.empty_body {
                        let marker = ByteSpan::new(line.start, line.start + indent);
                        regions.push(SpannedRegion::structure(input, marker));
                        hang = Some(Hang {
                            width: indent,
                            empty_body: false,
                        });
                        push_hang_body(
                            &mut prose,
                            &mut prose_span,
                            &mut term,
                            &line,
                            indent,
                            false,
                        );
                    } else {
                        push_hang_body(&mut prose, &mut prose_span, &mut term, &line, 0, true);
                    }
                    i += 1;
                    continue;
                }
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                continue;
            }

            if leading_ws(line.text) > 0 {
                flush_item(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut regions,
                );
                regions.push(SpannedRegion::structure(input, line.span()));
                literal = true;
                i += 1;
                continue;
            }

            if let Some(at) = hard_break_at(line.text) {
                let join = !prose.is_empty();
                push_prose_until(&mut prose, &mut prose_span, &line, 0, at, join);
                flush_prose_spanned(&mut prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(
                    input,
                    ByteSpan::new(line.start + at, line.end),
                ));
                term = None;
                i += 1;
                continue;
            }

            push_prose_line(&mut prose, &mut prose_span, &line, true, true);
            term = None;
            i += 1;
        }

        flush_item(
            input,
            &mut prose,
            &mut prose_span,
            &mut term,
            &mut hang,
            &mut regions,
        );
        emit_unclosed_code(input, &lines, &stack, &mut regions);
        regions
    }
}

fn flush_item(
    input: &str,
    prose: &mut ProseBuf,
    prose_span: &mut Option<ByteSpan>,
    term: &mut Option<ByteSpan>,
    hang: &mut Option<Hang>,
    regions: &mut Vec<SpannedRegion>,
) {
    flush_prose_spanned(prose, prose_span, regions);
    if let Some(span) = term.take() {
        if !span.is_empty() {
            regions.push(SpannedRegion::structure(input, span));
        }
    }
    *hang = None;
}

fn push_hang_body(
    prose: &mut ProseBuf,
    prose_span: &mut Option<ByteSpan>,
    term: &mut Option<ByteSpan>,
    line: &Line<'_>,
    from: usize,
    join: bool,
) {
    let piece = line.text.get(from..).unwrap_or("");
    let trimmed = piece.trim();
    if !trimmed.is_empty() {
        if !prose.is_empty() && join {
            join_prose_gap(prose);
        }
        prose.push_str(trimmed);
        let end = line.start + line.text.len();
        match prose_span {
            None => {
                let start = if from == 0 {
                    line.start
                } else {
                    line.start + from
                };
                *prose_span = Some(ByteSpan::new(start, end));
            }
            Some(span) => span.end = end,
        }
    }
    *term = Some(line.terminator_span());
}

/// Prose covering `line.text[from..until]`, without the hard-break suffix.
fn push_prose_until(
    prose: &mut ProseBuf,
    prose_span: &mut Option<ByteSpan>,
    line: &Line<'_>,
    from: usize,
    until: usize,
    join: bool,
) {
    let piece = line.text.get(from..until).unwrap_or("");
    let trimmed = piece.trim();
    if trimmed.is_empty() {
        return;
    }
    if !prose.is_empty() && join {
        join_prose_gap(prose);
    }
    prose.push_str(trimmed);
    let local = piece.find(trimmed).unwrap_or(0);
    let start = line.start + from + local;
    let end = start + trimmed.len();
    match prose_span {
        None => *prose_span = Some(ByteSpan::new(start, end)),
        Some(span) => span.end = end,
    }
}

fn open_block(
    input: &str,
    line: &Line<'_>,
    index: usize,
    delim: Delim,
    pending_source: &mut Option<Option<String>>,
    stack: &mut Vec<OpenBlock>,
    regions: &mut Vec<SpannedRegion>,
) {
    let style = style_of(line.text, delim);
    let kind = kind_of(line.text, delim, pending_source);
    let hold = matches!(kind, OpenKind::Code { .. });
    let block = OpenBlock {
        style,
        kind,
        header: line.span(),
        start_index: index,
    };
    if !hold {
        regions.push(SpannedRegion::structure(input, line.span()));
    }
    stack.push(block);
}

fn kind_of(line: &str, delim: Delim, pending_source: &mut Option<Option<String>>) -> OpenKind {
    match delim {
        Delim::Example | Delim::Sidebar | Delim::Quote | Delim::Open => OpenKind::Prose,
        Delim::Passthrough | Delim::Comment | Delim::Table => OpenKind::Frozen,
        Delim::Listing | Delim::Literal | Delim::Fence => {
            if let Some(lang) = pending_source.take() {
                OpenKind::Code { lang }
            } else if matches!(delim, Delim::Fence) {
                OpenKind::Code {
                    lang: fence_lang(line),
                }
            } else {
                OpenKind::Frozen
            }
        }
    }
}

fn close_block(
    input: &str,
    lines: &[Line<'_>],
    stack: &mut Vec<OpenBlock>,
    closer_index: usize,
    regions: &mut Vec<SpannedRegion>,
) {
    let Some(block) = stack.pop() else {
        return;
    };
    let closer = lines[closer_index];
    match block.kind {
        OpenKind::Code { lang } => {
            let body = ByteSpan::new(block.header.end, closer.start);
            regions.push(SpannedRegion::code(
                input,
                lang,
                block.header,
                body,
                closer.span(),
            ));
        }
        OpenKind::Frozen | OpenKind::Prose => {
            regions.push(SpannedRegion::structure(input, closer.span()));
        }
    }
}

fn emit_unclosed_code(
    input: &str,
    lines: &[Line<'_>],
    stack: &[OpenBlock],
    regions: &mut Vec<SpannedRegion>,
) {
    let Some(block) = stack
        .iter()
        .find(|block| matches!(block.kind, OpenKind::Code { .. }))
    else {
        return;
    };
    for line in &lines[block.start_index..] {
        regions.push(SpannedRegion::structure(input, line.span()));
    }
}

fn closes(line: &str, style: &Style) -> bool {
    let t = line.trim();
    match style {
        Style::Exact(expected) => t == *expected,
        Style::Run { ch, min } => t.chars().count() == *min && t.chars().all(|c| c == *ch),
        Style::Backtick { min } => t.chars().count() >= *min && t.chars().all(|c| c == '`'),
        Style::Table => is_table_delim(t),
    }
}

fn style_of(line: &str, delim: Delim) -> Style {
    let t = line.trim();
    match delim {
        Delim::Open => Style::Exact("--"),
        Delim::Table => Style::Table,
        Delim::Fence => Style::Backtick {
            min: t.chars().take_while(|c| *c == '`').count(),
        },
        Delim::Listing => Style::Run {
            ch: '-',
            min: t.chars().count(),
        },
        Delim::Literal => Style::Run {
            ch: '.',
            min: t.chars().count(),
        },
        Delim::Example => Style::Run {
            ch: '=',
            min: t.chars().count(),
        },
        Delim::Sidebar => Style::Run {
            ch: '*',
            min: t.chars().count(),
        },
        Delim::Quote => Style::Run {
            ch: '_',
            min: t.chars().count(),
        },
        Delim::Passthrough => Style::Run {
            ch: '+',
            min: t.chars().count(),
        },
        Delim::Comment => Style::Run {
            ch: '/',
            min: t.chars().count(),
        },
    }
}

fn delimiter(line: &str) -> Option<Delim> {
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    if t == "--" {
        return Some(Delim::Open);
    }
    if is_table_delim(t) {
        return Some(Delim::Table);
    }
    if t.chars().all(|c| c == '`') && t.chars().count() >= 3 {
        return Some(Delim::Fence);
    }
    if let Some(lang) = t.strip_prefix("```") {
        if !lang.is_empty()
            && !lang.starts_with('`')
            && lang
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '_' | '.'))
        {
            return Some(Delim::Fence);
        }
    }
    let (ch, min, kind) = match t.chars().next() {
        Some('-') => ('-', 4, Delim::Listing),
        Some('.') => ('.', 4, Delim::Literal),
        Some('=') => ('=', 4, Delim::Example),
        Some('*') => ('*', 4, Delim::Sidebar),
        Some('_') => ('_', 4, Delim::Quote),
        Some('+') => ('+', 4, Delim::Passthrough),
        Some('/') => ('/', 4, Delim::Comment),
        _ => return None,
    };
    if t.chars().all(|c| c == ch) && t.chars().count() >= min {
        Some(kind)
    } else {
        None
    }
}

fn is_table_delim(t: &str) -> bool {
    let rest = t
        .strip_prefix('|')
        .or_else(|| t.strip_prefix('!'))
        .or_else(|| t.strip_prefix(','));
    let Some(rest) = rest else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c == '=') && rest.chars().count() >= 3
}

fn fence_lang(line: &str) -> Option<String> {
    let t = line.trim();
    let ticks = t.chars().take_while(|c| *c == '`').count();
    let lang = t[ticks..].trim();
    if lang.is_empty() {
        None
    } else {
        Some(lang.to_string())
    }
}

fn source_lang(line: &str) -> Option<Option<String>> {
    let t = line.trim();
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    if inner.contains('[') || inner.contains(']') {
        return None;
    }
    let mut parts = inner.split(',');
    if parts.next()?.trim() != "source" {
        return None;
    }
    let lang = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.contains('='))
        .map(str::to_string);
    Some(lang)
}

fn plus_continuation(line: &str) -> bool {
    line.trim() == "+"
}

/// Byte index of the ` +` hard break, when the line has text before it.
fn hard_break_at(text: &str) -> Option<usize> {
    let end = text.trim_end();
    let head = end.strip_suffix(" +")?;
    if head.trim().is_empty() {
        None
    } else {
        Some(head.len())
    }
}

fn closes_open(line: &str, stack: &[OpenBlock]) -> bool {
    stack.last().is_some_and(|top| closes(line, &top.style))
}

fn is_setext_underline(text: &str) -> bool {
    let t = text.trim();
    if t.chars().count() < 2 {
        return false;
    }
    let mut chars = t.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    (ch == '=' || ch == '-') && chars.all(|c| c == ch)
}

fn is_setext_title(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || text.starts_with([' ', '\t']) {
        return false;
    }
    if delimiter(text).is_some() || is_structure_line(text) || item_marker_len(text).is_some() {
        return false;
    }
    !plus_continuation(text)
}

/// `[listing]`, `[literal]`, `[source]`, and `[verse]` freeze the next paragraph.
fn paragraph_style(line: &str) -> bool {
    let t = line.trim();
    let Some(inner) = t.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) else {
        return false;
    };
    if t.starts_with("[[") || inner.contains('[') {
        return false;
    }
    let name = inner.split(',').next().unwrap_or("").trim();
    matches!(name, "listing" | "literal" | "source" | "verse")
}

fn is_structure_line(line: &str) -> bool {
    adoc_heading_line(line)
        || attribute_line(line)
        || block_title(line)
        || comment_line(line)
        || horizontal_rule(line)
        || line.trim() == "<<<"
        || anchor_line(line)
        || attr_list(line)
        || block_macro(line)
}

/// Column-0 heading: one or more `=` then a space and a title.
pub(crate) fn adoc_heading_line(line: &str) -> bool {
    if line.starts_with([' ', '\t']) {
        return false;
    }
    let t = line.trim_end();
    let n = t.bytes().take_while(|&b| b == b'=').count();
    if n == 0 {
        return false;
    }
    let rest = &t[n..];
    rest.starts_with(' ') && !rest.trim().is_empty()
}

fn attribute_line(line: &str) -> bool {
    let t = line.trim();
    let Some(rest) = t.strip_prefix(':') else {
        return false;
    };
    if rest.starts_with(':') {
        return false;
    }
    let Some(end) = rest.find(':') else {
        return false;
    };
    if end == 0 {
        return false;
    }
    rest[..end]
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '!' | '@'))
}

fn block_title(line: &str) -> bool {
    let t = line.trim();
    let Some(rest) = t.strip_prefix('.') else {
        return false;
    };
    if rest.starts_with(['.', ' ', '\t']) {
        return false;
    }
    !rest.is_empty()
}

fn comment_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") && delimiter(line).is_none()
}

fn horizontal_rule(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && t.chars().all(|c| c == '\'')
}

fn anchor_line(line: &str) -> bool {
    let t = line.trim();
    let Some(inner) = t
        .strip_prefix("[[")
        .and_then(|rest| rest.strip_suffix("]]"))
    else {
        return false;
    };
    !inner.is_empty() && !inner.contains('[')
}

fn attr_list(line: &str) -> bool {
    let t = line.trim();
    let Some(inner) = t.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) else {
        return false;
    };
    !t.starts_with("[[") && !inner.contains('[')
}

fn block_macro(line: &str) -> bool {
    let t = line.trim();
    let Some(idx) = t.find("::") else {
        return false;
    };
    if idx == 0 {
        return false;
    }
    let name = &t[..idx];
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        && t.ends_with(']')
        && t[idx + 2..].contains('[')
}

fn adoc_pragma(line: &str) -> Option<bool> {
    let t = line.trim();
    let rest = t.strip_prefix("//")?.trim();
    match rest {
        "snapper:on" => Some(true),
        "snapper:off" => Some(false),
        _ => None,
    }
}

fn item_marker_len(line: &str) -> Option<usize> {
    adoc_list_marker_len(line).or_else(|| adoc_admonition_marker_len(line))
}

/// Byte length of a list, callout, checklist, or definition marker,
/// including the trailing space when the item has one. `None` when `line`
/// is not a marker. The length covers a leading indent.
pub(crate) fn adoc_list_marker_len(line: &str) -> Option<usize> {
    if line.contains('\n') {
        return None;
    }
    let indent = leading_ws(line);
    let rest = &line[indent..];
    let body = checklist_len(rest)
        .or_else(|| numbered_len(rest))
        .or_else(|| bullet_len(rest))
        .or_else(|| callout_len(rest))
        .or_else(|| dlist_len(rest))?;
    Some(indent + body)
}

/// Byte length of `NOTE:` / `TIP:` / `IMPORTANT:` / `WARNING:` / `CAUTION:`,
/// including one trailing space when the body follows on the same line.
pub(crate) fn adoc_admonition_marker_len(line: &str) -> Option<usize> {
    if line.contains('\n') {
        return None;
    }
    let indent = leading_ws(line);
    let rest = &line[indent..];
    const NAMES: [&str; 5] = ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"];
    for name in NAMES {
        let Some(after) = rest
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix(':'))
        else {
            continue;
        };
        if after.starts_with(' ') {
            return Some(indent + name.len() + 2);
        }
        if after.is_empty() {
            return Some(indent + name.len() + 1);
        }
    }
    None
}

fn checklist_len(rest: &str) -> Option<usize> {
    let bullet = repeated(rest, '*')
        .or_else(|| repeated(rest, '-'))
        .or_else(|| repeated(rest, '.'))?;
    let after_bullet = rest[bullet..].strip_prefix(' ')?;
    let inner = after_bullet.strip_prefix('[')?;
    let (box_body, after_box) = inner.split_once(']')?;
    if !matches!(box_body, " " | "x" | "X" | "*") {
        return None;
    }
    let box_len = 1 + box_body.len() + 1;
    if after_box.starts_with(' ') {
        Some(bullet + 1 + box_len + 1)
    } else if after_box.is_empty() {
        Some(bullet + 1 + box_len)
    } else {
        None
    }
}

fn numbered_len(rest: &str) -> Option<usize> {
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let after = rest[digits..].strip_prefix('.')?;
    if after.starts_with([' ', '\t']) {
        Some(digits + 2)
    } else if after.is_empty() {
        Some(digits + 1)
    } else {
        None
    }
}

fn bullet_len(rest: &str) -> Option<usize> {
    for ch in ['*', '-', '.'] {
        let Some(n) = repeated(rest, ch) else {
            continue;
        };
        let after = &rest[n..];
        if after.starts_with([' ', '\t']) {
            return Some(n + 1);
        }
    }
    None
}

fn callout_len(rest: &str) -> Option<usize> {
    let inner = rest.strip_prefix('<')?;
    let (digits, after) = inner.split_once('>')?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let marker = 1 + digits.len() + 1;
    if after.starts_with(' ') {
        Some(marker + 1)
    } else if after.is_empty() {
        Some(marker)
    } else {
        None
    }
}

fn dlist_len(rest: &str) -> Option<usize> {
    let idx = rest.find("::")?;
    if idx == 0 {
        return None;
    }
    let term = &rest[..idx];
    if !term.chars().any(|c| c.is_ascii_alphanumeric()) || term.contains('[') {
        return None;
    }
    let after = &rest[idx + 2..];
    if after.contains('[') && rest.trim_end().ends_with(']') {
        return None;
    }
    if after.starts_with(' ') {
        Some(idx + 3)
    } else if after.is_empty() {
        Some(idx + 2)
    } else {
        None
    }
}

fn repeated(rest: &str, ch: char) -> Option<usize> {
    let n = rest.chars().take_while(|&c| c == ch).count();
    if n == 0 { None } else { Some(n) }
}

fn leading_ws(line: &str) -> usize {
    line.bytes()
        .take_while(|&b| b == b' ' || b == b'\t')
        .count()
}

/// True when a wrapped line would be parsed as a new AsciiDoc block.
pub(crate) fn adoc_line_opens_block(line: &str) -> bool {
    if line.trim().is_empty() {
        return false;
    }
    delimiter(line).is_some()
        || is_structure_line(line)
        || adoc_list_marker_len(line).is_some()
        || adoc_admonition_marker_len(line).is_some()
}

/// Content signal for documents with no prose extension.
///
/// A block macro (`include::`, `image::`, `video::`, `audio::`) is enough.
/// A column-0 heading counts only together with an attribute line, so a
/// plaintext note that starts with `= ` stays plaintext.
pub(crate) fn looks_like_asciidoc(text: &str) -> bool {
    let mut heading = false;
    let mut attribute = false;
    for line in text.lines() {
        let t = line.trim();
        if (t.starts_with("include::")
            || t.starts_with("image::")
            || t.starts_with("video::")
            || t.starts_with("audio::"))
            && block_macro(t)
        {
            return true;
        }
        if adoc_heading_line(line) {
            heading = true;
        }
        if attribute_line(t) {
            attribute = true;
        }
        if heading && attribute {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Format;
    use crate::oracle;
    use crate::parser::Region;
    use crate::{FormatConfig, format_text};

    fn format_adoc(input: &str) -> String {
        let cfg = FormatConfig {
            format: Format::AsciiDoc,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap_or_else(|err| panic!("format: {err}"));
        let again = format_text(&out, &cfg).unwrap_or_else(|err| panic!("second: {err}"));
        assert_eq!(out, again, "not idempotent:\n{out}");
        assert!(
            oracle::matches(Format::AsciiDoc, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}\n in regions={:?}\n out regions={:?}",
            AsciiDocParser.parse(input),
            AsciiDocParser.parse(&out)
        );
        out
    }

    #[test]
    fn paragraph_splits_sentences() {
        let out = format_adoc("The method is stable. It converges in three steps.\n");
        assert_eq!(out, "The method is stable.\nIt converges in three steps.\n");
    }

    #[test]
    fn list_and_admonition_hang() {
        let out = format_adoc("* The baseline is slower. It uses more memory.\n");
        assert_eq!(out, "* The baseline is slower.\n  It uses more memory.\n");
        let note = format_adoc("NOTE: Keep the warning together. The next claim hangs.\n");
        assert_eq!(
            note,
            "NOTE: Keep the warning together.\n      The next claim hangs.\n"
        );
    }

    #[test]
    fn nested_marker_and_definition_hang() {
        let out = format_adoc("** Inner item. Second sentence.\n");
        assert_eq!(out, "** Inner item.\n   Second sentence.\n");
        let term = format_adoc("CPU:: The chip. It runs.\n");
        assert_eq!(term, "CPU:: The chip.\n      It runs.\n");
    }

    #[test]
    fn listing_stays_frozen_and_source_is_code() {
        let listing = "----\nKeep this. Together.\n----\n";
        assert_eq!(format_adoc(listing), listing);
        let input = "[source,rust]\n----\nfn answer() {}\n// Keep this. Together.\n----\n";
        assert_eq!(format_adoc(input), input);
        let regions = AsciiDocParser.parse(input);
        assert!(
            regions.iter().any(|region| matches!(
                region,
                Region::Code {
                    lang: Some(lang),
                    ..
                } if lang == "rust"
            )),
            "{regions:?}"
        );
    }

    #[test]
    fn example_interior_reflows_and_literal_does_not() {
        let out = format_adoc("====\nInside the example. Second sentence.\n====\n");
        assert_eq!(out, "====\nInside the example.\nSecond sentence.\n====\n");
        let literal = "See the block.\n\n  This stays. Together.\n";
        let formatted = format_adoc(literal);
        assert!(
            formatted.contains("  This stays. Together.\n"),
            "{formatted}"
        );
        assert!(formatted.contains("See the block.\n"), "{formatted}");
    }

    #[test]
    fn heading_attribute_and_macro_stay() {
        let input = "= A short note\n:author: A. Writer\n\ninclude::chapter.adoc[]\n\nHello there. Next claim.\n";
        let out = format_adoc(input);
        assert!(
            out.starts_with("= A short note\n:author: A. Writer\n"),
            "{out}"
        );
        assert!(out.contains("include::chapter.adoc[]\n"), "{out}");
        assert!(out.contains("Hello there.\nNext claim.\n"), "{out}");
    }

    #[test]
    fn pragma_off_keeps_the_fused_sentence() {
        let input = "// snapper:off\nKeep this. Together.\n// snapper:on\nSplit this. Now.\n";
        let out = format_adoc(input);
        assert!(out.contains("Keep this. Together.\n"), "{out}");
        assert!(out.contains("Split this.\nNow.\n"), "{out}");
    }

    #[test]
    fn checklist_marker_includes_the_box() {
        assert_eq!(adoc_list_marker_len("* [ ] Item"), Some("* [ ] ".len()));
        assert_eq!(adoc_list_marker_len("<1> Step"), Some("<1> ".len()));
        assert_eq!(
            adoc_admonition_marker_len("WARNING: Careful"),
            Some("WARNING: ".len())
        );
        assert_eq!(adoc_list_marker_len("1. Item"), Some("1. ".len()));
        assert_eq!(adoc_list_marker_len("12. Item"), Some("12. ".len()));
    }

    #[test]
    fn plus_continuations_and_hard_breaks_stay() {
        let cont = "* item\n+\nThis is a continuation. It is a new paragraph.\n";
        let out = format_adoc(cont);
        assert!(out.contains("\n+\nThis is a continuation.\n"), "{out}");
        assert!(!out.contains("+ This"), "{out}");
        assert!(out.contains("It is a new paragraph.\n"), "{out}");

        let hard = "Keep this. Together. +\nNext line. Second sentence.\n";
        let out = format_adoc(hard);
        assert!(out.contains("Together. +\n"), "{out}");
        assert!(out.contains("Next line.\nSecond sentence.\n"), "{out}");
    }

    #[test]
    fn styled_paragraphs_stay_frozen() {
        for style in ["listing", "literal", "source", "verse"] {
            let input =
                format!("[{style}]\nKeep this. Together.\n\nNormal paragraph. Second sentence.\n");
            let out = format_adoc(&input);
            assert!(
                out.contains(&format!("[{style}]\nKeep this. Together.\n")),
                "{style}: {out}"
            );
            assert!(
                out.contains("Normal paragraph.\nSecond sentence.\n"),
                "{style}: {out}"
            );
        }
    }

    #[test]
    fn numbered_lists_hang() {
        let out = format_adoc("1. The baseline is slower. It uses more memory.\n");
        assert_eq!(out, "1. The baseline is slower.\n   It uses more memory.\n");
        let wider = format_adoc("12. The baseline is slower. It uses more memory.\n");
        assert_eq!(
            wider,
            "12. The baseline is slower.\n    It uses more memory.\n"
        );
    }

    #[test]
    fn a_longer_run_does_not_close_a_listing() {
        let input =
            "----\nA sentence. Another sentence.\n------\nstill inside. Still one line.\n----\n";
        assert_eq!(format_adoc(input), input);
    }

    #[test]
    fn a_two_line_heading_does_not_open_a_block() {
        let input = "Document Title\n----\n\nA sentence. Another sentence.\n";
        let out = format_adoc(input);
        assert!(out.starts_with("Document Title\n----\n"), "{out}");
        assert!(out.contains("A sentence.\nAnother sentence.\n"), "{out}");

        let listing = "Title\n----\n\n----\nKeep this. Together.\n----\n";
        assert_eq!(format_adoc(listing), listing);
    }

    #[test]
    fn detection_needs_a_macro_or_a_heading_and_attribute() {
        assert!(looks_like_asciidoc("include::chapter.adoc[]\n"));
        assert!(looks_like_asciidoc(
            "= Title\n:author: A. Writer\n\nHello.\n"
        ));
        assert!(!looks_like_asciidoc("= Title\n\nHello. World.\n"));
        assert!(!looks_like_asciidoc("Hello. World.\n"));
        assert_eq!(
            Format::detect(Some(std::path::Path::new("note.adoc")), "Hello. World.\n"),
            Format::AsciiDoc
        );
        assert_eq!(
            Format::detect(
                Some(std::path::Path::new("note.txt")),
                "= Title\n:author: A.\n"
            ),
            Format::Plaintext
        );
        assert_eq!(
            Format::detect(None, "= Title\n:author: A. Writer\n\nHello. World.\n"),
            Format::AsciiDoc
        );
    }
}
