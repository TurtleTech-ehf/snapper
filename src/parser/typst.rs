//! Native Typst markup parser.
//!
//! Markup paragraphs and list bodies are prose. Headings, statement-level
//! code (`#set`, `#let`, `#show`, …), unclosed code calls, raw fences,
//! comments, and math are structure or code. A closed content call that
//! shares a line with surrounding prose stays prose; the inline call is
//! kept atomic by the sentence splitter.

use crate::parser::{
    ByteSpan, FormatParser, Line, ProseBuf, SpannedRegion, flush_prose_spanned, iter_lines,
    join_prose_gap, push_prose_line,
};
#[cfg(test)]
use crate::parser::{Region, RegionOrigin};

pub struct TypstParser;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Frame {
    Paren,
    Brace,
    Content,
    String,
    Math,
    Comment,
}

#[derive(Clone, Debug, Default)]
struct Depth {
    frames: Vec<Frame>,
    /// Backtick count of a raw span left open at the end of the last line.
    raw: Option<usize>,
}

impl Depth {
    fn top(&self) -> Option<Frame> {
        self.frames.last().copied()
    }

    /// Innermost frame is a code group, string, math, or block comment.
    fn top_is_code(&self) -> bool {
        matches!(
            self.top(),
            Some(Frame::Paren | Frame::Brace | Frame::String | Frame::Math | Frame::Comment)
        )
    }

    fn in_content(&self) -> bool {
        self.top() == Some(Frame::Content)
    }

    /// Walk one source line, updating open groups.
    ///
    /// Only code lines and content closers are fed. Markup prose is not,
    /// so a `]` inside a sentence does not close a content block.
    fn feed(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if let Some(need) = self.raw {
                if bytes[i] == b'`' {
                    let ticks = backtick_run(bytes, i);
                    if ticks >= need {
                        self.raw = None;
                        i += need;
                        continue;
                    }
                    i += ticks.max(1);
                    continue;
                }
                i += 1;
                continue;
            }
            match self.top() {
                Some(Frame::String) => {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        self.frames.pop();
                    }
                    i += 1;
                    continue;
                }
                Some(Frame::Math) => {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'$' {
                        self.frames.pop();
                    }
                    i += 1;
                    continue;
                }
                Some(Frame::Comment) => {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        self.frames.push(Frame::Comment);
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        self.frames.pop();
                        i += 2;
                        continue;
                    }
                    i += 1;
                    continue;
                }
                _ => {}
            }
            if bytes[i] == b'`' {
                let ticks = backtick_run(bytes, i);
                // `` is an empty raw. Any other run reads through to its closer.
                if ticks == 2 {
                    i += 2;
                    continue;
                }
                if let Some(end) = raw_closer_end(bytes, i + ticks, ticks) {
                    i = end;
                    continue;
                }
                self.raw = Some(ticks);
                return;
            }
            if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
                return;
            }
            if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                self.frames.push(Frame::Comment);
                i += 2;
                continue;
            }
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            match bytes[i] {
                b'"' => self.frames.push(Frame::String),
                b'$' => self.frames.push(Frame::Math),
                b'(' => self.frames.push(Frame::Paren),
                b')' if self.top() == Some(Frame::Paren) => {
                    self.frames.pop();
                }
                b'{' => self.frames.push(Frame::Brace),
                b'}' if self.top() == Some(Frame::Brace) => {
                    self.frames.pop();
                }
                b'[' if self.top() != Some(Frame::Content) => {
                    self.frames.push(Frame::Content);
                }
                b']' if self.top() == Some(Frame::Content) => {
                    self.frames.pop();
                }
                _ => {}
            }
            i += 1;
        }
    }
}

struct Hang {
    /// Byte width of the marker or indent continuation lines must reach.
    width: usize,
    /// The marker line had no body. The next indented line owns its indent.
    empty_body: bool,
}

