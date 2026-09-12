use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Line, Region, SpannedRegion, flush_prose_spanned, iter_lines,
    join_prose_gap, push_prose_line,
};

/// Match `.. code-block:: LANG` or `.. sourcecode:: LANG` (or `.. code:: LANG`).
static CODE_DIRECTIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*\.\.\s+(?:code-block|sourcecode|code)::\s*([A-Za-z0-9_+.\-]+)?\s*$").unwrap()
});

/// Docutils `Body.grid_table_top_pat`: top/bottom of a full table.
static GRID_TABLE_TOP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\+-[-+]+-\+ *$").unwrap());

/// Docutils `explicit.constructs` footnote then citation (before comment).
/// Footnote label: `[0-9]+` / `#` / `#simplename` / `*`. Citation: `simplename`.
/// `Inliner.simplename` ASCII: alphanumerics with internal `-._+:`.
static FOOTNOTE_CITATION_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^\.\. +\[(?:",
        r"[0-9]+",
        r"|#(?:[A-Za-z0-9]+(?:[-._+:][A-Za-z0-9]+)*)?",
        r"|\*",
        r"|[A-Za-z0-9]+(?:[-._+:][A-Za-z0-9]+)*",
        r")\](?: +|$)",
    ))
    .unwrap()
});

/// Docutils `Body.patterns['option_marker']`: short `-a`/`+v`, long
/// `--long`/`/V`, optional arg (`--input=file`, `-b file`, `<file>`),
/// comma groups, then two-or-more spaces or end of line.
static OPTION_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^((-|\+)[a-zA-Z0-9]( ?([a-zA-Z][a-zA-Z0-9_-]*|<[^<>]+>))?",
        r"|(--|/)[a-zA-Z0-9][a-zA-Z0-9_-]*([ =]([a-zA-Z][a-zA-Z0-9_-]*|<[^<>]+>))?)",
        r"(, ((-|\+)[a-zA-Z0-9]( ?([a-zA-Z][a-zA-Z0-9_-]*|<[^<>]+>))?",
        r"|(--|/)[a-zA-Z0-9][a-zA-Z0-9_-]*([ =]([a-zA-Z][a-zA-Z0-9_-]*|<[^<>]+>))?))*",
        r"(  +| ?$)",
    ))
    .unwrap()
});

pub struct RstParser;

impl FormatParser for RstParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        parse_line_based(input)
    }
}

/// Line-based RST parser. Handles directives, literal blocks (indented
/// and quoted), doctest blocks, sections, field lists, option lists,
/// footnotes, citations, comments, anonymous hyperlink targets, Jinja
/// statements (`{% ... %}`), line blocks, tables, definition lists, and
/// block-quote hang spaces as structure regions. Docutils container
/// directives (admonitions, figure, topic, sidebar, container, leftover
/// body.py parsed-literal / epigraph / highlights / pull-quote / compound)
/// nested-parse their body: the opener and option fields stay Structure;
/// the body hangs as Prose.
fn parse_line_based(input: &str) -> Vec<SpannedRegion> {
    let mut regions = Vec::new();
    let mut current_prose = String::new();
    let mut prose_span: Option<ByteSpan> = None;
    let mut in_literal_block = false;
    let mut literal_indent: usize = 0;
    let mut in_directive = false;
    let mut directive_indent: usize = 0;
    let mut in_definition = false;
    let mut definition_indent: usize = 0;
    // Comment body: indented lines after `..` / `.. text` stay Structure.
    // A blank closes the comment (GitHub #176).
    let mut in_comment = false;
    let mut comment_indent: usize = 0;
    // Hang column of the current list item (`- ` → 2) or block quote.
    // Continuation paragraphs after a blank stay in the item when
    // indented this far.
    let mut list_hang: Option<usize> = None;
    // Compact `.. note::` + indented body + flush paragraph (GitHub #344):
    // the body hangs as a block quote, so a column-0 line must close that
    // hang instead of joining After markup into the note Prose.
    let mut in_container_body = false;
    // Line-block hang (`| `): first flush line that is not `| ` and not
    // a hang is a new paragraph, not more line-block (GitHub #409).
    let mut in_line_block = false;
    let mut pragma_off = false;

    // Code-block directive bookkeeping. Mutually exclusive with `in_directive`.
    let mut in_code_block = false;
    let mut code_indent: usize = 0;
    let mut code_lang: Option<String> = None;
    let mut code_header = ByteSpan::default();
    let mut code_body_start = 0usize;
    let mut code_body_end = 0usize;
    let mut code_footer_start: Option<usize> = None;

    let lines = iter_lines(input);
    let total = lines.len();
    let mut i = 0;

    while i < total {
        let line = &lines[i];
        let line_text = line.text;

        // Pragma check; inside a code-block directive the per-language
        // reflow path handles pragmas instead.
        if !in_code_block {
            if let Some(on) = super::check_pragma(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                pragma_off = !on;
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if pragma_off {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
        }

        // Inside an rst code-block directive body.
        // The body consists of lines indented past `code_indent`, plus
        // interior blank lines. The block ends at a non-blank line whose
        // indent drops below `code_indent`.
        if in_code_block {
            let leading = line_text.len() - line_text.trim_start().len();
            if line_text.trim().is_empty() {
                // Could be interior blank or end-of-block; buffer and look ahead.
                if code_footer_start.is_none() {
                    code_footer_start = Some(line.start);
                }
                i += 1;
                continue;
            }
            if leading >= code_indent {
                // A later body line claims any buffered interior blanks.
                code_footer_start = None;
                code_body_end = line.end;
                i += 1;
                continue;
            }
            // Less-indented non-blank line: close the code block.
            in_code_block = false;
            let footer = match code_footer_start.take() {
                Some(fs) => ByteSpan::new(fs, line.start),
                None => ByteSpan::new(line.start, line.start),
            };
            let body_end = if code_body_end > code_body_start {
                code_body_end
            } else {
                footer.start
            };
            regions.push(SpannedRegion::code(
                input,
                code_lang.take(),
                code_header,
                ByteSpan::new(code_body_start, body_end),
                footer,
            ));
            // Fall through to reprocess this line as normal.
        }

        // Inside literal block
        if in_literal_block {
            let leading = line_text.len() - line_text.trim_start().len();
            if line_text.trim().is_empty() || leading >= literal_indent {
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
            in_literal_block = false;
        }

        // Inside directive body
        if in_directive {
            let leading = line_text.len() - line_text.trim_start().len();
            if line_text.trim().is_empty() {
                // Interior blanks stay in the body. A trailing blank
                // (next non-blank is below the body indent, or EOF)
                // ends the directive so the following hung paragraph
                // is a real BlankLines + hang, not a splice that
                // outdents the second sentence (GitHub #135).
                match next_nonblank_indent(&lines, i) {
                    Some(n) if n >= directive_indent => {
                        regions.push(SpannedRegion::structure(input, line.span()));
                        i += 1;
                        continue;
                    }
                    _ => in_directive = false,
                }
            } else if leading >= directive_indent {
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            } else {
                in_directive = false;
            }
        }

        // Inside comment body (indented lines after `..` / `.. text`).
        // Docutils explicit markup / comment() ends at a blank, so a
        // later indent is a hung quote, not more comment Structure
        // (GitHub #176).
        if in_comment {
            if line_text.trim().is_empty() {
                in_comment = false;
            } else {
                let leading = line_text.len() - line_text.trim_start().len();
                if leading >= comment_indent {
                    regions.push(SpannedRegion::structure(input, line.span()));
                    i += 1;
                    continue;
                }
                in_comment = false;
            }
        }

        // Blank line
        if line_text.trim().is_empty() {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::blank(input, line.span()));
            i += 1;
            continue;
        }

        // Inside a definition-list body (indented lines after a flush term).
        // Blanks already fell through above so they stay BlankLines.
        if in_definition {
            let leading = line_text.len() - line_text.trim_start().len();
            if leading >= definition_indent {
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
            in_definition = false;
        }

        // RST code-block directive (.. code-block:: LANG)
        if let Some(caps) = CODE_DIRECTIVE_RE.captures(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            code_lang = caps.get(1).map(|m| m.as_str().to_string());
            code_header = line.span();
            // Docutils accepts a two-space body; +3 is convention only.
            let leading = line_text.len() - line_text.trim_start().len();
            code_indent = leading + 2;
            code_body_start = line.end;
            code_body_end = line.end;
            code_footer_start = None;
            in_code_block = true;
            i += 1;
            continue;
        }

        // RST directive (`.. name::`). Container directives (admonitions,
        // figure, topic, sidebar, container, leftover body.py
        // parsed-literal / epigraph / highlights / pull-quote / compound)
        // nested-parse their body: the opener and `:option:` fields stay
        // Structure; the body hangs and reflows like a block quote.
        // Opaque names keep the old freeze (GitHub #54 / #351 / #386).
        // A flush paragraph after a compact
        // container body is a new paragraph, not more note (GitHub #344).
        // Specific admonitions nested-parse same-line text after `::`
        // as the first body paragraph: marker Structure, body hung
        // Prose that still splits (GitHub #349).
        let trimmed = line_text.trim_start();
        if trimmed.starts_with(".. ") && trimmed.contains("::") {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            if let Some(marker_len) = rst_admonition_marker_len(line_text) {
                if line_text.len() > marker_len && !line_text[marker_len..].trim().is_empty() {
                    list_hang = Some(marker_len);
                    in_container_body = true;
                    regions.push(SpannedRegion::structure(
                        input,
                        ByteSpan::new(line.start, line.start + marker_len),
                    ));
                    current_prose.push_str(line_text[marker_len..].trim());
                    prose_span = Some(ByteSpan::new(line.start + marker_len, line.end));
                    i += 1;
                    continue;
                }
            }
            regions.push(SpannedRegion::structure(input, line.span()));
            if rst_directive_name(trimmed).is_some_and(|n| is_rst_container_directive(&n)) {
                in_container_body = true;
                i += 1;
                continue;
            }
            in_container_body = false;
            let leading = line_text.len() - trimmed.len();
            // Docutils accepts a two-space body; +3 is convention only.
            directive_indent = leading + 2;
            in_directive = true;
            i += 1;
            continue;
        }

        // Footnote / citation. Docutils explicit.constructs matches these
        // before comment, so `.. [1]` is not a comment (GitHub #175).
        // Marker is Structure; same-line body is hung Prose.
        if let Some(marker_len) = rst_footnote_citation_marker_len(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            list_hang = Some(marker_len);
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(line.start, line.start + marker_len),
            ));
            if line_text.len() > marker_len {
                current_prose.push_str(line_text[marker_len..].trim());
                prose_span = Some(ByteSpan::new(line.start + marker_len, line.end));
            }
            i += 1;
            continue;
        }

        // RST comment (`..` or `.. text` without `::`). Bare `..` is a
        // comment opener; following indented lines are the comment body.
        if is_rst_comment_opener(trimmed) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            let leading = line_text.len() - trimmed.len();
            // Any indent past the opener is body (docutils).
            comment_indent = leading + 1;
            in_comment = true;
            i += 1;
            continue;
        }

        // Jinja statement (`{% ... %}`). sphinx-jinja / Jinja2 block
        // delimiters. Consecutive statements stay unjoined; they are
        // not RST prose (GitHub #196).
        if is_rst_jinja_statement(trimmed) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            list_hang = None;
            regions.push(SpannedRegion::structure(input, line.span()));
            i += 1;
            continue;
        }

        // Anonymous hyperlink target. Docutils Body.anonymous is
        // `__( +|$)`, a sibling of `..` explicit markup. The whole
        // line is Structure so wrap cannot break the URI (GitHub #93).
        if is_rst_anonymous_target(trimmed) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            i += 1;
            continue;
        }

        // Doctest block: Docutils Body.doctest (`>>>( +|$)`) is checked
        // before Body.line, so a prompt-only `>>>` / `>>> ` is not a
        // `>` section underline. Lone `>>>` at EOL is Structure only
        // (GitHub #331); `>>> ` plus command still takes the block
        // through the next blank or dedent; no inline parse.
        if is_rst_doctest_opener(trimmed) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            if is_rst_empty_doctest_opener(trimmed) {
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
            let block_indent = line_text.len() - trimmed.len();
            let block_start = line.start;
            let mut block_end = line.end;
            i += 1;
            while i < total {
                let next = lines[i].text;
                if next.trim().is_empty() {
                    break;
                }
                let next_indent = next.len() - next.trim_start().len();
                if next_indent < block_indent {
                    break;
                }
                block_end = lines[i].end;
                i += 1;
            }
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(block_start, block_end),
            ));
            continue;
        }

        // Section underline
        if is_underline(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            i += 1;
            continue;
        }

        // Section title (next line is underline)
        if i + 1 < total && is_underline(lines[i + 1].text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            i += 1;
            continue;
        }

        // Field list. Docutils field_marker is
        // `:(?![: ])([^:\\]|\\.|:(?!([ `]|$)))*(?<! ):( +|$)`.
        // Interior colons in the name are allowed (GitHub #341).
        // `:role:`text`` is interpreted text, not a field.
        // Empty `:name:` and simple `:Author:` stay whole-line
        // Structure (GitHub #330). Interior-colon names hang the body
        // as Prose so SemBr still splits.
        if let Some(marker_len) = rst_field_marker_len(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            let body = &line_text[marker_len..];
            let name = line_text[line_text.len() - trimmed.len()..marker_len].trim();
            let interior_colon = name
                .strip_prefix(':')
                .and_then(|s| s.strip_suffix(':'))
                .is_some_and(|inner| inner.contains(':'));
            if interior_colon && !body.trim().is_empty() {
                list_hang = Some(marker_len);
                regions.push(SpannedRegion::structure(
                    input,
                    ByteSpan::new(line.start, line.start + marker_len),
                ));
                current_prose.push_str(body.trim());
                prose_span = Some(ByteSpan::new(line.start + marker_len, line.end));
            } else {
                regions.push(SpannedRegion::structure(input, line.span()));
            }
            i += 1;
            continue;
        }

        // Literal block intro (line ending with ::)
        if trimmed.ends_with("::") {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            // Find indent of next non-blank line
            let mut j = i + 1;
            while j < total && lines[j].text.trim().is_empty() {
                j += 1;
            }
            if j < total {
                let next = lines[j].text;
                let next_indent = next.len() - next.trim_start().len();
                if next_indent > 0 {
                    literal_indent = next_indent;
                    in_literal_block = true;
                } else if let Some(q) = rst_quoted_literal_quote(next) {
                    // Flush `>` / `|` / … after `::`. The blank after
                    // `::` is the separator, not the Docutils terminator;
                    // consume quoted lines here so that blank cannot
                    // close the block before the first `>` is seen.
                    i += 1;
                    while i < total && lines[i].text.trim().is_empty() {
                        flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                        regions.push(SpannedRegion::blank(input, lines[i].span()));
                        i += 1;
                    }
                    while i < total && rst_quoted_literal_continues(lines[i].text, q) {
                        regions.push(SpannedRegion::structure(input, lines[i].span()));
                        i += 1;
                    }
                    continue;
                }
            }
            i += 1;
            continue;
        }

        // List item: marker is Structure so `1. First` does not split
        // after `1.`, and each item is its own region so adjacent
        // bullets are not glued onto one line.
        //
        // Docutils opens a list only after a blank, a structure line,
        // or the start of the document. A list-like line that follows
        // an open paragraph with no blank stays in that paragraph
        // (GitHub #134). Empty ASCII `-`/`*` at EOL and unicode bullets
        // (`•`/`‣`/`⁃`, with or without payload) interrupt (GitHub #324).
        // Once a list is open (`list_hang`), the next marker is the
        // next item.
        if let Some(marker_len) = rst_list_marker_len(line_text) {
            if !current_prose.is_empty()
                && list_hang.is_none()
                && !rst_docutils_bullet_interrupts_paragraph(line_text)
            {
                push_prose_line(&mut current_prose, &mut prose_span, line, true, true);
                i += 1;
                continue;
            }
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            list_hang = Some(marker_len);
            // A next line indented past the hang is a definition of this
            // item (GitHub #132), not a compact wrap. Keep term+marker as
            // Structure so reflow does not rehang the body at hang width.
            if let Some(def_indent) = list_item_definition_indent(&lines, i, marker_len) {
                regions.push(SpannedRegion::structure(input, line.span()));
                definition_indent = def_indent;
                in_definition = true;
                i += 1;
                continue;
            }
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(line.start, line.start + marker_len),
            ));
            if line_text.len() > marker_len {
                current_prose.push_str(line_text[marker_len..].trim());
                prose_span = Some(ByteSpan::new(line.start + marker_len, line.end));
            }
            i += 1;
            continue;
        }

        // Option list: option column (marker + 2+ spaces) is Structure
        // so wrap cannot eat the alignment. Description is Prose and
        // hangs at the column width like a list item (GitHub #89).
        if let Some(col_len) = rst_option_column_len(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            list_hang = Some(col_len);
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(line.start, line.start + col_len),
            ));
            if line_text.len() > col_len {
                current_prose.push_str(line_text[col_len..].trim());
                prose_span = Some(ByteSpan::new(line.start + col_len, line.end));
            }
            i += 1;
            continue;
        }

        // Line block: Docutils Body.line_block (`\|( +|$)`) is checked
        // before grid_table_top. `| ` is Structure; the rest hangs as
        // Prose so SemBr still splits (GitHub #174). A following flush
        // line is a new paragraph, not more hang (GitHub #409).
        if let Some(marker_len) = rst_line_block_marker_len(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            list_hang = Some(marker_len);
            in_line_block = true;
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(line.start, line.start + marker_len),
            ));
            if line_text.len() > marker_len {
                current_prose.push_str(line_text[marker_len..].trim());
                prose_span = Some(ByteSpan::new(line.start + marker_len, line.end));
            }
            i += 1;
            continue;
        }

        // Grid table: Docutils grid_table_top (`+---+---+`) plus the
        // `| cell |` / `+===+` rows it isolates. Full-line Structure.
        if is_rst_grid_table_top(trimmed) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            let end = rst_grid_table_end(&lines, i);
            for row in &lines[i..=end] {
                regions.push(SpannedRegion::structure(input, row.span()));
            }
            i = end + 1;
            continue;
        }

        // Leftover `+` fragments (not a grid_table_top) stay Structure.
        // Leftover `|` is not a table: Docutils Inliner substitution_ref
        // (`|version|`) is inline, so those lines stay Prose (snapper-t0th).
        if trimmed.starts_with('+') {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            i += 1;
            continue;
        }

        // Simple table: `=====  =====` borders plus the rows they wrap.
        // `is_underline` rejects those borders (interior spaces), so without
        // this the header/body rows fall through to prose and get joined.
        // Interior blanks stay BlankLines; the walk still reaches the closer.
        // A top with no matching closer still isolates the top line so
        // SemBr cannot join it onto following flush prose (GitHub #347).
        if is_simple_table_border(line_text) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            let end = simple_table_end(&lines, i).unwrap_or(i);
            for row in &lines[i..=end] {
                if row.text.trim().is_empty() {
                    regions.push(SpannedRegion::blank(input, row.span()));
                } else {
                    regions.push(SpannedRegion::structure(input, row.span()));
                }
            }
            i = end + 1;
            continue;
        }

        // Definition list: flush term plus an immediately indented definition.
        // Without this both lines are Prose and splice joins them.
        if is_definition_term(&lines, i) {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            regions.push(SpannedRegion::structure(input, line.span()));
            let next = lines[i + 1].text;
            definition_indent = next.len() - next.trim_start().len();
            in_definition = true;
            i += 1;
            continue;
        }

        // List continuation: after a list item, a later line indented
        // to the hang (two spaces after `- `) is still the item.
        // A blank before it is a new paragraph: hang spaces stay
        // Structure so splice does not outdent them. Compact wrap
        // (no blank) is the same paragraph; join into the open Prose
        // so a reflow hang reparses as the source list item.
        // A join_prose_gap newline after `No.` / `etc.`, an open quote,
        // an open `(`, or an open RST inline literal stays in that
        // Prose; reflow repeats the hang on those continuation lines
        // (GitHub #129, #130, #131, #143).
        // Empty current_prose after a nested Structure block (directive)
        // is a new hung paragraph, not a compact join (GitHub #135).
        if let Some(hang) = list_hang {
            let leading = line_text.len() - line_text.trim_start().len();
            if leading >= hang {
                let after_blank = regions
                    .last()
                    .is_some_and(|r| matches!(r.region, Region::BlankLines(_)));
                if after_blank || current_prose.is_empty() {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(
                        input,
                        ByteSpan::new(line.start, line.start + leading),
                    ));
                    if line_text.len() > leading {
                        current_prose.push_str(line_text[leading..].trim());
                        prose_span = Some(ByteSpan::new(line.start + leading, line.end));
                    }
                } else if line_text.len() > leading {
                    if !current_prose.is_empty() {
                        join_prose_gap(&mut current_prose);
                    }
                    current_prose.push_str(line_text[leading..].trim());
                    match prose_span.as_mut() {
                        Some(span) => span.end = line.end,
                        None => {
                            prose_span = Some(ByteSpan::new(line.start + leading, line.end));
                        }
                    }
                }
                i += 1;
                continue;
            }
            list_hang = None;
            // Docutils ends a container at the first less-indented line.
            // Without a blank, the hung body is still in current_prose;
            // flush so After markup stays column-0 Prose (GitHub #344).
            // Same close for line-block: first flush line that is not
            // `| ` and not a hang is a new paragraph (GitHub #409).
            if (in_container_body || in_line_block) && leading == 0 {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_container_body = false;
                in_line_block = false;
            }
        }

        // Block quote: indented prose that is not a list, directive,
        // comment, definition, or literal. Hang spaces stay Structure
        // so splice does not outdent them (GitHub #58). Compact wrap
        // (no blank) joins via list_hang so SemBr still applies.
        let leading = line_text.len() - line_text.trim_start().len();
        if leading > 0 {
            flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            list_hang = Some(leading);
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(line.start, line.start + leading),
            ));
            if line_text.len() > leading {
                current_prose.push_str(line_text[leading..].trim());
                prose_span = Some(ByteSpan::new(line.start + leading, line.end));
            }
            i += 1;
            continue;
        }

        // Regular prose
        in_container_body = false;
        in_line_block = false;
        push_prose_line(&mut current_prose, &mut prose_span, line, true, true);
        i += 1;
    }

    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
    if in_code_block {
        let footer = match code_footer_start {
            Some(fs) => ByteSpan::new(fs, input.len()),
            None => ByteSpan::new(input.len(), input.len()),
        };
        let body_end = if code_body_end > code_body_start {
            code_body_end
        } else {
            footer.start
        };
        regions.push(SpannedRegion::code(
            input,
            code_lang.take(),
            code_header,
            ByteSpan::new(code_body_start, body_end),
            footer,
        ));
    }
    regions
}

