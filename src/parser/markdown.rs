use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Line, SpannedRegion, flush_prose_spanned, iter_lines, join_prose_gap,
    push_prose_line,
};

/// CommonMark 0.31.2 §4.2 ATX heading: 0–3 spaces, then 1–6 `#`, then
/// whitespace. Four spaces is indented code (ex. 80), not a heading.
static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^( {0,3}#{1,6}\s+)(.*)$").unwrap());

static FENCED_CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(`{3,}|~{3,})").unwrap());

/// Capture the language token immediately after a fence marker.
/// `lang` is `[A-Za-z0-9_+.-]+`; anything past it (info string) is ignored.
static FENCED_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:`{3,}|~{3,})\s*([A-Za-z0-9_+.\-]+)").unwrap());

/// CommonMark list marker: 0–3 spaces, then `-`/`*`/`+` or `1.`/`1)`, then a space.
/// Four or more spaces is indented code, not a list (spec 0.31.2 ex. 289).
static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^( {0,3}(?:[-*+]|\d+[.)]) )(.*)$").unwrap());

/// List-looking line at any indent (including 4+ spaces). LIST_ITEM_RE is
/// 0–3 only; a 4-space dash is indented code, but after a blank we still
/// need the shape so hang-relative close can hand it to snapper-tupp.
static LIST_LOOKING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\t ]*(?:[-*+]|\d+[.)]) ").unwrap());

/// Markdown blockquote prefix: optional indent plus one or more `>`
/// each followed by an optional space (CommonMark 0.31.2 ex. 229).
/// Nested `>>text` / `> > text` keeps the full prefix so reflow can
/// repeat it.
static QUOTE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*(?:> ?)+)(.*)$").unwrap());

/// Match a markdown table row: line whose trimmed form starts and ends with `|`.
/// Also matches separator rows like `|---|---|`.
/// GFM 4.10 makes leading/trailing pipes optional; those rows are
/// classified by [`gfm_table_end`] when a delimiter row is present.
static TABLE_ROW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\|.*\|\s*$").unwrap());

/// CommonMark setext underline: one or more `=` (level 1) or `-` (level 2),
/// optional leading indent up to three spaces, optional trailing spaces.
static SETEXT_UNDERLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ {0,3}(?:=+|-+)\s*$").unwrap());

/// Type 1 open: `<pre` / `<script` / `<style` / `<textarea` then space, tab, `>`, or EOL.
static HTML_TYPE1_OPEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^<(?:pre|script|style|textarea)(?:[ \t>]|$)").unwrap());

/// Type 1 close: any of the type-1 end tags; need not match the opener.
static HTML_TYPE1_CLOSE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</(?:pre|script|style|textarea)>").unwrap());

/// Type 7: a complete open or closing tag, then only whitespace.
static HTML_TYPE7_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)^
        (?:
          </[A-Za-z][A-Za-z0-9-]* \s*>
          |
          <[A-Za-z][A-Za-z0-9-]*
            (?:\s+[A-Za-z_:][A-Za-z0-9_.:-]*
              (?:\s*=\s*(?:[^\s"'=<>`]+|'[^']*'|"[^"]*"))?
            )*
            \s*/?>
        )
        \s*$"#,
    )
    .unwrap()
});

/// CommonMark type-6 block tags (case-insensitive).
static HTML_BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

pub struct MarkdownParser;

/// Close an open list item: flush accumulated prose and emit the trailing newline.
fn close_list_item(
    in_list_item: &mut bool,
    list_hang: &mut Option<usize>,
    current_prose: &mut String,
    prose_span: &mut Option<ByteSpan>,
    list_term: &mut Option<ByteSpan>,
    input: &str,
    regions: &mut Vec<SpannedRegion>,
) {
    if *in_list_item {
        flush_prose_spanned(current_prose, prose_span, regions);
        if let Some(span) = list_term.take() {
            if !span.is_empty() {
                regions.push(SpannedRegion::structure(input, span));
            }
        }
        *in_list_item = false;
        *list_hang = None;
    }
}

/// Indent of the next non-blank line after `start`, if any.
fn next_nonblank_indent(lines: &[Line<'_>], start: usize) -> Option<usize> {
    lines[start..]
        .iter()
        .find(|l| !l.text.trim().is_empty())
        .map(|l| line_indent(l.text))
}

fn starts_html_comment(line: &str) -> bool {
    line.trim_start().starts_with("<!--")
}

fn html_comment_closed(text: &str) -> bool {
    match text.find("<!--") {
        Some(i) => text[i + 4..].contains("-->"),
        None => text.contains("-->"),
    }
}

/// CommonMark 4.6 HTML block types 1 and 3–7 (type 2 is `<!--`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HtmlBlock {
    /// `<pre` / `<script` / `<style` / `<textarea` until the matching end tag.
    /// Emitted as Code so the body is not sentence-split.
    Type1,
    /// `<?` until `?>`.
    Type3,
    /// `<!` + ASCII letter until `>`.
    Type4,
    /// `<![CDATA[` until `]]>`.
    Type5,
    /// Block tag (`<div`, `</p`, …) until a following blank line. May interrupt.
    Type6,
    /// Complete open/close tag until a following blank line. Must not interrupt.
    Type7,
}

impl HtmlBlock {
    fn can_interrupt(self) -> bool {
        !matches!(self, HtmlBlock::Type7)
    }
}

/// Strip at most three leading spaces (CommonMark HTML-block indent).
fn html_block_rest(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && i < 3 && bytes[i] == b' ' {
        i += 1;
    }
    &line[i..]
}

fn is_html_block_tag(name: &str) -> bool {
    HTML_BLOCK_TAGS.iter().any(|t| name.eq_ignore_ascii_case(t))
}

fn type6_start(rest: &str) -> bool {
    let after = if let Some(a) = rest.strip_prefix("</") {
        a
    } else if let Some(a) = rest.strip_prefix('<') {
        a
    } else {
        return false;
    };
    let tag_len = after
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(after.len());
    if tag_len == 0 || !is_html_block_tag(&after[..tag_len]) {
        return false;
    }
    let after_tag = &after[tag_len..];
    after_tag.is_empty()
        || after_tag.starts_with(' ')
        || after_tag.starts_with('\t')
        || after_tag.starts_with('>')
        || after_tag.starts_with("/>")
}

fn type7_start(rest: &str) -> bool {
    if !HTML_TYPE7_RE.is_match(rest) {
        return false;
    }
    let name = rest
        .trim_start_matches('<')
        .trim_start_matches('/')
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .next()
        .unwrap_or("");
    !name.eq_ignore_ascii_case("script")
        && !name.eq_ignore_ascii_case("style")
        && !name.eq_ignore_ascii_case("pre")
        && !name.eq_ignore_ascii_case("textarea")
}

fn html_block_kind(line: &str) -> Option<HtmlBlock> {
    let rest = html_block_rest(line);
    if HTML_TYPE1_OPEN_RE.is_match(rest) {
        return Some(HtmlBlock::Type1);
    }
    if rest.starts_with("<?") {
        return Some(HtmlBlock::Type3);
    }
    if rest.starts_with("<![CDATA[") {
        return Some(HtmlBlock::Type5);
    }
    if rest.starts_with("<!") && rest.len() > 2 && rest.as_bytes()[2].is_ascii_alphabetic() {
        return Some(HtmlBlock::Type4);
    }
    if type6_start(rest) {
        return Some(HtmlBlock::Type6);
    }
    if type7_start(rest) {
        return Some(HtmlBlock::Type7);
    }
    None
}

fn html_block_line_ends(line: &str, kind: HtmlBlock) -> bool {
    match kind {
        HtmlBlock::Type1 => HTML_TYPE1_CLOSE_RE.is_match(line),
        HtmlBlock::Type3 => line.contains("?>"),
        HtmlBlock::Type4 => line.contains('>'),
        HtmlBlock::Type5 => line.contains("]]>"),
        HtmlBlock::Type6 | HtmlBlock::Type7 => false,
    }
}