impl FormatParser for TypstParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let lines = iter_lines(input);
        let mut regions = Vec::new();
        let mut prose = ProseBuf::default();
        let mut prose_span: Option<ByteSpan> = None;
        let mut term: Option<ByteSpan> = None;
        let mut hang: Option<Hang> = None;
        let mut carry = LineCarry::default();
        let mut depth = Depth::default();
        let mut pragma_off = false;
        let mut display_math = false;
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // A raw span stays one piece, including the newline and any
            // indentation inside it. The next line is not a heading or a list.
            if carry.raw.is_some() {
                extend_open_raw(
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &line,
                    hang.is_some(),
                );
                carry = line_carry(line.text, carry.raw);
                i += 1;
                continue;
            }

            if display_math {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                regions.push(SpannedRegion::structure(input, line.span()));
                if line.text.trim() == "$" {
                    display_math = false;
                }
                i += 1;
                continue;
            }

            if depth.top() == Some(Frame::Comment) {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                depth.feed(line.text);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if line.text.trim().is_empty() {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                regions.push(SpannedRegion::blank(input, line.span()));
                i += 1;
                continue;
            }

            if depth.top_is_code() {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                depth.feed(line.text);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if let Some(on) = super::check_pragma(line.text) {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                pragma_off = !on;
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }
            if pragma_off {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if is_line_comment(line.text) || line.text.trim_start().starts_with("/*") {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                depth.feed(line.text);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if fence_open(line.text).is_some() {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                i = take_fence(input, &lines, i, &mut regions);
                continue;
            }

            if line.text.trim() == "$" {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                display_math = true;
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if is_display_math_line(line.text) {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if is_heading_line(line.text) {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if depth.in_content() && is_content_closer(line.text) {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                depth.feed(line.text);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if is_opening_code_line(line.text, &depth) {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                depth.feed(line.text);
                regions.push(SpannedRegion::structure(input, line.span()));
                i += 1;
                continue;
            }

            if let Some(marker_len) = list_marker_bytes(line.text) {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                let marker = ByteSpan::new(line.start, line.start + marker_len);
                regions.push(SpannedRegion::structure(input, marker));
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
                    carry = line_carry(body, None);
                }
                i += 1;
                continue;
            }

            if let Some(active) = hang.as_ref() {
                let indent = leading_spaces(line.text);
                if indent >= active.width && indent < line.text.len() {
                    if carry.hard_break {
                        split_hang_after_terminator(
                            input,
                            &mut prose,
                            &mut prose_span,
                            &mut term,
                            &line,
                            &mut regions,
                        );
                        let spaces = leading_spaces(line.text);
                        let body = line.text[spaces..].trim();
                        carry = line_carry(body, None);
                        i += 1;
                        continue;
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
                        carry = line_carry(line.text[indent..].trim(), None);
                    } else {
                        push_hang_body(&mut prose, &mut prose_span, &mut term, &line, 0, true);
                        carry = line_carry(line.text.trim(), None);
                    }
                    i += 1;
                    continue;
                }
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                continue;
            }

            let indent = leading_spaces(line.text);
            if indent > 0 && indent < line.text.len() {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
                let marker = ByteSpan::new(line.start, line.start + indent);
                regions.push(SpannedRegion::structure(input, marker));
                hang = Some(Hang {
                    width: indent,
                    empty_body: false,
                });
                push_hang_body(&mut prose, &mut prose_span, &mut term, &line, indent, false);
                carry = line_carry(line.text[indent..].trim(), None);
                i += 1;
                continue;
            }

            if carry.hard_break {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
                    &mut carry,
                    &mut regions,
                );
            }
            push_prose_line(&mut prose, &mut prose_span, &line, true, true);
            term = None;
            carry = line_carry(line.text.trim(), None);
            i += 1;
        }

        flush_open(
            input,
            &mut prose,
            &mut prose_span,
            &mut term,
            &mut hang,
            &mut carry,
            &mut regions,
        );
        regions
    }
}

fn flush_open(
    input: &str,
    prose: &mut ProseBuf,
    prose_span: &mut Option<ByteSpan>,
    term: &mut Option<ByteSpan>,
    hang: &mut Option<Hang>,
    carry: &mut LineCarry,
    regions: &mut Vec<SpannedRegion>,
) {
    flush_prose_spanned(prose, prose_span, regions);
    if let Some(span) = term.take() {
        if !span.is_empty() {
            regions.push(SpannedRegion::structure(input, span));
        }
    }
    *hang = None;
    *carry = LineCarry::default();
}

/// End a list-item line that holds a comment or a hash expression, then
/// keep the following indented line in the same item.
fn split_hang_after_terminator(
    input: &str,
    prose: &mut ProseBuf,
    prose_span: &mut Option<ByteSpan>,
    term: &mut Option<ByteSpan>,
    line: &Line<'_>,
    regions: &mut Vec<SpannedRegion>,
) {
    flush_prose_spanned(prose, prose_span, regions);
    if let Some(span) = term.take() {
        if !span.is_empty() {
            regions.push(SpannedRegion::structure(input, span));
        }
    }
    let spaces = leading_spaces(line.text);
    if spaces > 0 {
        regions.push(SpannedRegion::structure(
            input,
            ByteSpan::new(line.start, line.start + spaces),
        ));
    }
    push_hang_body(prose, prose_span, term, line, spaces, false);
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

#[derive(Default)]
struct LineCarry {
    /// Backtick count of a raw span that still wants its closer.
    raw: Option<usize>,
    /// This line ends with a line comment or a hash expression. The next
    /// line must stay after that newline.
    hard_break: bool,
}

fn backtick_run(bytes: &[u8], start: usize) -> usize {
    let mut n = 0;
    while start + n < bytes.len() && bytes[start + n] == b'`' {
        n += 1;
    }
    n
}

/// Index just past a closing run of `ticks` backticks, if one exists.
fn raw_closer_end(bytes: &[u8], from: usize, ticks: usize) -> Option<usize> {
    if ticks == 0 {
        return None;
    }
    let mut i = from;
    let mut found = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            found += 1;
            if found == ticks {
                return Some(i + 1);
            }
        } else {
            found = 0;
        }
        i += 1;
    }
    None
}

/// End of a Typst raw span at `start`, or `None` when `start` is not a backtick.
///
/// Typst 0.15 reads the whole backtick run first. Exactly two backticks are
/// an empty raw. Any other run, including one that crosses lines, ends at
/// the same number of closing backticks. An unclosed raw runs to the end.
pub(crate) fn typst_raw_span_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'`') {
        return None;
    }
    let ticks = backtick_run(bytes, start);
    if ticks == 2 {
        return Some(start + 2);
    }
    if ticks == 0 {
        return None;
    }
    Some(raw_closer_end(bytes, start + ticks, ticks).unwrap_or(text.len()))
}