/// Docutils `Body.patterns['doctest']`: `>>>( +|$)`.
/// Prompt-only `>>>` and `>>> ` plus command both open a block; `>>>print` does not.
pub(crate) fn is_rst_doctest_opener(trimmed: &str) -> bool {
    trimmed == ">>>" || trimmed.starts_with(">>> ")
}

/// Lone `>>>` / `>>> ` (spaces only) at EOL. No command, so the next
/// line is not doctest output (GitHub #331).
fn is_rst_empty_doctest_opener(trimmed: &str) -> bool {
    trimmed
        .strip_prefix(">>>")
        .is_some_and(|rest| rest.trim().is_empty())
}

/// Docutils `Body.patterns['anonymous']`: `__( +|$)`.
/// Sibling of `..` explicit markup. `__https` (no space) is not a target.
pub(crate) fn is_rst_anonymous_target(trimmed: &str) -> bool {
    trimmed == "__" || trimmed.starts_with("__ ")
}

/// Whole-line Jinja statement: `{%` … `%}` with optional whitespace-control
/// `-`/`+` on either brace (`{%-`, `{%+`, `-%}`, `+%}`).
///
/// Kanged from Jinja2 / sphinx-jinja block delimiters (`block_start_string`
/// / `block_end_string`), not a pandoc RST parse (GitHub #196).
pub(crate) fn is_rst_jinja_statement(trimmed: &str) -> bool {
    let s = trimmed.trim_end();
    // `{%-` / `{%+` / `-%}` / `+%}` still start with `{%` and end with `%}`.
    s.starts_with("{%") && s.ends_with("%}")
}

/// Byte length of a Docutils footnote or citation opener on `line`,
/// including leading indent and the space after `]`.
/// `None` when the line is a comment or other explicit markup.
pub(crate) fn rst_footnote_citation_marker_len(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    FOOTNOTE_CITATION_MARKER_RE
        .find(&line[indent..])
        .map(|m| indent + m.end())
}

/// Docutils directive name on a `.. name::` line, ASCII-lowercased.
/// `None` when the line is not an explicit-markup directive.
fn rst_directive_name(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("..")?;
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let rest = rest.trim_start();
    let name_end = rest.find("::")?;
    let name = rest[..name_end].trim();
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return None;
    }
    Some(name.to_ascii_lowercase())
}

/// Docutils specific admonitions plus leftover body.py names with no
/// arguments (epigraph / highlights / pull-quote / compound): same-line
/// text after `::` is the first nested-parsed body paragraph
/// (GitHub #349 / #351).
fn is_rst_specific_admonition(name: &str) -> bool {
    matches!(
        name,
        "note"
            | "warning"
            | "caution"
            | "danger"
            | "tip"
            | "important"
            | "hint"
            | "error"
            | "attention"
            | "epigraph"
            | "highlights"
            | "pull-quote"
            | "compound"
    )
}

/// Docutils admonitions plus figure/topic/sidebar/container and leftover
/// body.py `parsed-literal`: bodies nested-parse, so hang + reflow.
/// Option fields stay Structure via the field-list arm. Other directive
/// names stay opaque. GitHub #386.
fn is_rst_container_directive(name: &str) -> bool {
    is_rst_specific_admonition(name)
        || matches!(
            name,
            "admonition" | "figure" | "topic" | "sidebar" | "container" | "parsed-literal"
        )
}