fn html_block_end_idx(kind: HtmlBlock, lines: &[Line<'_>], start_idx: usize) -> usize {
    match kind {
        HtmlBlock::Type6 | HtmlBlock::Type7 => {
            let mut j = start_idx;
            while j + 1 < lines.len() && !lines[j + 1].text.trim().is_empty() {
                j += 1;
            }
            j
        }
        _ => {
            if html_block_line_ends(lines[start_idx].text, kind) {
                return start_idx;
            }
            let mut j = start_idx + 1;
            while j < lines.len() {
                if html_block_line_ends(lines[j].text, kind) {
                    return j;
                }
                j += 1;
            }
            lines.len().saturating_sub(1)
        }
    }
}

/// Consume a CommonMark HTML block starting at `start_idx`. Returns the next index.
fn emit_html_block(
    kind: HtmlBlock,
    lines: &[Line<'_>],
    start_idx: usize,
    input: &str,
    regions: &mut Vec<SpannedRegion>,
) -> usize {
    let end_idx = html_block_end_idx(kind, lines, start_idx);
    if kind == HtmlBlock::Type1 {
        let header = lines[start_idx].span();
        if end_idx == start_idx {
            let empty = ByteSpan::new(lines[start_idx].end, lines[start_idx].end);
            regions.push(SpannedRegion::code(input, None, header, empty, empty));
        } else {
            let body = ByteSpan::new(lines[start_idx].end, lines[end_idx].start);
            let footer = lines[end_idx].span();
            regions.push(SpannedRegion::code(input, None, header, body, footer));
        }
    } else {
        let start = lines[start_idx].start;
        let end = lines[end_idx].end;
        regions.push(SpannedRegion::structure(input, ByteSpan::new(start, end)));
    }
    end_idx + 1
}

/// Two or more trailing spaces, or an unescaped trailing backslash.
/// Returns `(content_len, hard_at)` relative to `text`.
fn hard_break_rel(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    if !bytes.is_empty() && *bytes.last().unwrap() == b'\\' {
        let mut n = 0usize;
        let mut i = bytes.len();
        while i > 0 && bytes[i - 1] == b'\\' {
            n += 1;
            i -= 1;
        }
        if n % 2 == 1 {
            return Some((text.len() - 1, text.len() - 1));
        }
        return None;
    }
    let stripped = text.trim_end_matches(' ');
    if text.len() - stripped.len() >= 2 {
        return Some((stripped.len(), stripped.len()));
    }
    None
}

struct ProseAcc<'a> {
    text: &'a mut String,
    span: &'a mut Option<ByteSpan>,
    term: &'a mut Option<ByteSpan>,
}

/// Append `line.text[piece_from..]` to the running prose buffer.
///
/// A hard break flushes prose and emits the break (spaces or `\`, plus the
/// line terminator) as Structure so splice copies those source bytes.
fn append_piece(
    acc: &mut ProseAcc<'_>,
    line: &Line<'_>,
    piece_from: usize,
    join_space: bool,
    include_term_if_soft: bool,
    input: &str,
    regions: &mut Vec<SpannedRegion>,
) {
    let piece = &line.text[piece_from..];
    if let Some((content_end, hard_at)) = hard_break_rel(piece) {
        let raw = &piece[..content_end];
        let trimmed = raw.trim_start();
        let left = raw.len() - trimmed.len();
        if !trimmed.is_empty() {
            if !acc.text.is_empty() && join_space {
                join_prose_gap(acc.text);
            }
            acc.text.push_str(trimmed);
            let start = line.start + piece_from + left;
            let end = line.start + piece_from + content_end;
            match acc.span {
                None => *acc.span = Some(ByteSpan::new(start, end)),
                Some(s) => s.end = end,
            }
        }
        flush_prose_spanned(acc.text, acc.span, regions);
        let hard = ByteSpan::new(line.start + piece_from + hard_at, line.end);
        if !hard.is_empty() {
            regions.push(SpannedRegion::structure(input, hard));
        }
        *acc.term = None;
        return;
    }
    if piece_from == 0 {
        push_prose_line(acc.text, acc.span, line, join_space, include_term_if_soft);
        *acc.term = if include_term_if_soft {
            None
        } else {
            Some(line.terminator_span())
        };
        return;
    }
    if !piece.is_empty() {
        if !acc.text.is_empty() && join_space {
            join_prose_gap(acc.text);
        }
        acc.text.push_str(piece);
        let start = line.start + piece_from;
        let end = line.start + line.text.len();
        match acc.span {
            None => *acc.span = Some(ByteSpan::new(start, end)),
            Some(s) => s.end = end,
        }
    }
    *acc.term = Some(line.terminator_span());
}

/// True when `line` is a CommonMark setext underline (`===` or `---`).
fn is_setext_underline(line: &str) -> bool {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return false;
    }
    SETEXT_UNDERLINE_RE.is_match(trimmed)
}

/// CommonMark 0.31.2 §4.1 thematic break: 0–3 spaces of indentation,
/// then three or more matching `-` / `*` / `_`, each optionally followed
/// by spaces or tabs. A leading tab is indent ≥ 4, so not a break.
fn is_thematic_break(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && i < 3 && bytes[i] == b' ' {
        i += 1;
    }
    if i >= bytes.len() {
        return false;
    }
    let marker = bytes[i];
    if !matches!(marker, b'-' | b'*' | b'_') {
        return false;
    }
    let mut count = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == marker {
            count += 1;
            i += 1;
        } else if b == b' ' || b == b'\t' {
            i += 1;
        } else {
            return false;
        }
    }
    count >= 3
}

/// Leading whitespace width in bytes (`trim_start` prefix).
fn line_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// CommonMark 4.4 indented-code line: a tab, or at least four spaces, then
/// non-whitespace. Blank lines are not openers; they end or continue a block.
fn is_indented_code_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    let prefix = &line[..line.len() - trimmed.len()];
    prefix.contains('\t') || prefix.len() >= 4
}

/// Count CommonMark blockquote markers at the start of `line`.
/// Each marker is `>` plus an optional space. Leading whitespace is skipped.
fn quote_marker_depth(line: &str) -> usize {
    let mut rest = line.trim_start();
    let mut depth = 0;
    while let Some(after) = rest.strip_prefix('>') {
        depth += 1;
        rest = after.strip_prefix(' ').unwrap_or(after);
    }
    depth
}

/// Strip exactly `depth` blockquote markers (`>` plus optional space).
/// Leading whitespace is skipped once, matching [`quote_marker_depth`].
fn strip_quote_markers(line: &str, depth: usize) -> Option<&str> {
    if depth == 0 {
        return Some(line);
    }
    let mut rest = line.trim_start();
    for _ in 0..depth {
        rest = rest.strip_prefix('>')?;
        rest = rest.strip_prefix(' ').unwrap_or(rest);
    }
    Some(rest)
}

/// Closing fence: same marker char, length at least the opener, indent at
/// most `max(3, opener_indent)`. CommonMark allows 0–3 spaces on a closer;
/// list-nested openers keep their own indent so a matching 4-space closer
/// still ends the block. Deeper inner fences stay content.
fn is_closing_fence(line: &str, fence_marker: &str, opener_indent: usize) -> bool {
    if line_indent(line) > opener_indent.max(3) {
        return false;
    }
    let Some(caps) = FENCED_CODE_RE.captures(line.trim_start()) else {
        return false;
    };
    let marker = caps.get(1).unwrap().as_str();
    marker.chars().next() == fence_marker.chars().next() && marker.len() >= fence_marker.len()
}

/// True when `s` contains an unescaped `|` (GFM table cell separator).
fn has_unescaped_pipe(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'|' {
            return true;
        }
        i += 1;
    }
    false
}

/// True when `s` ends in an unescaped `|` (even number of preceding backslashes).
fn trailing_unescaped_pipe(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.last() != Some(&b'|') {
        return false;
    }
    let mut n = 0usize;
    let mut i = bytes.len() - 1;
    while i > 0 && bytes[i - 1] == b'\\' {
        n += 1;
        i -= 1;
    }
    n % 2 == 0
}

/// Split `s` on unescaped `|`. A trailing leftover is always included.
fn split_unescaped_pipes(s: &str) -> Vec<&str> {
    let mut cells = Vec::new();
    let mut start = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'|' {
            cells.push(&s[start..i]);
            start = i + 1;
        }
        i += 1;
    }
    cells.push(&s[start..]);
    cells
}

/// GFM 4.10 delimiter cell: optional colon, one or more hyphens, optional colon.
fn is_gfm_delimiter_cell(cell: &str) -> bool {
    let c = cell.trim();
    if c.is_empty() {
        return false;
    }
    let bytes = c.as_bytes();
    let mut i = 0;
    if bytes[i] == b':' {
        i += 1;
    }
    let dash_start = i;
    while i < bytes.len() && bytes[i] == b'-' {
        i += 1;
    }
    if i == dash_start {
        return false;
    }
    if i < bytes.len() && bytes[i] == b':' {
        i += 1;
    }
    i == bytes.len()
}

/// Cells of a GFM table line. Leading and trailing pipes are optional.
/// Indent of four or more spaces (or a leading tab) is indented code, not a table.
fn gfm_table_cells(line: &str) -> Option<Vec<&str>> {
    if line.trim().is_empty() || is_indented_code_line(line) {
        return None;
    }
    if line_indent(line) > 3 || !has_unescaped_pipe(line) {
        return None;
    }
    let trimmed = line.trim();
    let mut inner = trimmed;
    if inner.starts_with('|') {
        inner = &inner[1..];
    }
    if trailing_unescaped_pipe(inner) {
        inner = &inner[..inner.len() - 1];
    }
    let cells = split_unescaped_pipes(inner);
    if cells.is_empty() {
        return None;
    }
    Some(cells)
}

fn is_gfm_table_row(line: &str) -> bool {
    gfm_table_cells(line).is_some()
}

/// Last line of a GFM table starting at `start` (header), if the next line
/// is a delimiter with a matching cell count. Data rows may omit flanking
/// pipes (GFM 4.10 / pulldown `ENABLE_TABLES`, ex. 199).
fn gfm_table_end(lines: &[Line<'_>], start: usize) -> Option<usize> {
    let header = gfm_table_cells(lines[start].text)?;
    let delim = lines.get(start + 1)?;
    let dcells = gfm_table_cells(delim.text)?;
    if header.len() != dcells.len() || !dcells.iter().all(|c| is_gfm_delimiter_cell(c)) {
        return None;
    }
    let mut end = start + 1;
    for (j, line) in lines.iter().enumerate().skip(start + 2) {
        if !is_gfm_table_row(line.text) {
            break;
        }
        end = j;
    }
    Some(end)
}

/// True when `line` may be the text of a setext heading (non-empty, not an ATX
/// marker line, not a table row, not a list item, not a fence opener).
fn is_setext_title_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if HEADING_RE.is_match(line) {
        return false;
    }
    if TABLE_ROW_RE.is_match(line) {
        return false;
    }
    if LIST_ITEM_RE.is_match(line) || QUOTE_RE.is_match(line) {
        return false;
    }
    if is_thematic_break(line) {
        return false;
    }
    if FENCED_CODE_RE.is_match(line.trim_start()) {
        return false;
    }
    // Pandoc / academic MD display math is not a setext title.
    if line.trim().starts_with("$$") {
        return false;
    }
    true
}