/// True when byte `at` sits inside a raw span.
pub(crate) fn typst_byte_in_raw(text: &str, at: usize) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() && i <= at {
        if bytes[i] == b'`' {
            if let Some(end) = typst_raw_span_end(text, i) {
                if i < at && at < end {
                    return true;
                }
                if end > i {
                    i = end;
                    continue;
                }
            }
        }
        i += text[i..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn extend_open_raw(
    prose: &mut ProseBuf,
    prose_span: &mut Option<ByteSpan>,
    term: &mut Option<ByteSpan>,
    line: &Line<'_>,
    in_hang: bool,
) {
    if let Some(span) = term.take() {
        if !span.is_empty() {
            prose.push('\n');
            if let Some(s) = prose_span.as_mut() {
                s.end = span.end;
            }
        }
    } else if !prose.is_empty() {
        prose.push('\n');
    }
    prose.push_str(line.text);
    let end = if in_hang {
        line.start + line.text.len()
    } else {
        line.end
    };
    match prose_span {
        Some(span) => span.end = end,
        None => *prose_span = Some(ByteSpan::new(line.start, end)),
    }
    if in_hang {
        *term = Some(line.terminator_span());
    }
}

/// How `line` ends, given a raw span already open from the previous line.
fn line_carry(line: &str, mut raw: Option<usize>) -> LineCarry {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut hard = false;
    while i < bytes.len() {
        if let Some(need) = raw {
            if bytes[i] == b'`' {
                let ticks = backtick_run(bytes, i);
                if ticks >= need {
                    i += need;
                    raw = None;
                    continue;
                }
                i += ticks.max(1);
                continue;
            }
            i += 1;
            continue;
        }
        if bytes[i] == b'`' {
            let ticks = backtick_run(bytes, i);
            if ticks == 2 {
                i += 2;
                continue;
            }
            if ticks > 0 {
                if let Some(end) = raw_closer_end(bytes, i + ticks, ticks) {
                    i = end;
                    continue;
                }
                return LineCarry {
                    raw: Some(ticks),
                    hard_break: false,
                };
            }
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            if let Some(end) = typst_block_comment_end(line, i) {
                if end < line.len() {
                    i = end;
                    continue;
                }
            }
            return LineCarry {
                raw: None,
                hard_break: hard,
            };
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            // `https://` is a link. The slashes sit immediately after `:`.
            if i == 0 || bytes[i - 1] != b':' {
                return LineCarry {
                    raw: None,
                    hard_break: true,
                };
            }
            i += 2;
            continue;
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'#' && hash_can_start(&line[i..]) {
            hard = true;
            let start = i;
            if let Some(end) = typst_hash_expr_end(line, i) {
                if end > i {
                    if let Some(ticks) = raw_left_open(&line[start..end]) {
                        return LineCarry {
                            raw: Some(ticks),
                            hard_break: false,
                        };
                    }
                    i = end;
                    continue;
                }
            }
        }
        i += line[i..].chars().next().map_or(1, char::len_utf8);
    }
    LineCarry {
        raw: None,
        hard_break: hard,
    }
}

/// The newline after `line` ends a comment or a hash expression.
///
/// Joining the next line onto it would swallow text into a `//` comment or
/// continue a code statement. A later sentence of the same paragraph has to
/// stay on this line: a break there would not join back.
pub(crate) fn typst_line_owns_its_newline(line: &str) -> bool {
    line_carry(line.trim(), None).hard_break
}

/// Tick count of a raw that `text` does not close. Strings hide backticks.
fn raw_left_open(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut string = false;
    while i < bytes.len() {
        if string {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                string = false;
            }
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            string = true;
            i += 1;
            continue;
        }
        if bytes[i] == b'`' {
            let ticks = backtick_run(bytes, i);
            if ticks == 2 {
                i += 2;
                continue;
            }
            if ticks > 0 {
                if let Some(end) = raw_closer_end(bytes, i + ticks, ticks) {
                    i = end;
                    continue;
                }
                return Some(ticks);
            }
        }
        i += 1;
    }
    None
}

fn take_fence(
    input: &str,
    lines: &[Line<'_>],
    index: usize,
    regions: &mut Vec<SpannedRegion>,
) -> usize {
    let line = lines[index];
    let (_indent, ticks, lang) = fence_open(line.text).unwrap_or((0, 3, None));
    let header = line.span();
    let body_start = line.end;
    let mut j = index + 1;
    while j < lines.len() {
        if fence_close(lines[j].text, ticks) {
            regions.push(SpannedRegion::code(
                input,
                lang,
                header,
                ByteSpan::new(body_start, lines[j].start),
                lines[j].span(),
            ));
            return j + 1;
        }
        j += 1;
    }
    let end = input.len();
    regions.push(SpannedRegion::code(
        input,
        lang,
        header,
        ByteSpan::new(body_start, end),
        ByteSpan::new(end, end),
    ));
    j
}

fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|&b| b == b' ').count()
}

fn is_line_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

fn is_heading_line(line: &str) -> bool {
    let bytes = line.as_bytes();
    if bytes.first() != Some(&b'=') {
        return false;
    }
    let marks = bytes.iter().take_while(|&&b| b == b'=').count();
    match bytes.get(marks) {
        None | Some(b' ' | b'\t') => true,
        Some(_) => false,
    }
}

fn is_display_math_line(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(inner) = trimmed.strip_prefix('$').and_then(|s| s.strip_suffix('$')) else {
        return false;
    };
    if inner.is_empty() || inner.contains('$') {
        return false;
    }
    inner.starts_with(char::is_whitespace)
        && inner.ends_with(char::is_whitespace)
        && !inner.trim().is_empty()
}

fn is_content_closer(line: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with(']') {
        return false;
    }
    let rest = trimmed.trim_start_matches(']');
    rest.chars()
        .all(|c| matches!(c, ')' | ',' | ';' | '.' | ' ' | '\t' | '{' | '}'))
}

fn hash_can_start(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix('#') else {
        return false;
    };
    let Some(ch) = rest.chars().next() else {
        return false;
    };
    ch == '_' || ch.is_ascii_alphabetic() || matches!(ch, '[' | '(' | '{' | '"' | '$')
}

fn statement_keyword(trimmed: &str) -> bool {
    let Some(rest) = trimmed.trim_start().strip_prefix('#') else {
        return false;
    };
    let ident: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    matches!(
        ident.as_str(),
        "let" | "set" | "show" | "import" | "include" | "context" | "export" | "return"
    )
}

fn is_opening_code_line(line: &str, depth: &Depth) -> bool {
    if depth.top_is_code() {
        return true;
    }
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') || !hash_can_start(trimmed) {
        return false;
    }
    if statement_keyword(trimmed) {
        return true;
    }
    let mut probe = depth.clone();
    let before = probe.frames.len();
    probe.feed(line);
    probe.frames.len() > before || probe.top_is_code()
}