/// Byte length of a specific-admonition opener (`.. note::` plus
/// following whitespace, including leading indent). `None` when the
/// line is not a no-argument admonition. Body text after the marker
/// is not included, so `Some(s.len())` is the Structure prefix used
/// for hang (GitHub #349).
pub(crate) fn rst_admonition_marker_len(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = &line[indent..];
    let rest = trimmed.strip_prefix("..")?;
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let name_off = rest.len() - rest.trim_start().len();
    let after_ws = &rest[name_off..];
    let name_end = after_ws.find("::")?;
    let name = after_ws[..name_end].trim();
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return None;
    }
    if !is_rst_specific_admonition(&name.to_ascii_lowercase()) {
        return None;
    }
    let colons_at = indent + 2 + name_off + name_end;
    let after_colons = &line[colons_at + 2..];
    let pad = after_colons.len() - after_colons.trim_start().len();
    Some(colons_at + 2 + pad)
}

/// True when `trimmed` is an RST comment opener, not a `.. name::`
/// directive and not a footnote/citation (`.. [1]`, `.. [CIT2002]`).
/// Bare `..` (no trailing space) is a valid opener; so is `.. text`.
pub(crate) fn is_rst_comment_opener(trimmed: &str) -> bool {
    if trimmed.contains("::") {
        return false;
    }
    if rst_footnote_citation_marker_len(trimmed).is_some() {
        return false;
    }
    trimmed == ".." || trimmed.starts_with(".. ") || trimmed.starts_with("..\t")
}

/// Comment openers pandoc's RST reader drops (zero blocks). Hyperlink
/// targets (`.. _name:`) and footnotes (`.. [1]`) start with `..` but
/// survive the reader, so they are not a drop.
pub(crate) fn is_rst_dropped_comment_opener(trimmed: &str) -> bool {
    if !is_rst_comment_opener(trimmed) {
        return false;
    }
    let after = trimmed.strip_prefix("..").unwrap_or("").trim_start();
    if after.is_empty() {
        return true;
    }
    !after.starts_with('_') && !after.starts_with('[')
}

/// Source has an RST `..` comment that `pandoc -f rst` would delete.
pub(crate) fn source_has_dropped_rst_comments(input: &str) -> bool {
    input
        .lines()
        .any(|line| is_rst_dropped_comment_opener(line.trim_start()))
}

/// True when `trimmed` is an RST field-list item (`:name: value`,
/// interior-colon `:py:mod: value`, or empty `:name:` at EOL).
/// Docutils `field_marker` is
/// `:(?![: ])([^:\\]|\\.|:(?!([ `]|$)))*(?<! ):( +|$)`.
/// `:role:`text`` is interpreted text, not a field
/// (GitHub #126 / #330 / #341).
pub(crate) fn is_rst_field_list_line(trimmed: &str) -> bool {
    rst_field_marker_end(trimmed).is_some()
}

/// Byte length of a Docutils field marker on `line`, including leading
/// indent and the spaces after the closing colon.
pub(crate) fn rst_field_marker_len(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    rst_field_marker_end(&line[indent..]).map(|n| indent + n)
}

/// Byte length of the field marker at the start of `trimmed` (no
/// leading indent). Walks the Docutils `field_marker` so an interior
/// colon in the name (`:py:mod:`) is not taken as the closer.
fn rst_field_marker_end(trimmed: &str) -> Option<usize> {
    let bytes = trimmed.as_bytes();
    if bytes.len() < 3 || bytes[0] != b':' {
        return None;
    }
    // `(?![: ])`: first name byte is not `:` or space.
    if bytes[1] == b':' || bytes[1] == b' ' {
        return None;
    }
    // Name body: `([^:\\]|\\.|:(?!([ `]|$)))*`
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                if i + 1 >= bytes.len() {
                    return None;
                }
                i += 2;
            }
            b':' => {
                let next = bytes.get(i + 1).copied();
                // Interior colon is `:(?!([ `]|$)`.
                if !matches!(next, None | Some(b' ') | Some(b'`')) {
                    i += 1;
                    continue;
                }
                // Closing colon. `(?<! )`: previous byte is not space.
                if bytes[i - 1] == b' ' {
                    return None;
                }
                let after = i + 1;
                if after == bytes.len() {
                    return Some(after);
                }
                // `( +|$)`, plus the pre-#341 tab-as-separator form.
                if !bytes[after].is_ascii_whitespace() {
                    return None;
                }
                let mut end = after;
                while end < bytes.len() && bytes[end] == b' ' {
                    end += 1;
                }
                return Some(end);
            }
            _ => i += 1,
        }
    }
    None
}

/// Byte length of a Docutils line-block opener on `line`, including
/// leading indent and the spaces after `|`. Pattern: `\|( +|$)`.
/// `| ` / `|` at EOL open a line block; `|cell` does not (GitHub #174).
pub(crate) fn rst_line_block_marker_len(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    let t = &line[indent..];
    if t == "|" {
        return Some(indent + 1);
    }
    let after = t.strip_prefix('|')?;
    let spaces = after.bytes().take_while(|&b| b == b' ').count();
    (spaces > 0).then_some(indent + 1 + spaces)
}

/// Docutils `Body.grid_table_top_pat`: `\+-[-+]+-\+ *$`.
fn is_rst_grid_table_top(trimmed: &str) -> bool {
    GRID_TABLE_TOP_RE.is_match(trimmed)
}

/// Byte length of the RST option column on `line`, including leading
/// indent and the two-or-more spaces (or EOL) after the last option.
/// Docutils `Body.option_marker`: `-a`, `--long`, `--input=file`, `/V`.
pub(crate) fn rst_option_column_len(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    let t = &line[indent..];
    OPTION_MARKER_RE.find(t).map(|m| indent + m.end())
}

/// Byte length of one compact RST list opener at the start of `t`
/// (no leading indent). Docutils `Body.patterns` bullet is
/// `[-+*\u2022\u2023\u2043]( +|$)`. ASCII `*` / `-` match a trailing
/// space or EOL. Unicode `•` / `‣` / `⁃` match the same. Lone `+` is a
/// grid-table leftover fragment, not a list opener (GitHub #324).
fn rst_one_list_marker_len(t: &str) -> Option<usize> {
    if t.starts_with("* ") || t.starts_with("- ") {
        return Some(2);
    }
    if t == "*" || t == "-" {
        return Some(1);
    }
    if let Some(after) = t.strip_prefix("+ ") {
        if after.starts_with('-') || after.starts_with('+') {
            return None;
        }
        return Some(2);
    }
    for bullet in ["•", "‣", "⁃"] {
        if t == bullet {
            return Some(bullet.len());
        }
        if t.starts_with(bullet) && t[bullet.len()..].starts_with(' ') {
            return Some(bullet.len() + 1);
        }
    }
    rst_enumerator_marker_len(t)
}

/// Empty ASCII `-`/`*` at EOL and unicode Docutils bullets interrupt an
/// open paragraph. Trailing-space ASCII `* ` / `- ` / `+ ` still follow
/// GitHub #134 (GitHub #324).
fn rst_docutils_bullet_interrupts_paragraph(line: &str) -> bool {
    let t = line.trim_start();
    t == "-" || t == "*" || t.starts_with('•') || t.starts_with('‣') || t.starts_with('⁃')
}

/// Byte length of a compact RST list opener on `line`, including the
/// trailing space: `* `, `- `, `+ ` (not a `+--+` table rule), empty
/// `-`/`*` at EOL, unicode `•`/`‣`/`⁃` plus space or EOL, or a
/// Docutils enumerator (`1.`, `a.`, `i.`, `#.`, `1)`, `(1)`) plus the
/// following space (GitHub #91, #324).
///
/// Same-line nested markers (`- - item`, `- 1. item`) are consumed so
/// the hang is the inner item width. A two-space continuation after
/// `- - ` is a sibling; four spaces stay inside the inner item
/// (GitHub #125).
///
/// Extra spaces after that compact opener (`*  Candidate:`) stay in the
/// hang. Shrinking only the marker to `* Candidate:` leaves a
/// three-space body and child, which Docutils treats as a block quote
/// (GitHub #133).
pub(crate) fn rst_list_marker_len(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start().len();
    let mut pos = indent;
    let mut found = false;
    while let Some(n) = rst_one_list_marker_len(&line[pos..]) {
        pos += n;
        found = true;
    }
    if found {
        while pos < line.len() && line.as_bytes()[pos] == b' ' {
            pos += 1;
        }
    }
    found.then_some(pos)
}

/// Length of a Docutils enumerator plus trailing space, or the marker
/// alone at EOL. Sequences: arabic, single-letter alpha, roman, `#`.
/// Suffixes: `.`, `)`, `(n)`.
fn rst_enumerator_marker_len(t: &str) -> Option<usize> {
    let bytes = t.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let paren = bytes[0] == b'(';
    let mut i = usize::from(paren);
    let enum_start = i;
    // Alpha/roman at EOL (`  A.`) must not steal a hung one-letter sentence.
    // Digits and `#` keep the pre-#91 EOL form (`1.`, `#.`).
    let mut need_space = false;
    if i < bytes.len() && bytes[i] == b'#' {
        i += 1;
    } else if i < bytes.len() && bytes[i].is_ascii_digit() {
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    } else if i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        let upper = bytes[i].is_ascii_uppercase();
        let letter_start = i;
        while i < bytes.len()
            && bytes[i].is_ascii_alphabetic()
            && bytes[i].is_ascii_uppercase() == upper
        {
            i += 1;
        }
        let seq = &t[letter_start..i];
        if seq.len() > 1 && !is_rst_roman(seq) {
            return None;
        }
        need_space = true;
    } else {
        return None;
    }
    if i == enum_start {
        return None;
    }
    if paren {
        if bytes.get(i) != Some(&b')') {
            return None;
        }
        i += 1;
    } else if matches!(bytes.get(i), Some(b'.') | Some(b')')) {
        i += 1;
    } else {
        return None;
    }
    if matches!(bytes.get(i), Some(b' ')) {
        Some(i + 1)
    } else if i == t.len() && !need_space {
        Some(i)
    } else {
        None
    }
}

/// Docutils roman enumerator: one case, subtractive 1..=3999 (`i`, `iv`, `xii`).
fn is_rst_roman(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    let upper = bytes[0].is_ascii_uppercase();
    if !bytes.iter().all(|&b| {
        b.is_ascii_alphabetic()
            && b.is_ascii_uppercase() == upper
            && matches!(
                b.to_ascii_uppercase(),
                b'I' | b'V' | b'X' | b'L' | b'C' | b'D' | b'M'
            )
    }) {
        return false;
    }
    rst_roman_ok(&s.to_ascii_uppercase())
}

fn rst_roman_ok(u: &str) -> bool {
    let b = u.as_bytes();
    let mut i = 0;
    i = take_roman_repeat(b, i, b'M', 3);
    i = take_roman_place(b, i, b'C', b'D', b'M');
    i = take_roman_place(b, i, b'X', b'L', b'C');
    i = take_roman_place(b, i, b'I', b'V', b'X');
    i == b.len()
}

fn take_roman_repeat(b: &[u8], i: usize, ch: u8, max: usize) -> usize {
    let mut n = 0;
    let mut j = i;
    while j < b.len() && b[j] == ch && n < max {
        j += 1;
        n += 1;
    }
    j
}

fn take_roman_place(b: &[u8], i: usize, one: u8, five: u8, ten: u8) -> usize {
    if i + 1 < b.len() && b[i] == one && (b[i + 1] == ten || b[i + 1] == five) {
        return i + 2;
    }
    let mut j = i;
    if j < b.len() && b[j] == five {
        j += 1;
    }
    take_roman_repeat(b, j, one, 3)
}

/// Docutils quoted-literal quoting characters: printable 7-bit ASCII
/// except alphanumerics (same set as section adornments).
fn is_rst_quote_char(b: u8) -> bool {
    b.is_ascii_graphic() && !b.is_ascii_alphanumeric()
}

/// Quote byte when `line` is a flush quoted-literal line, else `None`.
fn rst_quoted_literal_quote(line: &str) -> Option<u8> {
    let indent = line.len() - line.trim_start().len();
    if indent > 0 {
        return None;
    }
    let b = *line.as_bytes().first()?;
    is_rst_quote_char(b).then_some(b)
}

fn rst_quoted_literal_continues(line: &str, quote: u8) -> bool {
    let indent = line.len() - line.trim_start().len();
    indent == 0 && line.as_bytes().first() == Some(&quote)
}

/// Check if a line is a section underline (2+ repeated punctuation chars).
/// Docutils `Body.patterns['line']` / `pats.nonalphanum7bit`: any printable
/// 7-bit ASCII except alphanumerics (same set as quoted-literal quotes).
/// Body.doctest wins over Body.line: prompt-only `>>>` / `>>> ` are not
/// `>` adornments. `>>>>>` (and `>>` / `>>>>`) stay underlines.
/// Body.explicit wins over Body.line: lone `..` is an empty comment
/// (GitHub #343), not a `.` section underline. `...` / `....` stay underlines.
fn is_underline(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 2 {
        return false;
    }
    if is_rst_doctest_opener(trimmed) {
        return false;
    }
    if is_rst_comment_opener(trimmed) {
        return false;
    }
    let first = trimmed.as_bytes()[0];
    is_rst_quote_char(first) && trimmed.bytes().all(|b| b == first)
}

/// RST simple-table border: `=` column groups separated by spaces
/// (`=====  =====`). A solid `=====` is a section underline, not a table.
fn is_simple_table_border(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 3 {
        return false;
    }
    let mut groups = 0u32;
    let mut in_eq = false;
    let mut saw_space_between = false;
    for b in t.bytes() {
        match b {
            b'=' => {
                if !in_eq {
                    groups += 1;
                    in_eq = true;
                }
            }
            b' ' => {
                if in_eq {
                    saw_space_between = true;
                }
                in_eq = false;
            }
            _ => return false,
        }
    }
    saw_space_between && groups >= 2
}