/// Pandoc / academic Markdown display math: a line that starts with `$$`.
fn display_math_open(line: &str) -> bool {
    line.trim().starts_with("$$")
}

/// A line that is only `$$` is an opener, not a one-line `$$...$$` block.
fn display_math_is_single_line(line: &str) -> bool {
    let t = line.trim();
    t != "$$" && t.ends_with("$$")
}

fn is_display_math_close(line: &str) -> bool {
    line.trim_end().ends_with("$$")
}

impl FormatParser for MarkdownParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let mut regions: Vec<SpannedRegion> = Vec::new();
        let mut current_prose = String::new();
        let mut prose_span: Option<ByteSpan> = None;
        let mut in_fenced_code = false;
        let mut fence_marker = String::new();
        let mut fence_indent = 0usize;
        let mut fence_quote_depth = 0usize;
        let mut code_header = ByteSpan::default();
        let mut code_body_start = 0usize;
        let mut code_lang: Option<String> = None;
        let mut in_frontmatter = false;
        let mut frontmatter_fence = String::new();
        let mut in_list_item = false;
        let mut list_hang: Option<usize> = None;
        let mut list_after_blank = false;
        let mut list_term: Option<ByteSpan> = None;
        let mut in_display_math = false;
        let mut pragma_off = false;

        let lines = iter_lines(input);
        let total = lines.len();
        let mut i = 0;

        while i < total {
            let line: &Line<'_> = &lines[i];
            let line_text = line.text;
            let line_number = i + 1;

            // Check for snapper:off/on pragmas. Inside a fenced code block,
            // the markdown parser does NOT short-circuit on pragmas; the
            // code-block reflow handles them per-language (the markers
            // `#`, `//`, `--`, `;` are all valid pragma prefixes inside
            // their respective languages).
            if !in_fenced_code {
                if let Some(on) = super::check_pragma(line_text) {
                    close_list_item(
                        &mut in_list_item,
                        &mut list_hang,
                        &mut current_prose,
                        &mut prose_span,
                        &mut list_term,
                        input,
                        &mut regions,
                    );
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    pragma_off = !on;
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }

                if pragma_off {
                    close_list_item(
                        &mut in_list_item,
                        &mut list_hang,
                        &mut current_prose,
                        &mut prose_span,
                        &mut list_term,
                        input,
                        &mut regions,
                    );
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
            }

            // Front matter detection (only at start of file)
            if line_number == 1 && (line_text.trim() == "---" || line_text.trim() == "+++") {
                in_frontmatter = true;
                frontmatter_fence = line_text.trim().to_string();
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if in_frontmatter {
                if line_text.trim() == frontmatter_fence {
                    in_frontmatter = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // Inside fenced code block
            if in_fenced_code {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                // Quoted openers close on the same `>` depth (space optional).
                // Missing prefix falls back to the raw line (unquoted closer).
                let closer_src =
                    strip_quote_markers(line_text, fence_quote_depth).unwrap_or(line_text);
                if is_closing_fence(closer_src, &fence_marker, fence_indent) {
                    in_fenced_code = false;
                    fence_quote_depth = 0;
                    regions.push(SpannedRegion::code(
                        input,
                        code_lang.take(),
                        code_header,
                        ByteSpan::new(code_body_start, line.start),
                        line.span(),
                    ));
                }
                i += 1;
                continue;
            }

            // Inside display math $$...$$ -- everything is structure
            if in_display_math {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if is_display_math_close(line_text) {
                    in_display_math = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // Fenced code block start. `>` (space optional) then ``` / ~~~
            // opens Code inside the quote; FENCED_CODE_RE on the raw line
            // would miss `> ``` and leave inner lines as sentence-split Prose.
            let quote_depth = quote_marker_depth(line_text);
            let fence_src = strip_quote_markers(line_text, quote_depth).unwrap_or(line_text);
            if let Some(caps) = FENCED_CODE_RE.captures(fence_src.trim_start()) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                fence_marker = caps.get(1).unwrap().as_str().to_string();
                fence_indent = line_indent(fence_src);
                fence_quote_depth = quote_depth;
                in_fenced_code = true;
                code_lang = FENCED_LANG_RE
                    .captures(fence_src.trim_start())
                    .map(|c| c.get(1).unwrap().as_str().to_string());
                code_header = line.span();
                code_body_start = line.end;
                i += 1;
                continue;
            }

            // CommonMark 4.4 indented code: after a blank (or any flush
            // boundary), 4 spaces or a tab is Code through the blank that
            // ends the block. Cannot interrupt a paragraph or list item.
            // Fences above still win so `    ```lang` stays a nested fence.
            //
            // After a blank inside an open list, close only when the next
            // line is list-looking (even at 4 spaces) or indented hang+4
            // (code inside the item). Then the tupp arm below emits Code.
            // Hang-width continuation (`10. ` + 4 spaces) stays in the item
            // (snapper-cbxn / CommonMark 5.2). Do not change the tupp arm.
            if list_after_blank && is_indented_code_line(line_text) {
                if let Some(hang) = list_hang {
                    let leading = line_indent(line_text);
                    if LIST_LOOKING_RE.is_match(line_text) || leading >= hang + 4 {
                        close_list_item(
                            &mut in_list_item,
                            &mut list_hang,
                            &mut current_prose,
                            &mut prose_span,
                            &mut list_term,
                            input,
                            &mut regions,
                        );
                        list_after_blank = false;
                    }
                }
            }
            if current_prose.is_empty() && !in_list_item && is_indented_code_line(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let header = ByteSpan::new(line.start, line.start);
                let body_start = line.start;
                let mut body_end = line.end;
                let mut footer = ByteSpan::new(line.end, line.end);
                i += 1;
                while i < total {
                    let nxt = &lines[i];
                    if is_indented_code_line(nxt.text) {
                        body_end = nxt.end;
                        footer = ByteSpan::new(nxt.end, nxt.end);
                        i += 1;
                        continue;
                    }
                    if nxt.text.trim().is_empty() {
                        let mut j = i + 1;
                        while j < total && lines[j].text.trim().is_empty() {
                            j += 1;
                        }
                        if j < total && is_indented_code_line(lines[j].text) {
                            body_end = nxt.end;
                            footer = ByteSpan::new(nxt.end, nxt.end);
                            i += 1;
                            continue;
                        }
                        footer = nxt.span();
                        i += 1;
                        break;
                    }
                    break;
                }
                regions.push(SpannedRegion::code(
                    input,
                    None,
                    header,
                    ByteSpan::new(body_start, body_end),
                    footer,
                ));
                continue;
            }

            // Display math open (`$$` or one-line `$$...$$`).
            if display_math_open(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if !display_math_is_single_line(line_text) {
                    in_display_math = true;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // Blank line. CommonMark: a blank does not close the item when
            // the next non-blank is indented to the list hang (ex. 256).
            if line_text.trim().is_empty() {
                let stay_in_item = in_list_item
                    && list_hang.is_some_and(|hang| {
                        next_nonblank_indent(&lines, i + 1).is_some_and(|ind| ind >= hang)
                    });
                if stay_in_item {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    if let Some(span) = list_term.take() {
                        if !span.is_empty() {
                            regions.push(SpannedRegion::structure(input, span));
                        }
                    }
                    regions.push(SpannedRegion::blank(input, line.span()));
                    list_after_blank = true;
                    i += 1;
                    continue;
                }
                list_after_blank = false;
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::blank(input, line.span()));
                i += 1;
                continue;
            }

            // Heading — keep the entire ATX line as Structure.
            // Splitting into Structure("### ") + Prose(title) let the sentence
            // reflow engine break titles after "1." or mid-phrase, producing
            // orphan headings like:
            //   ### 1.
            //   `cargo binstall` (preferred binary install)
            // CommonMark ATX headings are single-line; do not reflow them.
            // 0–3 space indent is still a heading (CM 0.31.2 §4.2).
            if HEADING_RE.is_match(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // Setext heading: title line + underline of `=` or `-`.
            // Without this, title text is Prose and the underline is glued on
            // (or mid-title periods reflow), collapsing the heading.
            if i + 1 < total
                && is_setext_title_line(line_text)
                && is_setext_underline(lines[i + 1].text)
            {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                regions.push(SpannedRegion::structure(input, lines[i + 1].span()));
                i += 2;
                continue;
            }

            // CommonMark 4.1 thematic break. After setext so Foo\n--- stays
            // a heading; Foo\n\n--- is paragraph + HR. Before LIST_ITEM_RE
            // so `* * *` / `- - -` are not lists.
            if is_thematic_break(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // GFM table: header + delimiter (leading/trailing pipes optional).
            // Pipe-less rows are Structure only when a separator is present.
            if i + 1 < total {
                if let Some(end) = gfm_table_end(&lines, i) {
                    close_list_item(
                        &mut in_list_item,
                        &mut list_hang,
                        &mut current_prose,
                        &mut prose_span,
                        &mut list_term,
                        input,
                        &mut regions,
                    );
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    for row in &lines[i..=end] {
                        regions.push(SpannedRegion::structure(input, row.span()));
                    }
                    i = end + 1;
                    continue;
                }
            }

            // Table row (pipe-delimited, flanking pipes required)
            if TABLE_ROW_RE.is_match(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            // HTML comment block (not a snapper pragma — those are handled above).
            if starts_html_comment(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if html_comment_closed(line_text) {
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
                let start = line.start;
                i += 1;
                while i < total {
                    let done = lines[i].text.contains("-->");
                    i += 1;
                    if done {
                        break;
                    }
                }
                let end = lines
                    .get(i.saturating_sub(1))
                    .map(|l| l.end)
                    .unwrap_or(input.len());
                regions.push(SpannedRegion::structure(input, ByteSpan::new(start, end)));
                continue;
            }

            // CommonMark 4.6 HTML blocks types 1 and 3–7. Type 2 is above.
            // Type 7 cannot interrupt a paragraph (open prose / list item).
            if let Some(kind) = html_block_kind(line_text) {
                let in_paragraph = !current_prose.is_empty() || in_list_item;
                if kind.can_interrupt() || !in_paragraph {
                    close_list_item(
                        &mut in_list_item,
                        &mut list_hang,
                        &mut current_prose,
                        &mut prose_span,
                        &mut list_term,
                        input,
                        &mut regions,
                    );
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    i = emit_html_block(kind, &lines, i, input, &mut regions);
                    continue;
                }
            }

            // Blockquote: emit the full `>` / `> ` / `>>` / `> > ` prefix
            // as Structure. Space after each `>` is optional (ex. 229).
            // Checked before list items so nested `>>` is not flattened.
            // Each source quote line is its own item so splice ranges stay
            // contiguous. A hard break is Structure; the next line supplies
            // its own `>` (no pre-emitted resume marker). Lazy lines
            // without `>` stay in the open item (`in_list_item`) so
            // hanging_prefix repeats the marker (ex. 228).
            if let Some(caps) = QUOTE_RE.captures(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let text = caps.get(2).unwrap().as_str();
                if text.trim().is_empty() {
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
                if HEADING_RE.is_match(text)
                    || TABLE_ROW_RE.is_match(text)
                    || FENCED_CODE_RE.is_match(text.trim_start())
                    || is_thematic_break(text)
                {
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
                let marker_span = ByteSpan::new(line.start, line.start + marker.len());
                regions.push(SpannedRegion::structure(input, marker_span));
                in_list_item = true;
                append_piece(
                    &mut ProseAcc {
                        text: &mut current_prose,
                        span: &mut prose_span,
                        term: &mut list_term,
                    },
                    line,
                    marker.len(),
                    false,
                    false,
                    input,
                    &mut regions,
                );
                i += 1;
                continue;
            }

            // List item: emit marker as Structure, start accumulating text as prose.
            // Continuation lines are appended until a block boundary.
            if let Some(caps) = LIST_ITEM_RE.captures(line_text) {
                close_list_item(
                    &mut in_list_item,
                    &mut list_hang,
                    &mut current_prose,
                    &mut prose_span,
                    &mut list_term,
                    input,
                    &mut regions,
                );
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let marker_span = ByteSpan::new(line.start, line.start + marker.len());
                regions.push(SpannedRegion::structure(input, marker_span));
                in_list_item = true;
                list_hang = Some(marker.len());
                list_after_blank = false;
                append_piece(
                    &mut ProseAcc {
                        text: &mut current_prose,
                        span: &mut prose_span,
                        term: &mut list_term,
                    },
                    line,
                    marker.len(),
                    false,
                    false,
                    input,
                    &mut regions,
                );
                i += 1;
                continue;
            }

            // Regular prose (also serves as list-item continuation when in_list_item)
            if in_list_item {
                // After a blank, hang spaces stay Structure so splice
                // does not outdent the continuation paragraph.
                if list_after_blank {
                    list_after_blank = false;
                    if let Some(hang) = list_hang {
                        if line_indent(line_text) >= hang {
                            let hang_span = ByteSpan::new(line.start, line.start + hang);
                            regions.push(SpannedRegion::structure(input, hang_span));
                            append_piece(
                                &mut ProseAcc {
                                    text: &mut current_prose,
                                    span: &mut prose_span,
                                    term: &mut list_term,
                                },
                                line,
                                hang,
                                false,
                                false,
                                input,
                                &mut regions,
                            );
                            i += 1;
                            continue;
                        }
                    }
                }
                append_piece(
                    &mut ProseAcc {
                        text: &mut current_prose,
                        span: &mut prose_span,
                        term: &mut list_term,
                    },
                    line,
                    0,
                    true,
                    false,
                    input,
                    &mut regions,
                );
            } else {
                append_piece(
                    &mut ProseAcc {
                        text: &mut current_prose,
                        span: &mut prose_span,
                        term: &mut list_term,
                    },
                    line,
                    0,
                    true,
                    true,
                    input,
                    &mut regions,
                );
            }
            i += 1;
        }

        close_list_item(
            &mut in_list_item,
            &mut list_hang,
            &mut current_prose,
            &mut prose_span,
            &mut list_term,
            input,
            &mut regions,
        );
        flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
        // Unclosed fence at EOF: emit a code region with empty footer.
        if in_fenced_code {
            let eof = ByteSpan::new(input.len(), input.len());
            regions.push(SpannedRegion::code(
                input,
                code_lang.take(),
                code_header,
                ByteSpan::new(code_body_start, input.len()),
                eof,
            ));
        }
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Region;

    #[test]
    fn simple_prose() {
        let input = "Hello world. This is a test.\nAnother line here.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(
            regions,
            vec![Region::Prose(
                "Hello world. This is a test.\nAnother line here.".to_string()
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
    fn indented_inner_fence_stays_in_code_body() {
        // GitHub #48: trim_start() used to treat the indented ```python as
        // the outer closer, so print("hello") became Prose and lost indent.
        let input = concat!(
            "```{code-block} markdown\n",
            "\n",
            "    ```python\n",
            "    print(\"hello\")\n",
            "    ```\n",
            "```\n",
        );
        let regions = MarkdownParser.parse(input);
        match &regions[0] {
            Region::Code {
                header,
                body,
                footer,
                ..
            } => {
                assert_eq!(header, "```{code-block} markdown\n");
                assert_eq!(
                    body,
                    concat!(
                        "\n",
                        "    ```python\n",
                        "    print(\"hello\")\n",
                        "    ```\n"
                    )
                );
                assert_eq!(footer, "```\n");
            }
            other => panic!("expected one Code region, got {other:?}"),
        }
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("print"))),
            "indented example must not leak into Prose: {regions:?}"
        );
        assert_eq!(regions.len(), 1);
    }

    #[test]
    fn reporter_nested_indented_fence_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = concat!(
            "```{code-block} markdown\n",
            "\n",
            "    ```python\n",
            "    print(\"hello\")\n",
            "    ```\n",
            "```\n",
        );
        let mut cfg = FormatConfig {
            format: Format::Markdown,
            max_width: 0,
            ..Default::default()
        };
        assert_eq!(format_text(input, &cfg).unwrap(), input);
        cfg = cfg.without_safety_backstops();
        assert_eq!(format_text(input, &cfg).unwrap(), input);
    }

    #[test]
    fn list_nested_fence_still_closes_at_opener_indent() {
        let input = concat!(
            "- item:\n",
            "\n",
            "    ```rust\n",
            "    fn x() {}\n",
            "    ```\n",
        );
        let regions = MarkdownParser.parse(input);
        let code = regions.iter().find(|r| matches!(r, Region::Code { .. }));
        match code {
            Some(Region::Code {
                lang,
                header,
                body,
                footer,
            }) => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert_eq!(header, "    ```rust\n");
                assert_eq!(body, "    fn x() {}\n");
                assert_eq!(footer, "    ```\n");
            }
            other => panic!("list-nested fence must be Code, got {other:?}"),
        }
    }

    #[test]
    fn three_space_closer_still_ends_flush_fence() {
        let input = "```\ncode\n   ```\n";
        let regions = MarkdownParser.parse(input);
        match &regions[0] {
            Region::Code { body, footer, .. } => {
                assert_eq!(body, "code\n");
                assert_eq!(footer, "   ```\n");
            }
            other => panic!("expected Code closed by 3-space fence, got {other:?}"),
        }
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
                assert!(
                    s.ends_with('|') || s.ends_with("|\n"),
                    "table row must be the input slice: {s:?}"
                );
            }
        }
    }

    /// GitHub #103 / snapper-36vt: GFM 4.10 leading and trailing pipes optional.
    fn ticket_gfm_table_fixture() -> &'static str {
        concat!("Name | Note\n", "--- | ---\n", "Foo | Bar. Baz\n")
    }

    #[test]
    fn gfm_table_without_flanking_pipes_is_structure() {
        let input = ticket_gfm_table_fixture();
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().all(|r| matches!(r, Region::Structure(_))),
            "pipe-less GFM table rows must be Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Name") || p.contains("Bar"))),
            "pipe-less GFM table must not reflow as Prose, got {regions:?}"
        );
        let structure: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Structure(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            structure.iter().any(|s| s.contains("Name | Note")),
            "header must be Structure, got {regions:?}"
        );
        assert!(
            structure.iter().any(|s| s.contains("--- | ---")),
            "delimiter must be Structure, got {regions:?}"
        );
        assert!(
            structure.iter().any(|s| s.contains("Foo | Bar. Baz")),
            "body row must be Structure, got {regions:?}"
        );
    }

    #[test]
    fn gfm_table_without_flanking_pipes_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = ticket_gfm_table_fixture();
        let cfg = FormatConfig {
            format: Format::Markdown,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input, "GFM table must stay identity, got:\n{out}");
        assert!(
            out.contains("Foo | Bar. Baz"),
            "cell period must not sentence-split, got:\n{out}"
        );
        assert!(
            !out.lines().any(|l| l.trim() == "Baz"),
            "must not split Bar. Baz onto the next line, got:\n{out}"
        );
    }

    #[test]
    fn gfm_table_ex199_mixed_pipes_is_structure() {
        // GFM 4.10 example 199: leading/trailing pipes optional, alignment colons.
        let input = "| abc | defghi |\n:-: | -----------:\nbar | baz\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().all(|r| matches!(r, Region::Structure(_))),
            "GFM ex. 199 rows must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(r, Region::Prose(_))),
            "GFM ex. 199 must not reflow as Prose, got {regions:?}"
        );
    }

    #[test]
    fn pipe_in_prose_without_separator_stays_prose() {
        let input = "This is a sentence | with a pipe. Next sentence.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("with a pipe"))),
            "pipe without a delimiter row is not a table, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("with a pipe"))),
            "prose pipe must not become Structure, got {regions:?}"
        );
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
                "First line of item continuation text here.\nAnother sentence.".to_string()
            )
        );
        // No trailing newline in the source, so no terminator Structure.
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn list_item_continuation_stops_at_blank() {
        let input = "- Item one text.\ncontinuation.\n\nParagraph after.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Item one text.\ncontinuation.".to_string())
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
        assert_eq!(regions.len(), 5);
    }

    #[test]
    fn list_blank_indent_continuation_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "- Item one.\n\n  Still the same item.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "blank+indent must stay in the item, got:\n{out}"
        );
        assert!(
            out.contains("\n  Still the same item.\n"),
            "continuation must keep two-space hang, got:\n{out}"
        );
    }

    /// GitHub #102 / snapper-3hed: blank + indent stays in the item;
    /// 4-space after a blank is indented code, not a nested list.
    fn ticket_list_container_fixture() -> &'static str {
        concat!(
            "- Item one. Item two.\n",
            "\n",
            "  Still the same item. More here.\n",
            "\n",
            "Para.\n",
            "\n",
            "    - looks like a list, is indented code\n",
        )
    }

    #[test]
    fn list_container_blank_indent_stays_in_item() {
        let input = ticket_list_container_fixture();
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "- ")),
            "list marker must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "blank + 2-space hang must stay Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| { matches!(r, Region::Prose(p) if p.contains("Still the same item")) }),
            "indented continuation must stay Prose in the item, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("- looks like a list, is indented code")
            )),
            "4-space dash after blank must be Code, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("looks like a list")
            )),
            "indented-code line must not be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("- looks like")
            )),
            "4-space dash must not open a list, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Para."))),
            "unindented Para. must leave the list, got {regions:?}"
        );
    }

    #[test]
    fn list_container_fixture_keeps_hang_and_code_under_format() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let input = ticket_list_container_fixture();
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("\n  Still the same item.\n  More here.\n"),
            "blank+indent must stay in the item and hang, got:\n{out}"
        );
        assert!(
            out.contains("    - looks like a list, is indented code"),
            "4-space line must stay indented code, got:\n{out}"
        );
        assert!(
            !out.contains("\n- looks like a list"),
            "must not outdent indented code into a list, got:\n{out}"
        );
        assert!(
            out.contains("Item one.") && out.contains("Item two."),
            "item sentences still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
        assert!(
            oracle::matches(Format::Markdown, input, &out),
            "oracle must accept the fixture reflow\n in={input:?}\n out={out:?}"
        );

        let raw = format_text(input, &cfg.without_safety_backstops()).unwrap();
        assert!(
            raw.contains("\n  Still the same item."),
            "without backstops, hang must still stay, got:\n{raw}"
        );
        assert!(
            raw.contains("    - looks like a list, is indented code"),
            "without backstops, indented code must stay, got:\n{raw}"
        );
    }

    #[test]
    fn four_space_list_looking_after_blank_is_code_not_nested_list() {
        let input = "- Item one.\n\n    - looks like a list, is indented code\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("- looks like a list, is indented code")
            )),
            "4-space after blank must be Code, not a nested list, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("looks like a list")
            )),
            "4-space list-looking line must not be Prose, got {regions:?}"
        );
    }

    /// snapper-cbxn: `10. ` hang is 4; blank + 4 spaces is item continuation
    /// (CommonMark 5.2), not document-level indented code.
    #[test]
    fn wide_numbered_marker_blank_indent_stays_in_item() {
        let input = "10. Item one.\n\n    Still the same item.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "10. ")),
            "wide marker must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "    ")),
            "hang-width spaces after blank must stay Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| { matches!(r, Region::Prose(p) if p.contains("Still the same item")) }),
            "hang-width continuation must stay Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(r, Region::Code { .. })),
            "hang-width continuation must not be Code, got {regions:?}"
        );
    }

    #[test]
    fn numbered_item_four_space_continuation_stays_prose() {
        let input = "1. Item one.\n\n    Still the same item.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "1. ")),
            "marker must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
            "blank + 3-space hang must stay Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| { matches!(r, Region::Prose(p) if p.contains("Still the same item")) }),
            "4-space hang text after `1. ` must stay Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(r, Region::Code { .. })),
            "indent 4 with hang 3 must not be Code, got {regions:?}"
        );
    }

    #[test]
    fn hang_plus_four_after_blank_is_indented_code() {
        let input = "- Item one.\n\n      indented code inside the item\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("indented code inside the item")
            )),
            "hang+4 after blank must be Code, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("indented code inside the item")
            )),
            "hang+4 line must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn wide_numbered_marker_blank_indent_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "10. Item one.\n\n    Still the same item.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "hang-width continuation must stay in the item, got:\n{out}"
        );
        assert!(
            out.contains("\n    Still the same item.\n"),
            "continuation must keep four-space hang, got:\n{out}"
        );
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
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn heading_is_structure_not_prose() {
        let input = "## My Heading";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], Region::Structure("## My Heading".to_string()));
    }

    #[test]
    fn three_space_atx_heading_is_structure() {
        // snapper-zogf / GitHub #171 — CommonMark 0.31.2 §4.2 ex. 79.
        let input = "   # Title. Still the title.\n\nBody sentence one. Body sentence two.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            matches!(
                &regions[0],
                Region::Structure(s) if s == "   # Title. Still the title.\n"
            ),
            "indented ATX line including three spaces must be Structure, got: {:?}",
            regions[0]
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Still the title"))),
            "heading title must not be Prose: {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(p) => Some(p.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            prose.iter().any(|p| p.contains("Body sentence one")),
            "body must stay Prose: {regions:?}"
        );
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
                vec![Region::Structure(line.clone())],
                "level {hashes}"
            );
        }
    }

    #[test]
    fn setext_heading_equals_is_structure() {
        let input = "Setext Title With Period. Still Title\n=====================================\n\nBody after setext.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            matches!(&regions[0], Region::Structure(s) if s == "Setext Title With Period. Still Title\n"),
            "setext title must be Structure, got: {:?}",
            regions[0]
        );
        assert!(
            matches!(&regions[1], Region::Structure(s) if s.starts_with('=')),
            "setext underline must be Structure, got: {:?}",
            regions[1]
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Still Title"))),
            "setext title must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn setext_heading_dashes_is_structure() {
        let input = "Secondary Setext Title\n----------------------\n\nParagraph text here.\n";
        let regions = MarkdownParser.parse(input);
        assert_eq!(
            regions[0],
            Region::Structure("Secondary Setext Title\n".to_string())
        );
        assert!(matches!(&regions[1], Region::Structure(s) if s.starts_with('-')));
    }

    #[test]
    fn multi_sentence_setext_title_stays_one_line() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "Setext Title With Period. Still Title\n=====================================\n\nBody after setext. Second body.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.starts_with(
                "Setext Title With Period. Still Title\n=====================================\n"
            ),
            "setext title+underline must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("Still Title =====") && !out.contains("Still Title\nStill"),
            "must not glue underline onto reflowed title:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn setext_after_prose_flushes_body() {
        let input = "Body sentence one. Body two.\n\nHeading Here\n============\n";
        let regions = MarkdownParser.parse(input);
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(p) => Some(p.as_str()),
                _ => None,
            })
            .collect();
        assert!(prose.iter().any(|p| p.contains("Body sentence one")));
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "Heading Here\n"))
        );
    }

    #[test]
    fn blockquote_marker_is_structure() {
        let input = "> One. Two.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions[0], Region::Structure("> ".to_string()));
        assert_eq!(regions[1], Region::Prose("One. Two.".to_string()));
        // No trailing newline in the source, so no terminator Structure.
        assert_eq!(regions.len(), 2);
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains('>'))),
            "quote marker must not leak into Prose: {regions:?}"
        );
    }

    #[test]
    fn blockquote_multiline_keeps_each_marker() {
        // Each source quote line is its own item so splice ranges stay
        // contiguous. Reflow of already-split quotes is identity.
        let regions = MarkdownParser.parse("> One.\n> Two.");
        assert_eq!(regions[0], Region::Structure("> ".to_string()));
        assert_eq!(regions[1], Region::Prose("One.".to_string()));
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions[3], Region::Structure("> ".to_string()));
        assert_eq!(regions[4], Region::Prose("Two.".to_string()));
    }

    #[test]
    fn list_and_quote_multi_sentence_hangs() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let dash = format_text("- One. Two.\n", &cfg).unwrap();
        assert_eq!(dash, "- One.\n  Two.\n");
        assert_eq!(format_text(&dash, &cfg).unwrap(), dash);

        let numbered = format_text("1. One. Two.\n", &cfg).unwrap();
        assert_eq!(numbered, "1. One.\n   Two.\n");
        assert_eq!(format_text(&numbered, &cfg).unwrap(), numbered);

        let quote = format_text("> One. Two.\n", &cfg).unwrap();
        assert_eq!(quote, "> One.\n> Two.\n");
        assert_eq!(format_text(&quote, &cfg).unwrap(), quote);
    }

    #[test]
    fn blockquote_keeps_marker_on_each_content_line() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        for input in ["> One. Two.\n", "> One.\n> Two.\n"] {
            let out = format_text(input, &cfg).unwrap();
            let quote_lines: Vec<_> = out.lines().filter(|l| !l.is_empty()).collect();
            assert_eq!(
                quote_lines,
                vec!["> One.", "> Two."],
                "each content line needs `>`, input {input:?}, got:\n{out}"
            );
            assert_eq!(format_text(&out, &cfg).unwrap(), out);
        }
    }

    #[test]
    fn nested_blockquote_keeps_full_prefix() {
        let input = "> > Nested one. Nested two.";
        let regions = MarkdownParser.parse(input);
        assert_eq!(regions[0], Region::Structure("> > ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Nested one. Nested two.".to_string())
        );
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn nested_blockquote_reflow_repeats_prefix() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "> Quoted one. Quoted two.\n> > Nested one. Nested two.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            "> Quoted one.\n> Quoted two.\n> > Nested one.\n> > Nested two.\n"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn nested_list_stays_two_items_after_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "1. Parent one. Parent two.\n   - Child one. Child two.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            "1. Parent one.\n   Parent two.\n   - Child one.\n     Child two.\n"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let regions = MarkdownParser.parse(&out);
        assert_eq!(regions[0], Region::Structure("1. ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Parent one.\nParent two.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions[3], Region::Structure("   - ".to_string()));
        assert_eq!(
            regions[4],
            Region::Prose("Child one.\nChild two.".to_string())
        );
        assert_eq!(regions[5], Region::Structure("\n".to_string()));
    }

    #[test]
    fn hard_break_two_spaces_not_joined_with_space() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "line  \ncontinued. Next sentence.\n";
        let regions = MarkdownParser.parse(input);
        let joined: String = regions
            .iter()
            .map(|r| match r {
                Region::Prose(p) | Region::Structure(p) | Region::BlankLines(p) => p.as_str(),
                Region::Code { .. } => "",
            })
            .collect();
        assert!(
            !joined.contains("line continued"),
            "two trailing spaces are a hard break, not a space join: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| match r {
                Region::Structure(s) => s.contains("  \n") || s.ends_with("  \n"),
                _ => false,
            }) || joined.contains("line  \n"),
            "hard-break spaces must survive classification: {regions:?}"
        );

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("line continued"),
            "must not collapse hard break to a space, got:\n{out}"
        );
        assert!(
            out.contains("line  \n") || out.contains("line  \r"),
            "two trailing spaces must remain, got:\n{out:?}"
        );
        assert!(out.contains("Next sentence."));
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn hard_break_backslash_not_joined_with_space() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "line\\\ncontinued. Next sentence.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("line continued") && !out.contains("line\\ continued"),
            "backslash hard break must not become a space, got:\n{out}"
        );
        assert!(
            out.contains("line\\\ncontinued"),
            "backslash hard break must remain, got:\n{out:?}"
        );
        assert!(out.contains("Next sentence."));
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn html_comment_multiline_is_structure() {
        let input = "Before sentence. After.\n<!--\nHidden. With a period.\nStill comment.\n-->\nMore. Text.";
        let regions = MarkdownParser.parse(input);
        let comment = regions.iter().find_map(|r| match r {
            Region::Structure(s) if s.contains("<!--") => Some(s.as_str()),
            _ => None,
        });
        let comment = comment.expect(&format!("comment must be Structure, got {regions:?}"));
        assert!(comment.contains("<!--"), "{comment}");
        assert!(comment.contains("Hidden. With a period."), "{comment}");
        assert!(comment.contains("Still comment."), "{comment}");
        assert!(comment.contains("-->"), "{comment}");
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Hidden") || p.contains("Still comment"))),
            "comment body must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn html_comment_multiline_passes_through_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "Before sentence. After.\n<!--\nHidden. With a period.\nStill comment.\n-->\nMore. Text.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("<!--\nHidden. With a period.\nStill comment.\n-->\n"),
            "multiline comment must pass through, got:\n{out}"
        );
        assert!(out.contains("Before sentence.\nAfter."));
        assert!(out.contains("More.\nText."));
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn html_comment_pragma_still_disables_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "Hello world. Goodbye world.\n<!-- snapper:off -->\nKeep this. Exactly here.\n<!-- snapper:on -->\nFinal thing. Last sentence.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(out.contains("Hello world.\nGoodbye world.\n"));
        assert!(
            out.contains("Keep this. Exactly here.\n"),
            "pragma-off body must stay untouched, got:\n{out}"
        );
        assert!(out.contains("Final thing.\nLast sentence."));
        assert!(out.contains("<!-- snapper:off -->"));
        assert!(out.contains("<!-- snapper:on -->"));
    }

    #[test]
    fn quote_hard_break_then_nonquote_has_no_stray_marker() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "> line  \nNext sentence.\n";
        let regions = MarkdownParser.parse(input);
        let after_break = regions
            .iter()
            .skip_while(|r| !matches!(r, Region::Structure(s) if s.ends_with("  \n")));
        assert!(
            !after_break
                .clone()
                .any(|r| matches!(r, Region::Structure(s) if is_quote_resume(s))),
            "must not emit `>` after a quote hard break into non-quote, got: {regions:?}"
        );

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("> \n") && !out.lines().any(|l| l == ">" || l.trim() == ">"),
            "stray empty quote line, got:\n{out:?}"
        );
        assert!(
            out.starts_with("> line  \nNext sentence."),
            "hard break then non-quote body, got:\n{out:?}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let still_quote = format_text("> line  \n> continued.\n", &cfg).unwrap();
        assert!(
            still_quote.starts_with("> line  \n> continued."),
            "in-quote hard break must still resume `>`, got:\n{still_quote:?}"
        );
    }

    fn is_quote_resume(s: &str) -> bool {
        !s.is_empty()
            && !s.contains('\n')
            && s.contains('>')
            && s.bytes().all(|b| b == b'>' || b == b' ')
    }

    #[test]
    fn quote_wrap_repeats_prefix_under_max_width() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "> One two three four five six seven eight.\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            max_width: 20,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        let lines: Vec<_> = out.lines().filter(|l| !l.is_empty()).collect();
        assert!(
            lines.len() > 1,
            "sentence must wrap under max_width=20, got:\n{out}"
        );
        for line in &lines {
            assert!(
                line.starts_with("> "),
                "every wrap line keeps `>`, not a space hang, got:\n{out}"
            );
            assert!(
                line.chars().count() <= 20,
                "prefix counts toward max_width: {line:?} ({out})"
            );
        }
        assert!(
            !out.contains("\n  "),
            "must not hang quote wrap with spaces: {out:?}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    /// GitHub #101 / snapper-bznt: `> ``` must open Code, not sentence-split.
    #[test]
    fn quoted_fenced_code_is_not_sentence_split() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = concat!(
            "> ```\n",
            "> print(1. 2)\n",
            "> still code. yes\n",
            "> ```\n",
        );
        let regions = MarkdownParser.parse(input);
        match regions.iter().find(|r| matches!(r, Region::Code { .. })) {
            Some(Region::Code {
                header,
                body,
                footer,
                ..
            }) => {
                assert!(
                    header.contains("```"),
                    "quoted opener must be the Code header: {header:?}"
                );
                assert!(
                    body.contains("print(1. 2)"),
                    "quoted fence body must stay literal: {body:?}"
                );
                assert!(
                    body.contains("still code. yes"),
                    "quoted fence body must stay literal: {body:?}"
                );
                assert!(
                    footer.contains("```"),
                    "quoted closer must be the Code footer: {footer:?}"
                );
            }
            other => panic!("quoted fence must be Code, got {other:?} / {regions:?}"),
        }
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("print") || p.contains("still code")
            )),
            "quoted fence body must not be Prose: {regions:?}"
        );

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input, "quoted fence must not reflow, got:\n{out}");
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn quoted_tilde_fence_without_space_is_code() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = concat!(">~~~\n", "> print(1. 2)\n", "> still code. yes\n", ">~~~\n",);
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("print(1. 2)") && body.contains("still code. yes")
            )),
            ">`~~~` (space optional) must open Code, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("print") || p.contains("still code")
            )),
            "quoted tilde fence body must not be Prose: {regions:?}"
        );
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        }
        .without_safety_backstops();
        assert_eq!(format_text(input, &cfg).unwrap(), input);
    }

    #[test]
    fn nested_quoted_fence_is_code() {
        let input = "> > ```\n> > print(1. 2)\n> > still code. yes\n> > ```\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("print(1. 2)")
            )),
            "nested `> > ``` must open Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("print"))),
            "nested quoted fence body must not be Prose: {regions:?}"
        );
    }

    /// GitHub #86 / snapper-tupp: after a blank, 4 spaces is Code, not Prose.
    #[test]
    fn indented_code_after_blank_is_code_not_prose() {
        let input = concat!(
            "After a blank, this is code.\n",
            "\n",
            "    def f():\n",
            "        return 1.0\n",
            "\n",
            "Next sentence. Another.\n",
        );
        let regions = MarkdownParser.parse(input);
        match regions.iter().find(|r| matches!(r, Region::Code { .. })) {
            Some(Region::Code {
                lang,
                header,
                body,
                footer,
            }) => {
                assert_eq!(lang.as_deref(), None);
                assert_eq!(header, "");
                assert!(
                    body.contains("    def f():\n") && body.contains("        return 1.0\n"),
                    "indented lines stay in the Code body: {body:?}"
                );
                assert!(
                    !body.contains("Next sentence"),
                    "following prose must not enter the Code body: {body:?}"
                );
                assert_eq!(
                    footer, "\n",
                    "Code runs through the blank that ends the block: {footer:?}"
                );
            }
            other => panic!("indented code must be Code, got {other:?} / {regions:?}"),
        }
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("def f") || p.contains("return 1.0")
            )),
            "indented code must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn indented_code_fixture_is_identity_under_format() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let input = concat!(
            "After a blank, this is code.\n",
            "\n",
            "    def f():\n",
            "        return 1.0\n",
            "\n",
            "Next sentence. Another.\n",
        );
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("    def f():\n        return 1.0\n"),
            "indent and body must stay literal, got:\n{out}"
        );
        assert!(
            !out.contains("return 1.0.") && !out.contains("def f(): return"),
            "must not reflow indented code as prose, got:\n{out}"
        );
        assert!(
            out.contains("Next sentence.\nAnother."),
            "surrounding prose still splits, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
        assert!(
            oracle::matches(Format::Markdown, input, &out),
            "oracle must accept the fixture reflow\n in={input:?}\n out={out:?}"
        );

        let cfg = cfg.without_safety_backstops();
        let raw = format_text(input, &cfg).unwrap();
        assert!(
            raw.contains("    def f():\n        return 1.0\n"),
            "without backstops, indent must still stay, got:\n{raw}"
        );
    }

    #[test]
    fn tab_indented_code_after_blank_is_code() {
        let input = "Before.\n\n\tdef f():\n\t\treturn 1.0\n\nAfter.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. }
                    if body.contains("\tdef f():\n") && body.contains("\t\treturn 1.0\n")
            )),
            "tab indent after a blank must be Code, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("def f") || p.contains("return 1.0")
            )),
            "tab-indented code must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn indented_line_without_blank_stays_prose() {
        let input = "This is a paragraph\n    still the same paragraph.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            !regions.iter().any(|r| matches!(r, Region::Code { .. })),
            "lazy continuation is not indented code: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("still the same paragraph")
            )),
            "unblanked indent stays Prose: {regions:?}"
        );
    }

    /// GitHub #87 / snapper-e6ig: CommonMark HTML blocks must not reflow as prose.
    fn ticket_html_blocks_fixture() -> &'static str {
        concat!(
            "Intro. More.\n",
            "\n",
            "<div class=\"note\">\n",
            "<p>Hello. World.</p>\n",
            "</div>\n",
            "\n",
            "<script>\n",
            "x = 1. Next = 2.\n",
            "</script>\n",
        )
    }

    #[test]
    fn html_div_block_is_structure() {
        let regions = MarkdownParser.parse(ticket_html_blocks_fixture());
        let div = regions.iter().find_map(|r| match r {
            Region::Structure(s) if s.contains("<div") => Some(s.as_str()),
            _ => None,
        });
        let div = div.expect(&format!("div block must be Structure, got {regions:?}"));
        assert!(div.contains("<div class=\"note\">"), "{div}");
        assert!(div.contains("<p>Hello. World.</p>"), "{div}");
        assert!(div.contains("</div>"), "{div}");
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Hello") || p.contains("<div")
            )),
            "div body must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn html_script_block_is_code() {
        let regions = MarkdownParser.parse(ticket_html_blocks_fixture());
        match regions.iter().find(|r| matches!(r, Region::Code { .. })) {
            Some(Region::Code {
                header,
                body,
                footer,
                ..
            }) => {
                assert!(header.contains("<script>"), "{header:?}");
                assert!(
                    body.contains("x = 1. Next = 2."),
                    "script body must stay literal: {body:?}"
                );
                assert!(footer.contains("</script>"), "{footer:?}");
            }
            other => panic!("script block must be Code, got {other:?} / {regions:?}"),
        }
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Next = 2") || p.contains("<script")
            )),
            "script body must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn html_pre_block_is_code() {
        let input = "<pre>\nfoo. bar\n</pre>\n";
        let regions = MarkdownParser.parse(input);
        match regions.iter().find(|r| matches!(r, Region::Code { .. })) {
            Some(Region::Code {
                header,
                body,
                footer,
                ..
            }) => {
                assert!(header.contains("<pre>"), "{header:?}");
                assert!(body.contains("foo. bar"), "{body:?}");
                assert!(footer.contains("</pre>"), "{footer:?}");
            }
            other => panic!("pre block must be Code, got {other:?} / {regions:?}"),
        }
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("foo"))),
            "pre body must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn html_pi_declaration_and_cdata_are_structure() {
        let input = concat!(
            "<?php echo \"Hi. There\"; ?>\n",
            "<!DOCTYPE html something. else>\n",
            "<![CDATA[\n",
            "Hello. World.\n",
            "]]>\n",
        );
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("<?php") && s.contains("Hi. There")
            )),
            "processing instruction must be Structure: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("<!DOCTYPE") && s.contains("something. else")
            )),
            "declaration must be Structure: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("<![CDATA[") && s.contains("Hello. World.")
            )),
            "CDATA must be Structure: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(r, Region::Prose(_))),
            "types 3–5 must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn html_type7_complete_tag_is_structure() {
        let input = "<span class=\"note\">\n\nAfter. Text.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            matches!(&regions[0], Region::Structure(s) if s.contains("<span class=\"note\">")),
            "type 7 opener must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("<span"))),
            "type 7 tag must not be Prose: {regions:?}"
        );
    }

    #[test]
    fn html_type7_does_not_interrupt_paragraph() {
        let input = "Intro. More.\n<span class=\"x\">\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Intro") && p.contains("<span")
            )),
            "type 7 must stay in the paragraph, got: {regions:?}"
        );
    }

    #[test]
    fn html_blocks_ticket_fixture_does_not_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = ticket_html_blocks_fixture();
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Intro.\nMore."),
            "surrounding prose must still reflow, got:\n{out}"
        );
        assert!(
            out.contains("<div class=\"note\">\n<p>Hello. World.</p>\n</div>\n"),
            "div HTML block must stay raw, got:\n{out}"
        );
        assert!(
            out.contains("<script>\nx = 1. Next = 2.\n</script>\n"),
            "script HTML block must stay raw, got:\n{out}"
        );
        assert!(
            !out.contains("<p>Hello.\nWorld.</p>") && !out.contains("x = 1.\nNext = 2."),
            "HTML block interiors must not sentence-split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let raw = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        }
        .without_safety_backstops();
        let raw_out = format_text(input, &raw).unwrap();
        assert_eq!(
            raw_out, out,
            "oracle-silent path must match, got:\n{raw_out}"
        );
        assert_eq!(format_text(&raw_out, &raw).unwrap(), raw_out);
    }

    fn md_cfg() -> crate::FormatConfig {
        crate::FormatConfig {
            format: crate::format::Format::Markdown,
            ..Default::default()
        }
        .without_safety_backstops()
    }

    /// GitHub #84 / snapper-gfsw: Markdown `$$` display math is Structure.
    #[test]
    fn dollar_dollar_display_math_is_structure_not_prose() {
        let input = "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("This is a long sentence that must stay inside display math")
            )),
            "$$ body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This is a long sentence that must stay inside display math")
            )),
            "$$ body must not be Prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "$$")),
            "$$ delimiters must be Structure, got: {regions:?}"
        );
    }

    #[test]
    fn dollar_dollar_display_math_does_not_reflow_as_prose() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$\n";
        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains(
                "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$"
            ),
            "$$ display math must stay a structure block, got:\n{out}"
        );
        assert!(
            !out.contains("$$ This is a long sentence"),
            "must not join $$ into surrounding prose, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let raw = cfg.without_safety_backstops();
        let raw_out = format_text(input, &raw).unwrap();
        assert_eq!(
            raw_out, out,
            "oracle-silent path must match, got:\n{raw_out}"
        );
        assert_eq!(format_text(&raw_out, &raw).unwrap(), raw_out);
    }

    #[test]
    fn dollar_dollar_two_sentences_do_not_split() {
        use crate::format_text;

        let input =
            "$$\nFirst sentence. Second sentence that would split if this were prose.\n$$\n";
        let out = format_text(input, &md_cfg()).unwrap();
        assert!(
            out.contains(
                "$$\nFirst sentence. Second sentence that would split if this were prose.\n$$"
            ),
            "$$ display math must stay a structure block, got:\n{out}"
        );
        assert!(
            !out.contains("$$ First sentence"),
            "must not join $$ into surrounding prose, got:\n{out}"
        );
        assert!(
            !out.contains("First sentence.\nSecond sentence"),
            "$$ body must not split at sentence end, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    #[test]
    fn dollar_dollar_single_line_display_is_structure() {
        use crate::format_text;

        let input = "Before the math. More before.\n$$E = mc^2$$\nAfter the math. More after.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("$$E = mc^2$$"))),
            "single-line $$...$$ must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("E = mc^2"))),
            "single-line $$ body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &md_cfg()).unwrap();
        assert!(
            out.contains("$$E = mc^2$$"),
            "single-line $$ must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("Before the math.\nMore before."),
            "surrounding prose must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    #[test]
    fn inline_single_dollar_math_is_still_prose() {
        use crate::format_text;

        let input = "See $x = 1$ here. Next sentence.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("$x = 1$"))),
            "inline $...$ must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &md_cfg()).unwrap();
        assert!(
            out.contains("See $x = 1$ here.\nNext sentence."),
            "inline $...$ must not open display math, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Markdown): blank then `---` is HR, not
    /// joined prose; `***` after a paragraph interrupts (snapper-ojf8).
    fn ojf8_fixture() -> &'static str {
        concat!(
            "Hello world. Next sentence.\n",
            "\n",
            "---\n",
            "Still going. More text.\n",
            "\n",
            "Intro. Text.\n",
            "***\n",
        )
    }

    #[test]
    fn thematic_break_predicate_matches_commonmark_4_1() {
        assert!(is_thematic_break("***"));
        assert!(is_thematic_break("---"));
        assert!(is_thematic_break("___"));
        assert!(is_thematic_break("* * *"));
        assert!(is_thematic_break("- - -"));
        assert!(is_thematic_break("_ _ _"));
        assert!(is_thematic_break(" ***"));
        assert!(is_thematic_break("  ***"));
        assert!(is_thematic_break("   ***"));
        assert!(!is_thematic_break("    ***"));
        assert!(!is_thematic_break("**"));
        assert!(!is_thematic_break("--"));
        assert!(!is_thematic_break("==="));
        assert!(!is_thematic_break("*-*"));
        assert!(!is_thematic_break("+++"));
    }

    #[test]
    fn blank_then_dashes_is_thematic_break_not_joined_prose() {
        use crate::format_text;

        let input = "Hello world. Next sentence.\n\n---\nStill going. More text.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "---")),
            "--- after a blank must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("---"))),
            "--- must not join surrounding prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Hello world.") && p.contains("Next sentence.")
            )),
            "prose before the break must stay Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Still going.") && p.contains("More text.")
            )),
            "prose after the break must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &md_cfg()).unwrap();
        assert!(
            out.contains("Hello world.\nNext sentence.\n\n---\nStill going."),
            "--- must stay a break between paragraphs, got:\n{out}"
        );
        assert!(
            !out.contains("--- Still going.") && !out.contains("Next sentence. ---"),
            "--- must not join either paragraph, got:\n{out}"
        );
        assert!(
            out.contains("Still going.\nMore text."),
            "prose after --- must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    #[test]
    fn starred_thematic_break_interrupts_paragraph() {
        use crate::format_text;

        let input = "Intro. Text.\n***\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "***")),
            "*** must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("***"))),
            "*** must not join the preceding paragraph, got: {regions:?}"
        );
        let out = format_text(input, &md_cfg()).unwrap();
        assert!(
            out.contains("Intro.\nText.\n***"),
            "*** must interrupt the paragraph, got:\n{out}"
        );
        assert!(
            !out.contains("Text. ***") && !out.contains("Text.\n*** Text"),
            "*** must not glue onto prose, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    #[test]
    fn spaced_star_thematic_break_is_not_a_list() {
        use crate::format_text;

        // Isolated so origin/main cannot pass via LIST_ITEM_RE (`* ` + `* *`).
        let input = "Intro. Text.\n* * *\nAfter. More.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "* * *")),
            "* * * must be a Structure break, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "* ")),
            "* * * must not be a list marker, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("* *"))),
            "* * * must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &md_cfg()).unwrap();
        assert!(
            out.contains("Intro.\nText.\n* * *\nAfter."),
            "* * * must stay a break, not a list hang, got:\n{out}"
        );
        assert!(
            !out.contains("* * * After.") && !out.contains("\n  After."),
            "* * * must not reflow as a list, got:\n{out}"
        );
        assert!(
            out.contains("After.\nMore."),
            "prose after * * * must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    #[test]
    fn setext_dash_underline_stays_heading_not_hr() {
        use crate::format_text;

        let input = "Foo title. Still title\n---\nBody after. More.\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            matches!(&regions[0], Region::Structure(s) if s == "Foo title. Still title\n"),
            "Foo\\n--- must stay setext title, got: {regions:?}"
        );
        assert!(
            matches!(&regions[1], Region::Structure(s) if s.trim() == "---"),
            "setext underline must stay Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Still title"))),
            "setext title must not become Prose, got: {regions:?}"
        );
        let out = format_text(input, &md_cfg()).unwrap();
        assert!(
            out.starts_with("Foo title. Still title\n---\n"),
            "Foo\\n--- must stay setext, not paragraph + HR, got:\n{out}"
        );
        assert!(
            out.contains("Body after.\nMore."),
            "prose after setext must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    #[test]
    fn ojf8_fixture_thematic_breaks_stay_structure() {
        use crate::format_text;

        let input = ojf8_fixture();
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "---")),
            "ticket --- must be Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "***")),
            "ticket *** must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("---") || p.contains("***")
            )),
            "ticket breaks must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &md_cfg()).unwrap();
        assert!(
            out.contains("Hello world.\nNext sentence.\n\n---\nStill going.\nMore text."),
            "ticket --- fixture must not join, got:\n{out}"
        );
        assert!(
            out.contains("Intro.\nText.\n***"),
            "ticket *** fixture must interrupt, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    #[test]
    fn underscore_and_spaced_dash_breaks_are_structure() {
        use crate::format_text;

        for marker in ["___", "_ _ _", "- - -", "   ---"] {
            // Blank before dashes so setext cannot claim the line (Kang).
            let input = format!("Before. Text.\n\n{marker}\nAfter. More.\n");
            let regions = MarkdownParser.parse(&input);
            assert!(
                regions
                    .iter()
                    .any(|r| matches!(r, Region::Structure(s) if s.trim() == marker.trim())),
                "{marker:?} must be Structure, got: {regions:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains(marker.trim()))),
                "{marker:?} must not join prose, got: {regions:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Structure(s) if s == "- " || s == "* ")),
                "{marker:?} must not be a list marker, got: {regions:?}"
            );
            let out = format_text(&input, &md_cfg()).unwrap();
            assert!(
                out.contains(&format!("Before.\nText.\n\n{marker}\nAfter.")),
                "{marker:?} must stay a break, got:\n{out}"
            );
            assert!(
                out.contains("After.\nMore."),
                "prose after {marker:?} must still reflow, got:\n{out}"
            );
            assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (Format::Markdown): `>One` is a quote (CM 0.31.2
    /// ex. 229); lazy `five. six` stays in the quote (ex. 228).
    fn fbmn_fixture() -> &'static str {
        concat!(">One. Two.\n", "\n", "> Three. Four.\n", "five. six\n",)
    }

    #[test]
    fn quote_marker_space_is_optional() {
        let regions = MarkdownParser.parse(">One. Two.");
        assert_eq!(regions[0], Region::Structure(">".to_string()));
        assert_eq!(regions[1], Region::Prose("One. Two.".to_string()));
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains('>'))),
            "bare `>` must not leak into Prose: {regions:?}"
        );
    }

    #[test]
    fn nested_quote_without_spaces_is_quote() {
        let regions = MarkdownParser.parse(">>nested one. nested two.");
        assert_eq!(regions[0], Region::Structure(">>".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("nested one. nested two.".to_string())
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains('>'))),
            "bare `>>` must not leak into Prose: {regions:?}"
        );
    }

    #[test]
    fn lazy_continuation_stays_in_quote() {
        use crate::format_text;

        let input = "> Three. Four.\nfive. six\n";
        let regions = MarkdownParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Three. Four.") && p.contains("five. six")
            )),
            "lazy line must join quote prose, got: {regions:?}"
        );
        let out = format_text(input, &md_cfg()).unwrap();
        // "six" is lowercase, so it is not a new sentence; it must still
        // stay inside the quote (the `>` is repeated).
        assert_eq!(out, "> Three.\n> Four.\n> five. six\n");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);

        let caps = format_text("> Three. Four.\nFive. Six.\n", &md_cfg()).unwrap();
        assert_eq!(caps, "> Three.\n> Four.\n> Five.\n> Six.\n");
        assert_eq!(format_text(&caps, &md_cfg()).unwrap(), caps);
    }

    #[test]
    fn fbmn_fixture_optional_space_and_lazy_continuation() {
        use crate::format_text;

        let input = fbmn_fixture();
        let regions = MarkdownParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == ">")),
            ">One must emit Structure(`>`), got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("One. Two."))),
            ">One body must be Prose, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains('>'))),
            "quote markers must not leak into Prose: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Three. Four.") && p.contains("five. six")
            )),
            "lazy five. six must stay in the quote, got: {regions:?}"
        );

        let out = format_text(input, &md_cfg()).unwrap();
        assert_eq!(out, ">One.\n>Two.\n\n> Three.\n> Four.\n> five. six\n");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }

    #[test]
    fn nospace_quote_reflow_repeats_bare_marker() {
        use crate::format_text;

        let out = format_text(">One. Two.\n", &md_cfg()).unwrap();
        assert_eq!(out, ">One.\n>Two.\n");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);

        let nested = format_text(">>Nested one. Nested two.\n", &md_cfg()).unwrap();
        assert_eq!(nested, ">>Nested one.\n>>Nested two.\n");
        assert_eq!(format_text(&nested, &md_cfg()).unwrap(), nested);
    }
}