/// Byte length of a Typst bullet, enum, or term marker, including indent.
fn list_marker_bytes(line: &str) -> Option<usize> {
    let indent = leading_spaces(line);
    let rest = line.get(indent..)?;
    if rest.starts_with("- ")
        || rest.starts_with("+ ")
        || rest.starts_with("-\t")
        || rest.starts_with("+\t")
    {
        return Some(indent + 2);
    }
    if let Some(stripped) = rest.strip_prefix("/ ") {
        let colon = stripped.find(':')?;
        let after = &stripped[colon + 1..];
        if after.is_empty() || after.starts_with(' ') {
            let extra = after.chars().take_while(|c| *c == ' ').count();
            return Some(indent + 2 + colon + 1 + extra);
        }
    }
    None
}

/// True when `s` is only a term-list marker (`/ Term: `), including indent.
pub(crate) fn typst_term_marker_len(s: &str) -> Option<usize> {
    if s.is_empty() || s.contains('\n') {
        return None;
    }
    let marker = list_marker_bytes(s)?;
    if marker != s.len() {
        return None;
    }
    let rest = s.trim_start();
    if rest.starts_with("/ ") {
        Some(s.len())
    } else {
        None
    }
}

fn fence_open(line: &str) -> Option<(usize, usize, Option<String>)> {
    let indent = leading_spaces(line);
    let rest = line.get(indent..)?;
    let ticks = rest.bytes().take_while(|&b| b == b'`').count();
    if ticks < 3 {
        return None;
    }
    let after = rest[ticks..].trim_end();
    if after
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '+'))
    {
        return None;
    }
    let lang = if after.is_empty() {
        None
    } else {
        Some(after.to_string())
    };
    Some((indent, ticks, lang))
}

fn fence_close(line: &str, ticks: usize) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let n = trimmed.bytes().take_while(|&b| b == b'`').count();
    n >= ticks && n == trimmed.len()
}

/// True when putting `line` at column 0 would start a Typst block.
pub(crate) fn typst_line_opens_block(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("```") || trimmed.starts_with("//") || trimmed.starts_with("/*") {
        return true;
    }
    if is_heading_line(trimmed) || list_marker_bytes(trimmed).is_some() {
        return true;
    }
    if trimmed == "$" || is_display_math_line(trimmed) {
        return true;
    }
    // Closed content calls (`#emph[Hi.]`) stay prose. A statement or a
    // call that is still open would be reread as code.
    trimmed.starts_with('#')
        && hash_can_start(trimmed)
        && is_opening_code_line(trimmed, &Depth::default())
}