/// Indent of a definition immediately under a list item, if any.
///
/// Docutils treats a line indented past the item hang as a definition of
/// the item text (`  * Term` + five-space body), not a continuation
/// paragraph. A hang-width wrap (`    body` after `  * `) is not a
/// definition. A following list marker is a nested item, not a body.
fn list_item_definition_indent(lines: &[Line<'_>], i: usize, hang: usize) -> Option<usize> {
    let next = lines.get(i + 1)?.text;
    if next.trim().is_empty() {
        return None;
    }
    if rst_list_marker_len(next).is_some() {
        return None;
    }
    let next_indent = next.len() - next.trim_start().len();
    (next_indent > hang).then_some(next_indent)
}

/// Indent of the next non-blank line after `i`, if any.
fn next_nonblank_indent(lines: &[Line<'_>], i: usize) -> Option<usize> {
    for line in lines.iter().skip(i + 1) {
        if line.text.trim().is_empty() {
            continue;
        }
        return Some(line.text.len() - line.text.trim_start().len());
    }
    None
}

/// True when `lines[i]` is a definition-list term: the next physical line
/// is non-blank and indented further than this one. RST forbids a blank
/// between term and definition.
fn is_definition_term(lines: &[Line<'_>], i: usize) -> bool {
    let next = match lines.get(i + 1) {
        Some(line) => line.text,
        None => return false,
    };
    if next.trim().is_empty() {
        return false;
    }
    let line = lines[i].text;
    let indent = line.len() - line.trim_start().len();
    let next_indent = next.len() - next.trim_start().len();
    next_indent > indent
}

/// Last line of a simple table starting at `start`.
///
/// Docutils `isolate_simple_table` (states.py 1858-1898) scans to a
/// matching-width `=` border and does not stop on blanks. Close on the
/// second matching-width border, or on the first when the next line is
/// blank or the input ends. A different-width `=` border is malformed
/// and is not a closer. No matching closer: `None`, and the caller
/// keeps the top as Structure (GitHub #347 / `simple_table_top`).
fn simple_table_end(lines: &[Line<'_>], start: usize) -> Option<usize> {
    let toplen = lines[start].text.trim().len();
    let mut found = 0u32;
    let mut end = None;
    for (j, line) in lines.iter().enumerate().skip(start + 1) {
        if !is_simple_table_border(line.text) {
            continue;
        }
        if line.text.trim().len() != toplen {
            break;
        }
        found += 1;
        end = Some(j);
        let next_blank_or_eof = match lines.get(j + 1) {
            None => true,
            Some(next) => next.text.trim().is_empty(),
        };
        if found == 2 || next_blank_or_eof {
            break;
        }
    }
    end
}

/// Last line of a grid table starting at a `+---+` top.
///
/// Docutils `isolate_grid_table` keeps left-edge `+`/`|` lines and
/// closes on a later `grid_table_top`. If no closer exists, keep the
/// collected block so a truncated `+---+---+` / `| a |` pair stays
/// Structure.
fn rst_grid_table_end(lines: &[Line<'_>], start: usize) -> usize {
    let mut last = start;
    let mut last_top = start;
    for (j, line) in lines.iter().enumerate().skip(start + 1) {
        let t = line.text.trim();
        if t.is_empty() {
            break;
        }
        if !(t.starts_with('+') || t.starts_with('|')) {
            break;
        }
        last = j;
        if is_rst_grid_table_top(t) {
            last_top = j;
        }
    }
    if last_top > start { last_top } else { last }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Region;

    #[test]
    fn simple_prose() {
        let input = "Hello world. This is a test.\nAnother line here.";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Hello world.")))
        );
    }

    #[test]
    fn directive_preserved() {
        let input = "Some prose.\n\n.. code-block:: python\n\n   print('hello')\n\nMore prose.";
        let regions = RstParser.parse(input);
        let prose_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Prose(_)))
            .count();
        assert_eq!(prose_count, 2);
        // The code block surfaces as Region::Code with lang=python.
        let code = regions.iter().find_map(|r| match r {
            Region::Code { lang, body, .. } => Some((lang.clone(), body.clone())),
            _ => None,
        });
        let (lang, body) = code.expect("expected one Region::Code");
        assert_eq!(lang.as_deref(), Some("python"));
        assert!(body.contains("print('hello')"));
    }

    #[test]
    fn section_title_preserved() {
        let input = "My Title\n========\n\nSome text here.";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("My Title")))
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("====")))
        );
    }

    #[test]
    fn apostrophe_section_adornment_is_structure() {
        let input = "Input\n'''''\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Input"))),
            "title must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("'''''"))),
            "apostrophe underline must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(
                |r| matches!(r, Region::Prose(s) if s.contains("Input") || s.contains("'''''"))
            ),
            "apostrophe section must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_section_adornments_are_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        for (adornment, n) in [
            ('\'', 5),
            ('\'', 2),
            ('.', 5),
            ('_', 5),
            ('<', 5),
            ('>', 5),
            (':', 5),
            ('%', 5),
            ('@', 5),
        ] {
            let rule = adornment.to_string().repeat(n);
            let input = format!("Input\n{rule}\n");
            let out = format_text(&input, &cfg).unwrap();
            assert_eq!(
                out, input,
                "section adornment {adornment:?} x{n} must stay two lines, got:\n{out}"
            );
            assert!(
                !out.contains(&format!("Input {rule}")),
                "must not glue {adornment:?} adornment onto title, got:\n{out}"
            );
        }
    }

    /// GitHub #100 / snapper-j0ig: `:` is Docutils nonalphanum7bit.
    #[test]
    fn colon_section_adornment_is_structure() {
        let input = "Title\n:::::\n\nNext paragraph. Second sentence.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Title"))),
            "title must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(":::::"))),
            "colon underline must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(
                |r| matches!(r, Region::Prose(s) if s.contains("Title") || s.contains(":::::"))
            ),
            "colon section must not be Prose, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("Next paragraph.") && s.contains("Second sentence.")
            )),
            "following paragraph must stay Prose, got {regions:?}"
        );
    }

    #[test]
    fn colon_section_fixture_is_identity_and_prose_still_splits() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "Title\n:::::\n\nNext paragraph. Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, "Title\n:::::\n\nNext paragraph.\nSecond sentence.\n",
            "colon adornment stays; following prose still splits, got:\n{out}"
        );
        assert!(
            !out.contains("Title :::::"),
            "must not glue ::::: onto the title, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &cfg).unwrap(),
            out,
            "colon section fixture must be identity, got:\n{out}"
        );

        let titled = "Title one. Title two.\n:::::\n\nNext paragraph. Second sentence.\n";
        let titled_out = format_text(titled, &cfg).unwrap();
        assert_eq!(
            titled_out, "Title one. Title two.\n:::::\n\nNext paragraph.\nSecond sentence.\n",
            "multi-sentence title above ::::: must stay one line, got:\n{titled_out}"
        );
    }

    #[test]
    fn is_underline_accepts_docutils_nonalphanum7bit() {
        assert!(is_underline(":::::"));
        assert!(is_underline("::::: "));
        assert!(is_underline("%%%%%"));
        assert!(is_underline("@@@@@"));
        assert!(is_underline("$$$$$"));
        assert!(is_underline("!!!!!"));
        assert!(is_underline("?????"));
        assert!(is_underline(";;;;;"));
        assert!(is_underline("/////"));
        assert!(is_underline(&"\\".repeat(5)));
        assert!(is_underline("{{{{{"));
        assert!(is_underline("}}}}}"));
        assert!(is_underline("[[[[["));
        assert!(is_underline("]]]]]"));
        assert!(is_underline("|||||"));
        assert!(is_underline("&&&&&"));
        assert!(is_underline("((((("));
        assert!(is_underline(")))))"));
        assert!(is_underline(",,,,,"));
        assert!(is_underline("`````"));
        assert!(!is_underline("aaaaa"));
        assert!(!is_underline("11111"));
        assert!(!is_underline(":"));
        assert!(!is_underline(":::-"));
    }

    #[test]
    fn literal_block_preserved() {
        let input = "Example::\n\n   some code\n   more code\n\nBack to prose.";
        let regions = RstParser.parse(input);
        let structure_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Structure(_)))
            .count();
        assert!(structure_count >= 3);
    }

    #[test]
    fn quoted_literal_block_lines_are_structure() {
        let input = concat!(
            "Take it literally::\n",
            "\n",
            "> if literal_block:\n",
            ">     text = 'is left as-is'\n",
            ">     markup_processing = None\n",
        );
        let regions = RstParser.parse(input);
        for needle in [
            "> if literal_block:",
            ">     text = 'is left as-is'",
            ">     markup_processing = None",
        ] {
            assert!(
                regions
                    .iter()
                    .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
                "quoted literal line {needle:?} must be Structure, got {regions:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(s) if s.contains(needle))),
                "quoted literal line {needle:?} must not be Prose, got {regions:?}"
            );
        }
    }

    #[test]
    fn reporter_quoted_literal_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            "Take it literally::\n",
            "\n",
            "> if literal_block:\n",
            ">     text = 'is left as-is'\n",
            ">     markup_processing = None\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "quoted literal after :: must stay unjoined, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &cfg).unwrap(),
            out,
            "hung quoted literal must be identity, got:\n{out}"
        );
    }

    #[test]
    fn field_list_preserved() {
        let input = ":Author: Someone\n:Date: 2026\n\nParagraph text.";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Author")))
        );
    }

    #[test]
    fn field_list_line_rejects_interpreted_text_role() {
        assert!(is_rst_field_list_line(":Author: Someone"));
        assert!(is_rst_field_list_line(":class: test"));
        assert!(is_rst_field_list_line(":name:"));
        assert!(is_rst_field_list_line(":name: "));
        assert!(is_rst_field_list_line(
            ":py:mod: some.module.Name is here. Second sentence."
        ));
        assert!(is_rst_field_list_line(r":foo\:bar: value"));
        assert!(!is_rst_field_list_line(
            ":class:`CloudDatabase` exceeds a rate"
        ));
        assert!(!is_rst_field_list_line(":py:class:`CloudDatabase`"));
        assert!(!is_rst_field_list_line(":role:`text`"));
        assert!(!is_rst_field_list_line("::"));
        assert!(!is_rst_field_list_line(": name:"));
        assert!(!is_rst_field_list_line("Hello"));
        assert_eq!(
            rst_field_marker_len(":py:mod: some.module.Name is here."),
            Some(9)
        );
    }

    #[test]
    fn role_continuation_joins_list_item_prose() {
        let input = concat!(
            "* TooManyRequests is returned when a\n",
            "  :class:`CloudDatabase` exceeds a configured request rate\n",
            "  limit. Set requests_per_second_limit to 0 for every request.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "* ")),
            "marker must be Structure, got {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            [
                "TooManyRequests is returned when a :class:`CloudDatabase` exceeds a configured request rate limit. Set requests_per_second_limit to 0 for every request."
            ],
            "role continuation must join the item, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(":class:`CloudDatabase`")
            )),
            "role must not be a field-list Structure, got {regions:?}"
        );
    }

    #[test]
    fn reporter_role_continuation_second_sentence_hangs() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            "* TooManyRequests is returned when a\n",
            "  :class:`CloudDatabase` exceeds a configured request rate\n",
            "  limit. Set requests_per_second_limit to 0 for every request.\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                "* TooManyRequests is returned when a :class:`CloudDatabase` exceeds a configured request rate limit.\n",
                "  Set requests_per_second_limit to 0 for every request.\n",
            ),
            "second sentence must hang at `* ` width, got:\n{out}"
        );
        assert!(
            out.contains("\n  Set requests_per_second_limit"),
            "second sentence must keep two-space hang, got:\n{out}"
        );
        assert!(
            !out.contains("\nSet requests_per_second_limit"),
            "second sentence must not outdent to column 0, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung role continuation must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn adjacent_bullet_items_stay_separate_regions() {
        let input = "Features:\n\n* First item\n* Second item\n* Third item\n";
        let regions = RstParser.parse(input);
        let bullets: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) if s.contains("item") => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            bullets.len(),
            3,
            "each bullet must be its own Prose region, got {regions:?}"
        );
    }

    #[test]
    fn simple_table_rows_are_structure() {
        let input = "=====  =====\nName   Value\n=====  =====\nA      B\n=====  =====\n";
        let regions = RstParser.parse(input);
        let structure: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Structure(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            structure.iter().any(|s| s.contains("Name")),
            "header row must be Structure, got {regions:?}"
        );
        assert!(
            structure.iter().any(|s| s.contains("A")),
            "body row must be Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Name") || s.contains("A"))),
            "table rows must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_simple_table_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "=====  =====\nName   Value\n=====  =====\nA      B\n=====  =====\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input, "simple table must stay identity, got:\n{out}");
    }

    /// GitHub #99 / snapper-8n65: interior blank must not end the table.
    fn interior_blank_simple_table() -> &'static str {
        concat!(
            "=====  =====\n",
            "Name   Value\n",
            "=====  =====\n",
            "A      first\n",
            "\n",
            "       more\n",
            "=====  =====\n",
        )
    }

    #[test]
    fn simple_table_interior_blank_rows_are_structure() {
        let input = interior_blank_simple_table();
        let regions = RstParser.parse(input);
        for needle in [
            "Name   Value",
            "A      first",
            "       more",
            "=====  =====",
        ] {
            assert!(
                regions
                    .iter()
                    .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
                "table line {needle:?} must be Structure, got {regions:?}"
            );
        }
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("Name")
                        || s.contains("first")
                        || s.contains("more")
                        || s.contains("=====")
            )),
            "table lines must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn simple_table_interior_blank_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = interior_blank_simple_table();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "interior-blank simple table must stay identity, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &cfg).unwrap(),
            out,
            "interior-blank simple table must be identity, got:\n{out}"
        );
    }

    /// GitHub #174 / snapper-4a31: `| ` is a line block, not a grid row.
    fn line_block_fixture() -> &'static str {
        concat!(
            "| This is a line. Another sentence.\n",
            "| Next line. More text.\n",
        )
    }

    #[test]
    fn line_block_marker_len_matches_docutils() {
        assert_eq!(rst_line_block_marker_len("| "), Some(2));
        assert_eq!(rst_line_block_marker_len("| This is a line."), Some(2));
        assert_eq!(rst_line_block_marker_len("|   Nested."), Some(4));
        assert_eq!(rst_line_block_marker_len("  | hung."), Some(4));
        assert_eq!(rst_line_block_marker_len("|"), Some(1));
        assert_eq!(rst_line_block_marker_len("|cell"), None);
        assert_eq!(
            rst_line_block_marker_len("|version| is the current release."),
            None
        );
        assert_eq!(rst_line_block_marker_len("|version|."), None);
        assert_eq!(rst_line_block_marker_len("+---+---+"), None);
    }

    #[test]
    fn line_block_marker_is_structure_body_is_prose() {
        let regions = RstParser.parse(line_block_fixture());
        let structure: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Structure(s) if s.contains('|') => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            structure.iter().any(|s| s == &"| " || s.starts_with("| ")),
            "| plus spaces must be Structure, got {regions:?}"
        );
        assert!(
            !structure.iter().any(|s| s.contains("This is a line")),
            "line-block body must not be in the | Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("This is a line"))),
            "line-block body must be Prose, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Another sentence"))),
            "second sentence must stay Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("This is a line") || s.contains("Another sentence")
            )),
            "line-block sentences must not be full-line Structure, got {regions:?}"
        );
    }

    /// GitHub #409 / snapper-zy5o: a line-block ends at the first flush
    /// line that is not `| ` and not a hang. After. is a new paragraph.
    fn line_block_flush_prose_fixture() -> &'static str {
        concat!("| First line. Second line.\n", "After. Next.\n",)
    }

    #[test]
    fn line_block_does_not_swallow_following_flush_prose() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = line_block_flush_prose_fixture();
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "| ")),
            "| plus space must stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("First line.") && s.contains("Second line.")
            )),
            "line-block body must stay Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("First line.") && s.contains("After.")
            )),
            "After. must not join the line-block body, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("After.") && s.contains("Next.")
            )),
            "After. / Next. must stay Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("After.") || s.contains("Next.")
            )),
            "After. / Next. must not be Structure, got {regions:?}"
        );

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!("| First line.\n", "  Second line.\n", "After.\n", "Next.\n",),
            "line-block splits; After. stays flush and splits, got:\n{out}"
        );
        assert!(
            !out.contains("\n  After."),
            "After. must not inherit the line-block hang, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn grid_table_rows_stay_full_line_structure() {
        let input = "+---+---+\n| a | b |\n+---+---+\n";
        let regions = RstParser.parse(input);
        for needle in ["+---+---+", "| a | b |"] {
            assert!(
                regions
                    .iter()
                    .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
                "grid line {needle:?} must be Structure, got {regions:?}"
            );
        }
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("a") || s.contains("---+---+")
            )),
            "grid table must not be Prose, got {regions:?}"
        );
    }

    /// snapper-t0th: Inliner substitution_ref is inline prose, not a table.
    fn substitution_ref_fixture() -> &'static str {
        concat!(
            "|version| is the current release. Second sentence.\n",
            "\n",
            "The current release is\n",
            "|version|. Next sentence.\n",
        )
    }

    #[test]
    fn substitution_ref_is_prose_not_structure() {
        let regions = RstParser.parse(substitution_ref_fixture());
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("|version|")
            )),
            "|version| must not be leftover Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("|version| is the current release.")
                        && s.contains("Second sentence.")
            )),
            "lead |version| sentence must stay Prose, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("The current release is")
                        && s.contains("|version|.")
                        && s.contains("Next sentence.")
            )),
            "mid-paragraph |version| must stay Prose, got {regions:?}"
        );
    }

    #[test]
    fn substitution_ref_interior_punct_stays_prose() {
        // snapper-15i9 / GitHub #233: Inliner leftover. `|fig. 1|` is
        // still Prose (t0th); the period is not a region bound.
        let input = "See |fig. 1| in the caption. Next sentence.\n";
        let regions = RstParser.parse(input);
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("|fig. 1|")
            )),
            "|fig. 1| must stay Prose not Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("|fig. 1|") && s.contains("Next sentence.")
            )),
            "|fig. 1| and the next sentence must stay Prose, got {regions:?}"
        );
    }

    #[test]
    fn leftover_plus_fragment_stays_structure() {
        let input = "+===+===+\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("+===+===+"))),
            "leftover + fragment must stay Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains('+'))),
            "leftover + fragment must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn definition_list_term_and_body_are_structure() {
        let input = "Term\n   Definition sentence.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Term"))),
            "term must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Definition"))),
            "definition must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(r, Region::Prose(s) if s.contains("Term") || s.contains("Definition"))
            }),
            "definition list must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_definition_list_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "Term\n   Definition sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "definition list must stay identity, got:\n{out}"
        );
        assert!(
            out.starts_with("Term\n"),
            "term must stay at column 0, got:\n{out}"
        );
        assert!(
            out.contains("\n   Definition sentence."),
            "definition must keep indent, got:\n{out}"
        );
    }

    #[test]
    fn nested_list_item_overindent_is_definition_structure() {
        let input = concat!(
            "* Parent\n",
            "\n",
            "  * Check all paths.\n",
            "     If a node is visited again, push it.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| { matches!(r, Region::Structure(s) if s.contains("Check all paths.")) }),
            "over-indented term must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| {
                matches!(
                    r,
                    Region::Structure(s) if s.contains("If a node is visited again, push it.")
                )
            }),
            "five-space definition must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(
                    r,
                    Region::Prose(s)
                        if s.contains("Check all paths.")
                            || s.contains("If a node is visited again")
                )
            }),
            "definition inside a list item must not join as Prose, got {regions:?}"
        );
    }

    /// GitHub #134 / snapper-7rgg: `* ` after a paragraph with no blank
    /// is still that paragraph, not a list.
    fn compact_listlike_paragraph() -> &'static str {
        concat!(
            "Topics include:\n",
            "* Product requirements - what pages exist? What functionality lives on them?\n",
            "* Technical requirements\n",
        )
    }

    #[test]
    fn compact_listlike_prose_joins_into_one_paragraph() {
        let input = compact_listlike_paragraph();
        let regions = RstParser.parse(input);
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            [concat!(
                "Topics include: * Product requirements - what pages exist? What functionality lives on them?\n",
                "* Technical requirements",
            )],
            "compact list-like lines must stay one Prose paragraph, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains('*'))),
            "must not invent a list marker Structure, got {regions:?}"
        );
    }

    #[test]
    fn compact_listlike_prose_does_not_invent_a_list() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = compact_listlike_paragraph();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                "Topics include: * Product requirements - what pages exist?\n",
                "What functionality lives on them?\n",
                "* Technical requirements\n",
            ),
            "must stay one paragraph and not hang a list, got:\n{out}"
        );
        assert!(
            !out.contains("\n  What functionality"),
            "must not invent list hang, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "compact list-like paragraph must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn hang_after_nested_pull_quote_is_structure() {
        let input = concat!(
            "Loading-state race:\n",
            "\n",
            "   Ask this question:\n",
            "\n",
            "   .. pull-quote::\n",
            "\n",
            "      What happens?\n",
            "\n",
            "   First sentence.\n",
            "   Second sentence.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(".. pull-quote::"))),
            "nested directive must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
            "definition hang after the directive must be Structure, got {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            prose
                .iter()
                .any(|s| s.contains("First sentence.") && s.contains("Second sentence.")),
            "definition sentences after the directive must be one Prose, got {regions:?}"
        );
        assert!(
            prose.iter().any(|s| s.contains("What happens?")),
            "pull-quote body must hang as Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("What happens?")
            )),
            "pull-quote body must not freeze as Structure, got {regions:?}"
        );
    }

    #[test]
    fn adjacent_rst_lists_are_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        for input in [
            "Features:\n\n* First item\n* Second item\n* Third item\n",
            "Features:\n\n- First item\n- Second item\n",
            "Features:\n\n+ First item\n+ Second item\n",
            "Features:\n\n1. First item\n2. Second item\n",
            "Features:\n\n#. First item\n#. Second item\n",
        ] {
            let out = format_text(input, &cfg).unwrap();
            assert_eq!(out, input, "list must stay compact, got:\n{out}");
        }
    }

    #[test]
    fn list_continuation_paragraph_keeps_hang_structure() {
        let input = "- First sentence.\n\n  Second sentence.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "- ")),
            "marker must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| { matches!(r, Region::Prose(s) if s.contains("First sentence.")) }),
            "item text must be Prose, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "continuation hang must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| { matches!(r, Region::Prose(s) if s.contains("Second sentence.")) }),
            "continuation text must be Prose, got {regions:?}"
        );
    }

    #[test]
    fn compact_list_hang_joins_into_one_prose_region() {
        let input = "* One.\n  Two.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "* ")),
            "marker must be Structure, got {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            ["One.\nTwo."],
            "compact hang must join into one Prose, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "compact hang must not be Structure, got {regions:?}"
        );
    }

    #[test]
    fn compact_abbrev_list_hang_joins_into_one_prose_region() {
        let input = "* **Answer**: No.\n  A second sentence.\n";
        let regions = RstParser.parse(input);
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            ["**Answer**: No.\nA second sentence."],
            "compact abbrev hang must join into one Prose, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "compact abbrev hang must not be Structure, got {regions:?}"
        );
    }

    #[test]
    fn compact_quoted_list_hang_joins_into_one_prose_region() {
        let input = "* \"First sentence.\n  Second sentence.\"\n";
        let regions = RstParser.parse(input);
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            ["\"First sentence.\nSecond sentence.\""],
            "compact open-quote hang must join into one Prose, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "compact open-quote hang must not be Structure, got {regions:?}"
        );
    }

    #[test]
    fn compact_open_literal_list_hang_joins_into_one_prose_region() {
        let input = concat!(
            "- The error is ``not valid.\n",
            "  Choose from: one, two`` and continue.\n",
            "  A separate sentence.\n",
        );
        let regions = RstParser.parse(input);
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            [
                "The error is ``not valid.\nChoose from: one, two`` and continue.\nA separate sentence."
            ],
            "compact open-literal hang must join into one Prose, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "compact open-literal hang must not be Structure, got {regions:?}"
        );
    }

    #[test]
    fn compact_open_paren_list_hang_joins_into_one_prose_region() {
        let input = concat!(
            "- Ordering (smaller indices mean higher priority.\n",
            "  Recurse to the left side of the array)\n",
        );
        let regions = RstParser.parse(input);
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            [
                "Ordering (smaller indices mean higher priority.\nRecurse to the left side of the array)"
            ],
            "compact open-paren hang must join into one Prose, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "compact open-paren hang must not be Structure, got {regions:?}"
        );
    }

    #[test]
    fn reporter_open_quote_list_continuation_keeps_two_space_hang() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = "* \"First sentence.\n  Second sentence.\"\n* Next item.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "open-quote list continuation must keep two-space hang, got:\n{out}"
        );
        assert!(
            out.contains("\n  Second sentence.\""),
            "continuation must stay hung, got:\n{out}"
        );
        assert!(
            !out.contains("\nSecond sentence."),
            "must not outdent inside the open quote, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung open-quote list must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn reporter_open_literal_list_continuation_keeps_two_space_hang() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = concat!(
            "- The error is ``not valid.\n",
            "  Choose from: one, two`` and continue.\n",
            "  A separate sentence.\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "open-literal list continuation must keep two-space hang, got:\n{out}"
        );
        assert!(
            out.contains("\n  Choose from: one, two`` and continue."),
            "continuation inside the open literal must stay hung, got:\n{out}"
        );
        assert!(
            out.contains("\n  A separate sentence."),
            "following sentence in the same item must stay hung, got:\n{out}"
        );
        assert!(
            !out.contains("\nChoose from: one, two"),
            "must not outdent inside the open inline literal, got:\n{out}"
        );
        assert!(
            !out.contains("\nA separate sentence."),
            "must not outdent the following sentence, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung open-literal list must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn reporter_open_paren_list_continuation_keeps_two_space_hang() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = concat!(
            "- Ordering (smaller indices mean higher priority.\n",
            "  Recurse to the left side of the array)\n",
            "- Next item.\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "open-paren list continuation must keep two-space hang, got:\n{out}"
        );
        assert!(
            out.contains("\n  Recurse to the left side of the array)"),
            "continuation must stay hung, got:\n{out}"
        );
        assert!(
            !out.contains("\nRecurse to the left side of the array)"),
            "must not outdent inside the open parenthesis, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung open-paren list must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn reporter_abbrev_list_continuation_keeps_two_space_hang() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            "* **Answer**: No.\n",
            "  A second sentence.\n",
            "* Uses queues, caches, etc.\n",
            "  Another sentence.\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "list continuation after No./etc. must keep two-space hang, got:\n{out}"
        );
        assert!(
            out.contains("\n  A second sentence."),
            "No. continuation must hang, got:\n{out}"
        );
        assert!(
            out.contains("\n  Another sentence."),
            "etc. continuation must hang, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung abbrev list must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn starred_quote_list_item_is_idempotent_and_oracle() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = "* a*'*'. A.";
        let out = format_text(input, &cfg).expect("format_text");
        let twice = format_text(&out, &cfg).expect("second pass");
        assert_eq!(
            out, twice,
            "not idempotent:\n first={out:?}\n second={twice:?}"
        );
        assert_eq!(
            out, "* a*'*'.\n  A.",
            "list wrap must hang at marker width, got:\n{out}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn reporter_list_continuation_paragraph_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "- First sentence.\n\n  Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "list continuation paragraph must stay identity, got:\n{out}"
        );
        assert!(
            out.contains("\n  Second sentence."),
            "continuation must keep two-space hang, got:\n{out}"
        );
    }

    #[test]
    fn reporter_enumerated_item_second_sentence_hangs_at_marker_width() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "#. First sentence. Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, "#. First sentence.\n   Second sentence.\n",
            "second sentence must hang at `#. ` width, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung enumerated item must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );

        // Bullet hang from #51 stays.
        let bullet = "* First sentence. Second sentence.\n";
        let bullet_out = format_text(bullet, &cfg).unwrap();
        assert_eq!(
            bullet_out, "* First sentence.\n  Second sentence.\n",
            "bullet hang must stay, got:\n{bullet_out}"
        );
        let bullet_twice = format_text(&bullet_out, &cfg).unwrap();
        assert_eq!(
            bullet_out, bullet_twice,
            "hung bullet must be identity, got:\n{bullet_twice}"
        );
        let blank_hang = "- First sentence.\n\n  Second sentence.\n";
        let blank_out = format_text(blank_hang, &cfg).unwrap();
        assert_eq!(
            blank_out, blank_hang,
            "list continuation hang from #51 must stay, got:\n{blank_out}"
        );
    }

    #[test]
    fn nested_same_line_list_marker_is_inner_hang_structure() {
        let input =
            "- - Document typed fixture exports.\n    The package already includes ``py.typed``.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "- - ")),
            "inner hang must be one Structure of width 4, got {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            ["Document typed fixture exports.\nThe package already includes ``py.typed``."],
            "four-space continuation must join the inner item, got {regions:?}"
        );
    }

    #[test]
    fn two_space_bullet_marker_is_width_three_structure() {
        let input = concat!(
            "*  Candidate:\n",
            "\n",
            "   Search API.\n",
            "\n",
            "   * Child item.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "*  ")),
            "two-space bullet must be Structure of width 3, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "* ")),
            "must not shrink the marker to one space, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("Candidate:"))),
            "item text must be Prose, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "   * ")),
            "child bullet must keep three-space indent, got {regions:?}"
        );
    }

    #[test]
    fn comment_opener_and_body_are_structure() {
        let input = "..\n   First sentence.\n   Second sentence.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "..")),
            "bare .. must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("First sentence."))),
            "comment body must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Second sentence."))),
            "comment body must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(
                    r,
                    Region::Prose(s)
                        if s.contains("First sentence.") || s.contains("Second sentence.")
                )
            }),
            "comment body must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_comment_body_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "..\n   First sentence.\n   Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "reporter comment body must stay identity under format, got:\n{out}"
        );
        assert!(
            out.contains("\n   First sentence."),
            "first comment line must keep indent, got:\n{out}"
        );
        assert!(
            out.contains("\n   Second sentence."),
            "second comment line must keep indent, got:\n{out}"
        );
    }

    #[test]
    fn bare_dotdot_is_comment_opener() {
        assert!(is_rst_comment_opener(".."));
        assert!(is_rst_comment_opener(".. This is a comment."));
        assert!(is_rst_comment_opener("..\tThis is a comment."));
        assert!(!is_rst_comment_opener(".. note::"));
        assert!(!is_rst_comment_opener("..."));
        assert!(!is_rst_comment_opener("Hello"));
        assert!(!is_rst_comment_opener(".. [1]"));
        assert!(!is_rst_comment_opener(".. [#]"));
        assert!(!is_rst_comment_opener(".. [*]"));
        assert!(!is_rst_comment_opener(".. [CIT2002]"));
        assert!(is_rst_comment_opener(".. [not closed"));
        assert!(is_rst_comment_opener(".. [1]no-space"));
        assert!(is_rst_comment_opener(".. [not a footnote]"));
    }

    #[test]
    fn footnote_citation_marker_follows_docutils() {
        assert_eq!(rst_footnote_citation_marker_len(".. [1] "), Some(7));
        assert_eq!(rst_footnote_citation_marker_len(".. [#] "), Some(7));
        assert_eq!(rst_footnote_citation_marker_len(".. [*] "), Some(7));
        assert_eq!(rst_footnote_citation_marker_len(".. [CIT2002] "), Some(13));
        assert_eq!(rst_footnote_citation_marker_len(".. [1]"), Some(6));
        assert_eq!(rst_footnote_citation_marker_len("  .. [1] text"), Some(9));
        assert_eq!(rst_footnote_citation_marker_len(".. [#note] x"), Some(11));
        assert_eq!(rst_footnote_citation_marker_len("..[1] text"), None);
        assert_eq!(rst_footnote_citation_marker_len(".. [1]no-space"), None);
        assert_eq!(rst_footnote_citation_marker_len(".. [not closed"), None);
        assert_eq!(
            rst_footnote_citation_marker_len(".. This is a comment."),
            None
        );
        assert_eq!(
            rst_footnote_citation_marker_len(".. [not a footnote]"),
            None
        );
    }

    #[test]
    fn dropped_comment_excludes_targets_and_footnotes() {
        assert!(is_rst_dropped_comment_opener(".."));
        assert!(is_rst_dropped_comment_opener(".. This is a comment."));
        assert!(source_has_dropped_rst_comments(
            "..\n   First sentence.\n   Second sentence.\n"
        ));
        assert!(!is_rst_dropped_comment_opener(".. _label:"));
        assert!(!is_rst_dropped_comment_opener(".. [1]"));
        assert!(!is_rst_dropped_comment_opener(".. note::"));
        assert!(!source_has_dropped_rst_comments(
            "Hello world. Second sentence.\n\n.. _label:\n\n.. note::\n   Body.\n"
        ));
    }

    #[test]
    fn recognized_comment_line_keeps_indented_body() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = ".. This is a comment.\n   First sentence.\n   Second sentence.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "recognized comment line must keep indented body identity, got:\n{out}"
        );
        assert!(
            out.contains("\n   First sentence."),
            "first comment body line must keep indent, got:\n{out}"
        );
        assert!(
            out.contains("\n   Second sentence."),
            "second comment body line must keep indent, got:\n{out}"
        );
    }

    /// GitHub #176 / snapper-k7bb ticket fixture.
    fn comment_blank_then_quote_fixture() -> &'static str {
        concat!(
            ".. Comment sentence. Still comment.\n",
            "\n",
            "    This is a block quote. Another sentence.\n",
        )
    }

    #[test]
    fn comment_blank_closes_later_indent_is_hung_quote() {
        let regions = RstParser.parse(comment_blank_then_quote_fixture());
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(".. Comment sentence. Still comment.")
            )),
            "comment opener must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(r, Region::BlankLines(_))),
            "blank must close the comment, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a block quote")
            )),
            "post-blank indent must not stay comment Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("This is a block quote.") && s.contains("Another sentence.")
            )),
            "post-blank indent must be hung Prose, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "    ")),
            "quote hang spaces must be Structure, got {regions:?}"
        );
    }

    #[test]
    fn comment_blank_then_quote_splits_and_hangs() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(comment_blank_then_quote_fixture(), &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                ".. Comment sentence. Still comment.\n",
                "\n",
                "    This is a block quote.\n",
                "    Another sentence.\n",
            ),
            "blank must close the comment; quote must hang and split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    /// Ticket fixture (Format::Rst): container body hangs.
    fn note_container_fixture() -> &'static str {
        concat!(
            ".. note::\n",
            "\n",
            "   This is a long note sentence that must reflow. Second sentence.\n",
            "\n",
            "After the note. Next.\n",
        )
    }

    #[test]
    fn container_directive_option_is_structure_body_is_prose() {
        let input = concat!(
            ".. note::\n",
            "  :class: test\n",
            "\n",
            "  This is a long note sentence that must reflow. Second sentence.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(".. note::"))),
            "directive opener must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(":class: test"))),
            "two-space option must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("This is a long note sentence that must reflow.")
                        && s.contains("Second sentence.")
            )),
            "container body must be hung Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long note sentence")
            )),
            "container body must not freeze as Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  ")),
            "container body hang must be Structure, got {regions:?}"
        );
    }

    #[test]
    fn opaque_directive_body_stays_structure() {
        for input in [
            ".. raw:: html\n\n  <p>This is a long note sentence that must reflow. Second sentence.</p>\n",
            ".. include:: foo.rst\n  :start-after: Body.\n",
            ".. csv-table:: Title\n  :header: \"a\", \"b\"\n\n  \"1\", \"2\"\n",
        ] {
            let regions = RstParser.parse(input);
            assert!(
                !regions.iter().any(|r| matches!(
                    r,
                    Region::Prose(s)
                        if s.contains("This is a long")
                            || s.contains("start-after")
                            || s.contains("\"1\", \"2\"")
                )),
                "opaque body must not be Prose, got {regions:?} for {input:?}"
            );
        }
    }

    #[test]
    fn reporter_two_space_container_option_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = ".. note::\n  :class: test\n\n  Body.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "single-sentence container body must stay identity under format, got:\n{out}"
        );
        assert!(
            out.contains("\n  :class: test"),
            "option must keep two-space indent, got:\n{out}"
        );
        assert!(
            out.contains("\n  Body."),
            "body must keep two-space indent, got:\n{out}"
        );
    }

    #[test]
    fn note_container_fixture_body_hangs_and_splits() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = note_container_fixture();
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(".. note::"))),
            "note opener must stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("This is a long note sentence that must reflow.")
                        && s.contains("Second sentence.")
            )),
            "note body must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long note sentence")
            )),
            "note body must not freeze as Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("After the note.") && s.contains("Next.")
            )),
            "prose after the note must remain Prose, got {regions:?}"
        );

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                ".. note::\n",
                "\n",
                "   This is a long note sentence that must reflow.\n",
                "   Second sentence.\n",
                "\n",
                "After the note.\n",
                "Next.\n",
            ),
            "note body must hang and split; following prose must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    /// Ticket fixture (Format::Rst / GitHub #344): compact note + flush
    /// paragraph. After markup must not join the hung body.
    fn compact_note_flush_fixture() -> &'static str {
        concat!(
            "Intro sentence here. Another intro sentence.\n",
            ".. note::\n",
            "   Body here. Second body.\n",
            "After markup. Next sentence.\n",
        )
    }

    #[test]
    fn compact_note_flush_paragraph_stays_unindented() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = compact_note_flush_fixture();
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(".. note::"))),
            "note opener must stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("Body here.") && s.contains("Second body.")
            )),
            "note body must hang as Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("Body here.") && s.contains("After markup.")
            )),
            "After markup must not join the note body, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("After markup.") && s.contains("Next sentence.")
            )),
            "After markup must stay unindented Prose, got {regions:?}"
        );

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                "Intro sentence here.\n",
                "Another intro sentence.\n",
                ".. note::\n",
                "   Body here.\n",
                "   Second body.\n",
                "After markup.\n",
                "Next sentence.\n",
            ),
            "note body hangs; After markup stays flush and splits, got:\n{out}"
        );
        assert!(
            !out.contains("\n   After markup."),
            "After markup must not inherit the note hang, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    /// Ticket fixture (Format::Rst / GitHub #349): same-line note body.
    fn same_line_note_fixture() -> &'static str {
        ".. note:: This is a long note sentence that must reflow. Second sentence.\n"
    }

    #[test]
    fn same_line_note_opener_is_structure_body_is_hung_prose() {
        let regions = RstParser.parse(same_line_note_fixture());
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == ".. note:: ")),
            "opener .. note:: must stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("This is a long note sentence that must reflow.")
                        && s.contains("Second sentence.")
            )),
            "same-line note body must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long note sentence")
            )),
            "same-line note body must not stay whole-line Structure, got {regions:?}"
        );
        assert_eq!(
            rst_admonition_marker_len(".. note:: "),
            Some(".. note:: ".len())
        );
        assert_eq!(rst_admonition_marker_len(".. figure:: image.png"), None);
    }

    #[test]
    fn leftover_container_names_reflow_like_note() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        for name in [
            "warning",
            "caution",
            "danger",
            "tip",
            "important",
            "hint",
            "error",
            "attention",
            "admonition",
            "figure",
            "topic",
            "sidebar",
            "container",
            "parsed-literal",
            "epigraph",
            "highlights",
            "pull-quote",
            "compound",
        ] {
            let arg = if matches!(name, "figure" | "admonition" | "sidebar" | "topic") {
                " Title"
            } else {
                ""
            };
            let input = format!(
                ".. {name}::{arg}\n\n   This is a long note sentence that must reflow. Second sentence.\n\nAfter the note. Next.\n"
            );
            let regions = RstParser.parse(&input);
            assert!(
                regions.iter().any(|r| matches!(
                    r,
                    Region::Prose(s)
                        if s.contains("This is a long note sentence that must reflow.")
                            && s.contains("Second sentence.")
                )),
                "{name} body must be Prose, got {regions:?}"
            );
            assert!(
                !regions.iter().any(|r| matches!(
                    r,
                    Region::Structure(s) if s.contains("This is a long note sentence")
                )),
                "{name} body must not freeze as Structure, got {regions:?}"
            );
            let out = format_text(&input, &cfg).unwrap();
            assert!(
                out.contains(
                    "   This is a long note sentence that must reflow.\n   Second sentence.\n"
                ),
                "{name} body must hang and split, got:\n{out}"
            );
            assert!(
                out.contains("After the note.\nNext."),
                "prose after {name} must still reflow, got:\n{out}"
            );
            assert_eq!(format_text(&out, &cfg).unwrap(), out);
        }
    }

    /// Ticket fixture (Format::Rst / GitHub #386): leftover body.py
    /// parsed-literal hangs and splits; flush After. / Next. stay unindented.
    fn leftover_parsed_literal_fixture() -> &'static str {
        concat!(
            ".. parsed-literal::\n",
            "\n",
            "   This is a long note sentence that must reflow. Second sentence.\n",
            "After. Next.\n",
        )
    }

    #[test]
    fn leftover_parsed_literal_fixture_hangs_and_splits() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = leftover_parsed_literal_fixture();
        let regions = RstParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(".. parsed-literal::")
            )),
            "parsed-literal opener must stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("This is a long note sentence that must reflow.")
                        && s.contains("Second sentence.")
            )),
            "parsed-literal body must be hung Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long note sentence")
            )),
            "parsed-literal body must not freeze as Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
            "parsed-literal hang spaces must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("After.") && s.contains("Next.")
            )),
            "After. / Next. must stay unindented Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("Second sentence.") && s.contains("After.")
            )),
            "After. must not join the parsed-literal body, got {regions:?}"
        );
        assert_eq!(
            rst_admonition_marker_len(".. parsed-literal:: Title"),
            None,
            "parsed-literal argument is not a same-line admonition body"
        );

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                ".. parsed-literal::\n",
                "\n",
                "   This is a long note sentence that must reflow.\n",
                "   Second sentence.\n",
                "After.\n",
                "Next.\n",
            ),
            "parsed-literal body must hang and split; After. / Next. stay flush, got:\n{out}"
        );
        assert!(
            !out.contains("\n   After."),
            "After. must not inherit the parsed-literal hang, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn two_space_code_block_body_is_code() {
        let input = ".. code-block:: python\n\n  print(\"hello\")\n";
        let regions = RstParser.parse(input);
        let code = regions.iter().find_map(|r| match r {
            Region::Code { lang, body, .. } => Some((lang.clone(), body.clone())),
            _ => None,
        });
        let (lang, body) = code.expect("expected one Region::Code");
        assert_eq!(lang.as_deref(), Some("python"));
        assert!(
            body.contains("print(\"hello\")"),
            "two-space body must stay in the code region, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("print"))),
            "two-space code-block body must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_two_space_code_block_body_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = ".. code-block:: python\n\n  print(\"hello\")\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "reporter two-space code-block body must stay identity under format, got:\n{out}"
        );
        assert!(
            out.contains("\n  print(\"hello\")"),
            "body must keep two-space indent, got:\n{out}"
        );
    }

    #[test]
    fn block_quote_hang_is_structure() {
        let input = "Before.\n\n   First sentence.\n   Second sentence.\n\nAfter.\n";
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
            "quote hang must be Structure, got {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            prose.iter().any(|s| s.contains("Before.")),
            "surrounding paragraph must stay Prose, got {regions:?}"
        );
        assert!(
            prose.iter().any(|s| s.contains("After.")),
            "trailing paragraph must stay Prose, got {regions:?}"
        );
        assert!(
            prose
                .iter()
                .any(|s| s.contains("First sentence.") && s.contains("Second sentence.")),
            "compact quote lines must join into one Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_block_quote_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "Before.\n\n   First sentence.\n   Second sentence.\n\nAfter.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "reporter block quote must stay identity under format, got:\n{out}"
        );
        assert!(
            out.contains("\n   First sentence."),
            "first quote line must keep three-space indent, got:\n{out}"
        );
        assert!(
            out.contains("\n   Second sentence."),
            "second quote line must keep three-space indent, got:\n{out}"
        );
    }

    #[test]
    fn surrounding_unquoted_paragraphs_still_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            "Before. More before.\n\n",
            "   First sentence.\n   Second sentence.\n\n",
            "After. More after.\n"
        );
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Before.\nMore before."),
            "leading unquoted paragraph must still reflow, got:\n{out}"
        );
        assert!(
            out.contains("\n   First sentence.\n   Second sentence.\n"),
            "quoted sentences must keep indent, got:\n{out}"
        );
        assert!(
            out.contains("After.\nMore after."),
            "trailing unquoted paragraph must still reflow, got:\n{out}"
        );
    }

    #[test]
    fn compact_block_quote_reflows_with_hang() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "Before.\n\n   First sentence. Second sentence.\n\nAfter.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, "Before.\n\n   First sentence.\n   Second sentence.\n\nAfter.\n",
            "quoted sentences must reflow with hang, got:\n{out}"
        );
    }

    #[test]
    fn doctest_opener_matches_docutils_pattern() {
        assert!(is_rst_doctest_opener(">>>"));
        assert!(is_rst_doctest_opener(">>> "));
        assert!(is_rst_doctest_opener(">>> print(1)"));
        assert!(!is_rst_doctest_opener(">>>print(1)"));
        assert!(!is_rst_doctest_opener(">> > print(1)"));
        assert!(!is_rst_doctest_opener("print(1)"));
        assert!(!is_rst_doctest_opener(".. >>>"));
        assert!(!is_rst_doctest_opener(">>>>"));
        assert!(!is_rst_doctest_opener(">>>>>"));
        assert!(is_rst_empty_doctest_opener(">>>"));
        assert!(is_rst_empty_doctest_opener(">>> "));
        assert!(is_rst_empty_doctest_opener(">>>  "));
        assert!(!is_rst_empty_doctest_opener(">>> print(1)"));
        assert!(!is_rst_empty_doctest_opener(">>> print(1.2)"));
        assert!(!is_rst_empty_doctest_opener(">>>>"));
    }

    #[test]
    fn doctest_opener_is_not_a_section_underline() {
        assert!(!is_underline(">>>"));
        assert!(!is_underline(">>> "));
        assert!(!is_underline("  >>>"));
        assert!(is_underline(">>"));
        assert!(is_underline(">>>>"));
        assert!(is_underline(">>>>>"));
        assert!(is_underline("===== "));
        assert!(is_underline(":::::"));
    }

    #[test]
    fn empty_explicit_comment_is_not_a_section_underline() {
        assert!(!is_underline(".."));
        assert!(!is_underline(".. "));
        assert!(!is_underline("  .."));
        assert!(!is_underline(".. a comment."));
        assert!(is_underline("..."));
        assert!(is_underline("...."));
        assert!(is_underline("....."));
    }

    #[test]
    fn doctest_block_is_structure_not_prose() {
        let input = concat!(
            ">>> print('Python-specific usage examples; begun with \">>> \"')\n",
            "Python-specific usage examples; begun with \">>> \"\n",
            ">>> print('(cut and pasted from interactive Python sessions)')\n",
            "(cut and pasted from interactive Python sessions)\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains(">>> print('Python-specific usage examples")
                        && s.contains("Python-specific usage examples; begun with")
                        && s.contains(">>> print('(cut and pasted")
                        && s.contains("(cut and pasted from interactive Python sessions)")
            )),
            "doctest block must be one Structure region, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains(">>>") || s.contains("Python-specific")
            )),
            "doctest block must not be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(r, Region::Code { .. })),
            "doctest block is Structure, not Code, got {regions:?}"
        );
    }

    #[test]
    fn reporter_doctest_block_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            ">>> print('Python-specific usage examples; begun with \">>> \"')\n",
            "Python-specific usage examples; begun with \">>> \"\n",
            ">>> print('(cut and pasted from interactive Python sessions)')\n",
            "(cut and pasted from interactive Python sessions)\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "doctest block must stay identity under format, got:\n{out}"
        );
        assert!(
            !out.contains(
                ">>> print('Python-specific usage examples; begun with \">>> \"') Python-specific"
            ),
            "SemBr must not join prompt and output, got:\n{out}"
        );
        assert!(
            !out.contains("\">>> \" >>> print"),
            "SemBr must not join output and the next >>>, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &cfg).unwrap(),
            out,
            "doctest identity must survive a second pass"
        );
    }

    #[test]
    fn surrounding_prose_still_reflows_around_doctest() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            "Before. More before.\n\n",
            ">>> print(1)\n",
            "1\n\n",
            "After. More after.\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Before.\nMore before."),
            "leading paragraph must still reflow, got:\n{out}"
        );
        assert!(
            out.contains(">>> print(1)\n1\n"),
            "doctest lines must stay unjoined, got:\n{out}"
        );
        assert!(
            out.contains("After.\nMore after."),
            "trailing paragraph must still reflow, got:\n{out}"
        );
    }

    #[test]
    fn prompt_only_doctest_is_structure_not_underline() {
        let input = concat!(
            ">>>\n",
            "Python-specific usage examples; begun with \">>> \"\n",
            ">>> print('(cut and pasted from interactive Python sessions)')\n",
            "(cut and pasted from interactive Python sessions)\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == ">>>")),
            "prompt-only >>> at EOL must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("Python-specific usage examples; begun with")
            )),
            "text after empty >>> must stay Prose, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains(">>> print('(cut and pasted")
                        && s.contains("(cut and pasted from interactive Python sessions)")
            )),
            "same-line >>> print must still open a Structure block, got {regions:?}"
        );
        assert!(!is_underline(">>>"), ">>> must not be a > adornment");
    }

    #[test]
    fn prompt_only_doctest_is_identity_under_format() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            ">>>\n",
            "Python-specific usage examples; begun with \">>> \"\n",
            ">>> print('(cut and pasted from interactive Python sessions)')\n",
            "(cut and pasted from interactive Python sessions)\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "prompt-only >>> must stay identity under format, got:\n{out}"
        );
        assert!(
            !out.contains(">>> Python-specific"),
            "must not join prompt-only >>> onto the output line, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &cfg).unwrap(),
            out,
            "prompt-only doctest identity must survive a second pass"
        );
    }

    #[test]
    fn prompt_only_doctest_does_not_steal_preceding_prose_as_title() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!("Before. More before.\n", ">>>\n", "1\n",);
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Before.\nMore before."),
            "preceding prose must still reflow, not become a section title, got:\n{out}"
        );
        assert!(
            out.contains(">>>\n1\n"),
            "prompt-only >>> plus output must stay unjoined, got:\n{out}"
        );
        assert!(
            !out.contains("Before. More before.\n>>>"),
            "title-lookahead must not freeze preceding prose as a section, got:\n{out}"
        );
    }

    #[test]
    fn empty_doctest_opener_does_not_swallow_following_prose() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = concat!(
            "Intro sentence here. Another intro sentence.\n",
            ">>>\n",
            "After empty. Next sentence.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == ">>>")),
            ">>> at EOL must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("After empty.")
            )),
            "empty >>> must not swallow following prose as Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("After empty.") && s.contains("Next sentence.")
            )),
            "After empty. / Next sentence. must stay Prose, got {regions:?}"
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                "Intro sentence here.\n",
                "Another intro sentence.\n",
                ">>>\n",
                "After empty.\n",
                "Next sentence.\n",
            ),
            "following prose must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn same_line_doctest_command_still_takes_the_block() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = concat!(">>> print(1.2)\n", "1.2\n",);
        let regions = RstParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(">>> print(1.2)") && s.contains("1.2")
            )),
            ">>> print(1.2) must stay one Structure block, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("1.2"))),
            "same-line doctest output must not be Prose, got {regions:?}"
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            ">>> print(1.2) must stay unchanged, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn five_gt_adornment_stays_a_section_underline() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "Input\n>>>>>\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input, ">>>>> must stay a > adornment, got:\n{out}");
        assert!(
            !out.contains("Input >>>>>"),
            "must not glue >>>>> onto the title, got:\n{out}"
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Input"))),
            "title above >>>>> must stay Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == ">>>>>")),
            ">>>>> must stay Structure adornment, got {regions:?}"
        );
    }

    #[test]
    fn rst_role_closer_keeps_two_sentence_lines() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = "The task is in :file:`README.md`.\nUse this section for questions.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input, "role closer must stay two lines, got:\n{out}");
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let literal = "The task is in ``README.md``.\nUse this section for questions.\n";
        let out = format_text(literal, &cfg).unwrap();
        assert_eq!(
            out, literal,
            "double-backtick literal must stay split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let suffix = "The task is in `README.md`:file:.\nUse this section for questions.\n";
        let out = format_text(suffix, &cfg).unwrap();
        assert_eq!(
            out, suffix,
            "suffix role closer must stay two lines, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let same_line = "The task is in `README.md`:file:. Use this section for questions.\n";
        let out = format_text(same_line, &cfg).unwrap();
        assert_eq!(
            out, suffix,
            "same-line suffix role closer must split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn list_marker_len_matches_docutils_enumerators() {
        assert_eq!(
            rst_list_marker_len("a. Alpha item. Second sentence."),
            Some(3)
        );
        assert_eq!(
            rst_list_marker_len("(1) Paren arabic. Second sentence."),
            Some(4)
        );
        assert_eq!(
            rst_list_marker_len("i. Roman item. Second sentence."),
            Some(3)
        );
        assert_eq!(rst_list_marker_len("iv. Fourth item."), Some(4));
        assert_eq!(rst_list_marker_len("A) Upper alpha."), Some(3));
        assert_eq!(rst_list_marker_len("(a) Paren alpha."), Some(4));
        assert_eq!(rst_list_marker_len("#) Auto paren."), Some(3));
        assert_eq!(rst_list_marker_len("(#) Surrounded auto."), Some(4));
        assert_eq!(rst_list_marker_len("#. Auto period."), Some(3));
        assert_eq!(rst_list_marker_len("12. Digits."), Some(4));
        assert_eq!(rst_list_marker_len("  a. Nested."), Some(5));
        assert_eq!(rst_list_marker_len("Hello. World."), None);
        assert_eq!(rst_list_marker_len("mid. Not roman."), None);
        assert_eq!(rst_list_marker_len("(see below)"), None);
        assert_eq!(rst_list_marker_len("1.2.3 version"), None);
        assert_eq!(rst_list_marker_len("A."), None);
        assert_eq!(rst_list_marker_len("  A."), None);
        assert_eq!(rst_list_marker_len("1."), Some(2));
        assert_eq!(rst_list_marker_len("#."), Some(2));
        assert_eq!(rst_list_marker_len("ii. Roman"), Some(4));
        assert_eq!(rst_list_marker_len("IV. Upper roman"), Some(4));
        assert_eq!(rst_list_marker_len("(i) Paren roman"), Some(4));
        assert_eq!(rst_list_marker_len("See. Prose"), None);
        assert_eq!(rst_list_marker_len("dim. Not roman"), None);
        assert_eq!(
            rst_list_marker_len("- - Document typed fixture exports."),
            Some(4)
        );
        assert_eq!(rst_list_marker_len("- - "), Some(4));
        assert_eq!(rst_list_marker_len("  - - nested."), Some(6));
        assert_eq!(rst_list_marker_len("- 1. inner."), Some(5));
        assert_eq!(rst_list_marker_len("- --not-nested"), Some(2));
        assert_eq!(rst_list_marker_len("*  Candidate:"), Some(3));
        assert_eq!(rst_list_marker_len("*  "), Some(3));
        assert_eq!(rst_list_marker_len("-  Term"), Some(3));
        assert_eq!(rst_list_marker_len("  *  Nested"), Some(5));
        assert_eq!(rst_list_marker_len("-"), Some(1));
        assert_eq!(rst_list_marker_len("*"), Some(1));
        assert_eq!(rst_list_marker_len("  -"), Some(3));
        assert_eq!(rst_list_marker_len("+"), None);
        assert_eq!(rst_list_marker_len("•"), Some("•".len()));
        assert_eq!(rst_list_marker_len("‣"), Some("‣".len()));
        assert_eq!(rst_list_marker_len("⁃"), Some("⁃".len()));
        assert_eq!(rst_list_marker_len("• After empty item."), Some("• ".len()));
        assert_eq!(rst_list_marker_len("‣ payload"), Some("‣ ".len()));
        assert_eq!(rst_list_marker_len("⁃ payload"), Some("⁃ ".len()));
        assert_eq!(rst_list_marker_len("•After"), None);
        assert_eq!(rst_list_marker_len("-item"), None);
    }

    #[test]
    fn alpha_roman_paren_enumerators_are_structure_plus_prose() {
        let input = concat!(
            "a. Alpha item. Second sentence.\n",
            "(1) Paren arabic. Second sentence.\n",
            "i. Roman item. Second sentence.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "a. ")),
            "alpha marker must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "(1) ")),
            "paren arabic marker must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "i. ")),
            "roman marker must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| {
                matches!(r, Region::Prose(s) if s.contains("Alpha item.") && s.contains("Second sentence."))
            }),
            "alpha item text must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(
                    r,
                    Region::Prose(s) if s.contains("a. ") || s.contains("(1) ") || s.contains("i. ")
                )
            }),
            "enumerator must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_alpha_roman_paren_enumerators_hang_at_marker_width() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            "a. Alpha item. Second sentence.\n",
            "(1) Paren arabic. Second sentence.\n",
            "i. Roman item. Second sentence.\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                "a. Alpha item.\n",
                "   Second sentence.\n",
                "(1) Paren arabic.\n",
                "    Second sentence.\n",
                "i. Roman item.\n",
                "   Second sentence.\n",
            ),
            "alpha/roman/paren enumerators must hang like 1. items, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung enumerators must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn option_column_len_matches_docutils_forms() {
        assert_eq!(
            rst_option_column_len("-a            Output all. Keep this aligned."),
            Some(14)
        );
        assert_eq!(
            rst_option_column_len("--long        Long option. Another sentence."),
            Some(14)
        );
        assert_eq!(rst_option_column_len("--input=file  Input file."), Some(14));
        assert_eq!(rst_option_column_len("/V            VMS option."), Some(14));
        assert_eq!(rst_option_column_len("-a, --all     Output all."), Some(14));
        assert_eq!(rst_option_column_len("- First item"), None);
        assert_eq!(rst_option_column_len("Hello world."), None);
    }

    #[test]
    fn option_list_column_is_structure_description_is_prose() {
        let input = concat!(
            "-a            Output all. Keep this aligned.\n",
            "--long        Long option. Another sentence.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "-a            ")),
            "short option column must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "--long        ")),
            "long option column must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| {
                matches!(r, Region::Prose(s) if s.contains("Output all.") && s.contains("Keep this aligned."))
            }),
            "short-option description must be Prose, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| {
                matches!(r, Region::Prose(s) if s.contains("Long option.") && s.contains("Another sentence."))
            }),
            "long-option description must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(
                    r,
                    Region::Prose(s) if s.contains("-a") || s.contains("--long")
                )
            }),
            "option column must not be Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_option_list_keeps_alignment_and_hangs_description() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        };
        let input = concat!(
            "-a            Output all. Keep this aligned.\n",
            "--long        Long option. Another sentence.\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            concat!(
                "-a            Output all.\n",
                "              Keep this aligned.\n",
                "--long        Long option.\n",
                "              Another sentence.\n",
            ),
            "option descriptions must hang at the option column, got:\n{out}"
        );
        assert!(
            out.contains("-a            Output all."),
            "short option must keep two-or-more spaces, got:\n{out}"
        );
        assert!(
            out.contains("--long        Long option."),
            "long option must keep two-or-more spaces, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung option list must be identity, got:\n{twice}"
        );
        assert!(
            oracle::matches(Format::Rst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );

        let extra = concat!(
            "--input=file  Input file. Another sentence.\n",
            "/V            VMS option. Another sentence.\n"
        );
        let extra_out = format_text(extra, &cfg).unwrap();
        assert_eq!(
            extra_out,
            concat!(
                "--input=file  Input file.\n",
                "              Another sentence.\n",
                "/V            VMS option.\n",
                "              Another sentence.\n",
            ),
            "--input=file and /V must hang at the option column, got:\n{extra_out}"
        );

        // max_width wrap used to collapse the 2+ spaces into one.
        let wrap_cfg = FormatConfig {
            format: Format::Rst,
            max_width: 22,
            ..Default::default()
        };
        let wrap_in = "-a            Output all extra words here.\n";
        let wrap_out = format_text(wrap_in, &wrap_cfg).unwrap();
        assert!(
            wrap_out.contains("-a            "),
            "wrap must not eat option-column spaces, got:\n{wrap_out}"
        );
    }

    #[test]
    fn jinja_statement_matcher_follows_jinja2_block_delimiters() {
        assert!(is_rst_jinja_statement(r#"{% set foo = "foo" %}"#));
        assert!(is_rst_jinja_statement(r#"{% set bar = "bar" %}"#));
        assert!(is_rst_jinja_statement("{%- set foo = \"foo\" %}"));
        assert!(is_rst_jinja_statement("{% set foo = \"foo\" -%}"));
        assert!(is_rst_jinja_statement("{%+ set foo = \"foo\" +%}"));
        assert!(is_rst_jinja_statement("{% set foo = \"foo\" %}  "));
        assert!(is_rst_jinja_statement("{% for k, v in topics.items() %}"));
        assert!(is_rst_jinja_statement("{% endfor %}"));
        assert!(!is_rst_jinja_statement("{{ foo }}"));
        assert!(!is_rst_jinja_statement("{# comment #}"));
        assert!(!is_rst_jinja_statement(r#"The tag {% set x = 1 %}"#));
        assert!(!is_rst_jinja_statement("{% set foo = \"foo\""));
        assert!(!is_rst_jinja_statement("set foo = \"foo\" %}"));
        assert!(!is_rst_jinja_statement("Hello world."));
        assert!(!is_rst_jinja_statement(".. code-block:: python"));
    }

    /// GitHub #196 / snapper-79b2: consecutive `{% ... %}` lines stay Structure.
    #[test]
    fn consecutive_jinja_statements_are_structure_not_prose() {
        let input = concat!(
            "         {% set foo = \"foo\" %}\n",
            "         {% set bar = \"bar\" %}\n",
            "\n",
            "         .. code-block:: python\n",
            "\n",
            "            pass\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(r#"{% set foo = "foo" %}"#)
            )),
            "first Jinja statement must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(r#"{% set bar = "bar" %}"#)
            )),
            "second Jinja statement must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("{% set foo") || s.contains("{% set bar")
            )),
            "Jinja statements must not be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("set foo") && s.contains("set bar")
            )),
            "Jinja statements must not join into one Prose region, got {regions:?}"
        );
        let code = regions.iter().find_map(|r| match r {
            Region::Code {
                lang, header, body, ..
            } => Some((lang.clone(), header.clone(), body.clone())),
            _ => None,
        });
        let (lang, header, body) = code.expect("expected .. code-block:: as Region::Code");
        assert_eq!(lang.as_deref(), Some("python"));
        assert!(
            header.contains(".. code-block:: python"),
            "code-block header must stay structure/code, got {header:?}"
        );
        assert!(
            body.contains("pass"),
            "code-block body must stay, got {body:?}"
        );
    }

    #[test]
    fn anonymous_target_matcher_follows_docutils() {
        assert!(is_rst_anonymous_target("__"));
        assert!(is_rst_anonymous_target(
            "__ https://www.python.org/some/very/long/path"
        ));
        assert!(is_rst_anonymous_target("__  https://example.com"));
        assert!(!is_rst_anonymous_target("__https://example.com"));
        assert!(!is_rst_anonymous_target("___"));
        assert!(!is_rst_anonymous_target("Hello __ there"));
        assert!(!is_rst_anonymous_target(".. _name: https://example.com"));
    }

    /// GitHub #93 / snapper-lhat: `__ url` is Body.anonymous, not Prose.
    #[test]
    fn anonymous_hyperlink_target_is_structure_not_prose() {
        let input = concat!(
            "__ https://www.python.org/some/very/long/path\n",
            "\n",
            "See the target. Next sentence.\n",
        );
        let regions = RstParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("__ https://www.python.org/some/very/long/path")
            )),
            "anonymous target line must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("__") || s.contains("python.org")
            )),
            "anonymous target must not be Prose, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("See the target.") && s.contains("Next sentence.")
            )),
            "following paragraph must stay Prose, got {regions:?}"
        );
    }

    #[test]
    fn reporter_anonymous_target_is_identity_and_prose_still_splits() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Rst,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = concat!(
            "__ https://www.python.org/some/very/long/path\n",
            "\n",
            "See the target. Next sentence.\n",
        );
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.lines()
                .any(|l| l == "__ https://www.python.org/some/very/long/path"),
            "URI line must stay one Structure line, got:\n{out}"
        );
        assert_eq!(
            out,
            concat!(
                "__ https://www.python.org/some/very/long/path\n",
                "\n",
                "See the target.\n",
                "Next sentence.\n",
            ),
            "target stays; following prose still splits, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &cfg).unwrap(),
            out,
            "anonymous target fixture must be identity, got:\n{out}"
        );

        let wrap_cfg = FormatConfig {
            format: Format::Rst,
            max_width: 20,
            ..Default::default()
        }
        .without_safety_backstops();
        let wrap_out = format_text(input, &wrap_cfg).unwrap();
        assert!(
            wrap_out
                .lines()
                .any(|l| l == "__ https://www.python.org/some/very/long/path"),
            "narrow wrap must not break the URI, got:\n{wrap_out}"
        );
    }

    #[test]
    fn indented_anonymous_target_is_structure() {
        let input = "   __ https://example.com/a/very/long/path\n";
        let regions = RstParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("__ https://example.com/a/very/long/path")
            )),
            "indented anonymous target must be Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("example.com"))),
            "indented anonymous target must not be Prose, got {regions:?}"
        );
    }
}