/// End of a Typst `#` call, `//` comment, or `/* */` comment starting at `start`.
pub(crate) fn typst_protected_span_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if start >= bytes.len() {
        return None;
    }
    if bytes[start] == b'`' {
        return typst_raw_span_end(text, start);
    }
    if bytes[start] == b'#' {
        return typst_hash_expr_end(text, start);
    }
    if bytes[start] == b'/' && bytes.get(start + 1) == Some(&b'/') {
        if start > 0 && bytes[start - 1] == b':' {
            return None;
        }
        let rel = text[start..].find('\n').unwrap_or(text.len() - start);
        return Some(start + rel);
    }
    if bytes[start] == b'/' && bytes.get(start + 1) == Some(&b'*') {
        return typst_block_comment_end(text, start);
    }
    None
}

pub(crate) fn typst_hash_expr_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'#') {
        return None;
    }
    let mut i = start + 1;
    if i >= bytes.len() || bytes[i] == b'\n' {
        return None;
    }
    let first = text[i..].chars().next()?;
    let ident = first == '_' || first.is_ascii_alphabetic();
    let group = matches!(first, '[' | '(' | '{' | '"' | '$');
    if !ident && !group {
        return None;
    }
    if ident {
        let n: usize = text[i..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .map(|c| c.len_utf8())
            .sum();
        i += n;
    }
    // `let` / `set` / `show` run to a semicolon or the newline. A space
    // after the keyword is still the statement.
    let statement = statement_keyword(&text[start..]);
    let mut stack: Vec<u8> = Vec::new();
    loop {
        if i >= bytes.len() || bytes[i] == b'\n' {
            return Some(i.max(start + 1));
        }
        if bytes[i] == b'`' {
            if let Some(end) = typst_raw_span_end(text, i) {
                i = end;
                continue;
            }
        }
        if let Some(&top) = stack.last() {
            if top == b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] != b'\n' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    stack.pop();
                }
                i += 1;
                continue;
            }
            if top == b'$' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] != b'\n' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'$' {
                    stack.pop();
                }
                i += 1;
                continue;
            }
            if top == b'C' {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    stack.push(b'C');
                    i += 2;
                    continue;
                }
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    stack.pop();
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
        }
        // A line comment ends the expression. Closers after `//` do not count.
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            return Some(i);
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            stack.push(b'C');
            i += 2;
            continue;
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] != b'\n' {
            i += 2;
            continue;
        }
        match bytes[i] {
            b'"' => {
                stack.push(b'"');
                i += 1;
            }
            b'$' => {
                stack.push(b'$');
                i += 1;
            }
            b'(' => {
                stack.push(b'(');
                i += 1;
            }
            b')' => {
                if stack.last() == Some(&b'(') {
                    stack.pop();
                }
                i += 1;
            }
            b'{' => {
                stack.push(b'{');
                i += 1;
            }
            b'}' => {
                if stack.last() == Some(&b'{') {
                    stack.pop();
                }
                i += 1;
            }
            b'[' => {
                stack.push(b'[');
                i += 1;
            }
            b']' => {
                if stack.last() == Some(&b'[') {
                    stack.pop();
                }
                i += 1;
            }
            b'.' if stack.is_empty() => {
                let next = text.get(i + 1..).and_then(|s| s.chars().next());
                if next.is_some_and(|c| c == '_' || c.is_ascii_alphabetic()) {
                    i += 1;
                    let n: usize = text[i..]
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                        .map(|c| c.len_utf8())
                        .sum();
                    i += n;
                } else if statement {
                    i += 1;
                } else {
                    return Some(i);
                }
            }
            b';' if statement && stack.is_empty() => return Some(i + 1),
            _ if stack.is_empty() && statement => i += 1,
            _ if stack.is_empty() => return Some(i),
            _ => i += 1,
        }
    }
}

fn typst_block_comment_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'/') || bytes.get(start + 1) != Some(&b'*') {
        return None;
    }
    let mut depth = 1i32;
    let mut i = start + 2;
    while i < bytes.len() {
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            depth += 1;
            i += 2;
            continue;
        }
        if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Some(i);
            }
            continue;
        }
        i += 1;
    }
    Some(text.len())
}

#[cfg(test)]
fn assert_covered(input: &str, regions: &[SpannedRegion]) {
    let mut cursor = 0usize;
    for (idx, region) in regions.iter().enumerate() {
        let origin = region
            .origin
            .unwrap_or_else(|| panic!("region {idx} has no origin"));
        let (start, end) = match origin {
            RegionOrigin::Whole(span) => (span.start, span.end),
            RegionOrigin::Code { header, footer, .. } => (header.start, footer.end),
        };
        assert_eq!(
            start, cursor,
            "region {idx} starts at {start}, expected {cursor}, input {input:?}"
        );
        assert!(end >= start, "region {idx} has a backwards span");
        assert!(
            input.get(start..end).is_some(),
            "region {idx} is not on a char boundary"
        );
        cursor = end;
    }
    assert_eq!(cursor, input.len(), "regions do not cover {input:?}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Format;
    use crate::oracle;
    use crate::{FormatConfig, format_text};

    fn parse(input: &str) -> Vec<Region> {
        let regions = TypstParser.parse_full(input);
        assert_covered(input, &regions);
        regions.into_iter().map(|r| r.region).collect()
    }

    fn format_typst(input: &str) -> String {
        let cfg = FormatConfig {
            format: Format::Typst,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap_or_else(|e| panic!("format: {e}"));
        let again = format_text(&out, &cfg).unwrap_or_else(|e| panic!("second: {e}"));
        assert_eq!(out, again, "not idempotent:\n{out}");
        assert!(
            oracle::matches(Format::Typst, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}\n in regions={:?}\n out regions={:?}",
            TypstParser.parse(input),
            TypstParser.parse(&out)
        );
        out
    }

    #[test]
    fn paragraph_splits_sentences() {
        let out = format_typst("The method is stable. It converges in three steps.\n");
        assert_eq!(out, "The method is stable.\nIt converges in three steps.\n");
    }

    #[test]
    fn abbreviations_stay_in_the_sentence() {
        let out = format_typst("Dr. Alvarez confirmed Fig. 3. The trial ended.\n");
        assert_eq!(out, "Dr. Alvarez confirmed Fig. 3.\nThe trial ended.\n");
    }

    #[test]
    fn heading_stays_and_body_splits() {
        let input = "= Introduction <intro>\n\nSemantic breaks keep diffs small. Each sentence stays put.\n";
        let out = format_typst(input);
        assert!(out.starts_with("= Introduction <intro>\n\n"));
        assert!(out.contains("Semantic breaks keep diffs small.\nEach sentence stays put.\n"));
    }

    #[test]
    fn bullet_and_enum_hang() {
        let out = format_typst(
            "- The method is stable. It converges.\n+ The baseline is slower. It uses more memory.\n",
        );
        assert_eq!(
            out,
            concat!(
                "- The method is stable.\n",
                "  It converges.\n",
                "+ The baseline is slower.\n",
                "  It uses more memory.\n",
            )
        );
    }

    #[test]
    fn term_list_hangs_at_the_marker() {
        let out = format_typst("/ Typst: A markup typesetter. It compiles to PDF.\n");
        assert_eq!(
            out,
            "/ Typst: A markup typesetter.\n         It compiles to PDF.\n"
        );
    }

    #[test]
    fn raw_fence_and_math_stay_whole() {
        let input = concat!(
            "See the listing. Then read on.\n\n",
            "```rust\n",
            "fn f() {\n",
            "    // One sentence. Two sentences.\n",
            "    let x = 1;\n",
            "}\n",
            "```\n\n",
            "$\n",
            "  E = m c^2\n",
            "$\n\n",
            "The inline $E = m c^2$ stays put. The next claim follows.\n",
        );
        let out = format_typst(input);
        assert!(out.contains("See the listing.\nThen read on.\n"));
        assert!(out.contains("```rust\nfn f() {\n    // One sentence. Two sentences.\n"));
        assert!(out.contains("$\n  E = m c^2\n$\n"));
        assert!(out.contains("The inline $E = m c^2$ stays put.\nThe next claim follows.\n"));
    }

    #[test]
    fn display_math_line_is_structure() {
        let regions = parse("$ E = m c^2 $\n");
        assert!(matches!(&regions[0], Region::Structure(s) if s.contains("$ E")));
    }

    #[test]
    fn set_and_let_lines_stay() {
        let input = "#set text(lang: \"en\")\n#let name = \"Dr. Smith. Really.\"\n\nHello there. Next sentence.\n";
        let out = format_typst(input);
        assert!(out.starts_with("#set text(lang: \"en\")\n#let name = \"Dr. Smith. Really.\"\n"));
        assert!(out.contains("Hello there.\nNext sentence.\n"));
    }

    #[test]
    fn multiline_content_block_reflows() {
        let input = concat!(
            "#figure(\n",
            "  caption: [\n",
            "    The measured curve. Error bars show one deviation.\n",
            "  ],\n",
            ")\n",
        );
        let out = format_typst(input);
        assert_eq!(
            out,
            concat!(
                "#figure(\n",
                "  caption: [\n",
                "    The measured curve.\n",
                "    Error bars show one deviation.\n",
                "  ],\n",
                ")\n",
            )
        );
    }

    #[test]
    fn inline_call_stays_on_its_sentence() {
        let out = format_typst("Done. #emph[Dr. Smith agreed.] Next sentence.\n");
        assert_eq!(out, "Done.\n#emph[Dr. Smith agreed.] Next sentence.\n");
    }

    #[test]
    fn strong_emphasis_and_label_stay_atomic() {
        let out =
            format_typst("See *Fig. 1 is bold.* _Eq. 2 holds._ Use @fig.1 and <intro>. Next.\n");
        assert!(out.contains("*Fig. 1 is bold.*"));
        assert!(out.contains("_Eq. 2 holds._"));
        assert!(out.contains("@fig.1"));
        assert!(out.contains("<intro>"));
        assert!(out.ends_with("Next.\n") || out.contains("\nNext.\n"));
    }

    #[test]
    fn line_comment_and_pragma() {
        let input = concat!(
            "// snapper:off\n",
            "Keep this. And this.\n",
            "// snapper:on\n",
            "Split this. And this.\n",
            "// A note stays. It is not prose.\n",
        );
        let out = format_typst(input);
        assert!(out.contains("Keep this. And this.\n"));
        assert!(out.contains("Split this.\nAnd this.\n"));
        assert!(out.contains("// A note stays. It is not prose.\n"));
    }

    #[test]
    fn block_comment_is_structure() {
        let input = "/*\nKeep both. Sentences here.\n*/\nAfter the note. Next one.\n";
        let out = format_typst(input);
        assert!(out.starts_with("/*\nKeep both. Sentences here.\n*/\n"));
        assert!(out.contains("After the note.\nNext one.\n"));
    }

    #[test]
    fn hash_expr_stops_before_trailing_prose() {
        let expr = "#emph[Dr. Smith.] Next";
        let end = typst_hash_expr_end(expr, 0).unwrap();
        assert_eq!(&expr[..end], "#emph[Dr. Smith.]");
    }

    #[test]
    fn nested_list_is_a_new_item() {
        let out =
            format_typst("- Outer sentence. It continues.\n  - Inner sentence. It continues.\n");
        assert_eq!(
            out,
            concat!(
                "- Outer sentence.\n",
                "  It continues.\n",
                "  - Inner sentence.\n",
                "    It continues.\n",
            )
        );
    }

    #[test]
    fn unclosed_string_stays_structure() {
        let input =
            "#let x = \"Dr. Smith. Really.\nstill in the string. Not prose.\n\"\nHello. There.\n";
        let out = format_typst(input);
        assert!(
            out.starts_with("#let x = \"Dr. Smith. Really.\nstill in the string. Not prose.\n\"\n")
        );
        assert!(out.contains("Hello.\nThere.\n"));
    }

    #[test]
    fn max_width_does_not_invent_a_heading() {
        let cfg = FormatConfig {
            format: Format::Typst,
            max_width: 24,
            ..Default::default()
        };
        let input = "Words before the mark = Title must not become a heading on its own line.\n";
        let out = format_text(input, &cfg).unwrap();
        let again = format_text(&out, &cfg).unwrap();
        assert_eq!(out, again);
        assert!(oracle::matches(Format::Typst, input, &out), "oracle\n{out}");
        assert!(
            !out.lines()
                .any(|line| is_heading_line(line) || line.starts_with("#set")),
            "wrap invented a block:\n{out}"
        );
        assert!(out.contains('='), "lost the marker:\n{out}");
        assert!(out.contains("Title"), "lost the heading word:\n{out}");
    }

    #[test]
    fn configured_fence_comment_reflows() {
        use std::collections::HashMap;

        use crate::config::CodeLang;
        let mut code = HashMap::new();
        code.insert(
            "rust".to_string(),
            CodeLang {
                line_comment: Some("//".to_string()),
                ..Default::default()
            },
        );
        let cfg = FormatConfig {
            format: Format::Typst,
            code,
            ..Default::default()
        };
        let input = "```rust\n// One sentence. Two sentences.\nlet x = 1;\n```\n";
        let out = format_text(input, &cfg).unwrap();
        let again = format_text(&out, &cfg).unwrap();
        assert_eq!(out, again);
        assert!(out.contains("// One sentence.\n// Two sentences.\n"));
        assert!(out.contains("let x = 1;\n"));
        assert!(oracle::matches_ex(
            Format::Typst,
            input,
            &out,
            false,
            Some(&cfg)
        ));
    }

    #[test]
    fn letter_is_not_a_let_statement() {
        let regions = parse("#letter[Hello. There.]\n");
        assert!(
            matches!(&regions[0], Region::Prose(_)),
            "closed content call stays prose, got {regions:?}"
        );
    }

    #[test]
    fn statement_hash_runs_to_the_newline_or_semicolon() {
        let expr = "#set text(size: 12pt)";
        let end = typst_hash_expr_end(expr, 0).unwrap();
        assert_eq!(&expr[..end], "#set text(size: 12pt)");
        let stopped = "#let x = 1; Next";
        let end = typst_hash_expr_end(stopped, 0).unwrap();
        assert_eq!(&stopped[..end], "#let x = 1;");
    }

    #[test]
    fn raw_span_includes_the_newline() {
        let text = "See `Dr. Smith.\nReally` now.";
        let end = typst_raw_span_end(text, text.find('`').unwrap()).unwrap();
        assert_eq!(&text[text.find('`').unwrap()..end], "`Dr. Smith.\nReally`");
        assert!(typst_byte_in_raw(text, text.find("Smith").unwrap()));
    }

    #[test]
    fn line_comment_does_not_swallow_the_next_line() {
        let input = "Hello. World. // note\nNext line. More.\n";
        let out = format_typst(input);
        assert_eq!(
            out,
            concat!("Hello.\n", "World. // note\n", "Next line.\n", "More.\n",)
        );
        assert_typst_same(input, &out);
    }

    #[test]
    fn line_comment_does_not_swallow_a_list_continuation() {
        let input = "- Keep this. // note\n  And this stays. It continues.\n";
        let out = format_typst(input);
        assert_eq!(
            out,
            concat!(
                "- Keep this. // note\n",
                "  And this stays.\n",
                "  It continues.\n",
            )
        );
        assert_typst_same(input, &out);
    }

    #[test]
    fn raw_span_stays_atomic_before_the_next_statement() {
        let input = "Use `Dr. Smith.\nReally` now.\n#repr(raw)\nAfter the call. Next.\n";
        let out = format_typst(input);
        assert!(
            out.contains("`Dr. Smith.\nReally`"),
            "raw was split:\n{out}"
        );
        assert!(
            !out.contains("` #repr") && !out.contains("\n` #repr") && !out.contains("`#repr"),
            "closer glued onto the next call:\n{out}"
        );
        assert!(out.contains("#repr(raw)\n"));
        assert!(out.contains("After the call.\nNext.\n"));
        assert_typst_same(input, &out);
    }

    #[test]
    fn inline_raw_with_a_period_stays_atomic() {
        let input = "Use `Dr. Smith. Really` now. Next.\n";
        let out = format_typst(input);
        assert!(out.contains("`Dr. Smith. Really`"), "{out}");
        assert!(out.contains("Next.\n") || out.ends_with("Next.\n"));
        assert_typst_same(input, &out);
    }

    #[test]
    fn mid_line_set_keeps_its_terminating_newline() {
        let input = "Done. #set text(size: 12pt)\nThe next claim follows. Again.\n";
        let out = format_typst(input);
        assert_eq!(
            out,
            concat!(
                "Done. #set text(size: 12pt)\n",
                "The next claim follows.\n",
                "Again.\n",
            )
        );
        assert_typst_same(input, &out);
    }

    #[test]
    fn mid_line_let_keeps_its_terminating_newline() {
        let input = "Done. #let name = \"Dr. Smith.\"\nThe next claim follows.\n";
        let out = format_typst(input);
        assert_eq!(
            out,
            "Done. #let name = \"Dr. Smith.\"\nThe next claim follows.\n"
        );
        assert_typst_same(input, &out);
    }

    #[test]
    fn hash_line_is_not_joined_under_max_width() {
        let cfg = FormatConfig {
            format: Format::Typst,
            max_width: 40,
            ..Default::default()
        };
        let input = "Done. #set text(size: 12pt)\nThe next claim follows after the statement.\n";
        let out = format_text(input, &cfg).unwrap();
        let again = format_text(&out, &cfg).unwrap();
        assert_eq!(out, again, "{out}");
        assert!(oracle::matches(Format::Typst, input, &out), "{out}");
        assert!(
            out.contains("#set text(size: 12pt)\n"),
            "statement lost its newline:\n{out}"
        );
        assert!(!out.contains("12pt) The"), "{out}");
        assert_typst_same(input, &out);
    }

    #[test]
    fn hash_in_a_sentence_keeps_the_following_sentence() {
        let cfg = FormatConfig {
            format: Format::Typst,
            ..Default::default()
        }
        .without_safety_backstops();
        let input = "See #emph[Note]. Next claim. Another.\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input);
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
        assert_typst_same(input, &out);
        let odd = "#a*\"*. aA\".";
        let odd_out = format_text(odd, &cfg).unwrap();
        assert_eq!(odd_out, odd);
        assert!(oracle::matches(Format::Typst, odd, &odd_out), "{odd_out}");
    }

    /// Compile `input` and `output` with Typst 0.15.1 when that binary is on
    /// `PATH`. A missing compiler leaves the exact-output assertion in charge.
    fn assert_typst_same(input: &str, output: &str) {
        let Some(bin) = typst_0151() else {
            return;
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let in_svg = compile_typst_svg(&bin, dir.path(), "in", input);
        let out_svg = compile_typst_svg(&bin, dir.path(), "out", output);
        assert_eq!(
            in_svg, out_svg,
            "Typst 0.15.1 SVG diverged\n--- input ---\n{input}\n--- output ---\n{output}"
        );
    }

    fn typst_0151() -> Option<std::path::PathBuf> {
        let path = std::env::var_os("TYPST_BIN")
            .map(std::path::PathBuf::from)
            .or_else(which_typst)?;
        let version = std::process::Command::new(&path)
            .arg("--version")
            .output()
            .ok()?;
        if !version.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&version.stdout);
        if text.starts_with("typst 0.15.1") {
            Some(path)
        } else {
            None
        }
    }

    fn which_typst() -> Option<std::path::PathBuf> {
        let path = std::env::var_os("PATH")?;
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("typst");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    fn compile_typst_svg(
        bin: &std::path::Path,
        dir: &std::path::Path,
        name: &str,
        source: &str,
    ) -> Vec<u8> {
        let typ = dir.join(format!("{name}.typ"));
        let svg = dir.join(format!("{name}.svg"));
        std::fs::write(&typ, source).expect("write typst");
        let run = std::process::Command::new(bin)
            .arg("compile")
            .arg("--format")
            .arg("svg")
            .arg(&typ)
            .arg(&svg)
            .output()
            .expect("run typst");
        assert!(
            run.status.success(),
            "typst failed on {name}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        std::fs::read(&svg).expect("read svg")
    }
}
