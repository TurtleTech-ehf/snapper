//! Native Typst markup parser.
//!
//! Markup paragraphs and list bodies are prose. Headings, statement-level
//! code (`#set`, `#let`, `#show`, …), unclosed code calls, raw fences,
//! comments, and math are structure or code. A closed content call that
//! shares a line with surrounding prose stays prose; the inline call is
//! kept atomic by the sentence splitter.

use crate::parser::{
    ByteSpan, FormatParser, Line, SpannedRegion, flush_prose_spanned, iter_lines, join_prose_gap,
    push_prose_line,
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
        let mut prose = String::new();
        let mut prose_span: Option<ByteSpan> = None;
        let mut term: Option<ByteSpan> = None;
        let mut hang: Option<Hang> = None;
        let mut depth = Depth::default();
        let mut pragma_off = false;
        let mut display_math = false;
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if display_math {
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
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
                }
                i += 1;
                continue;
            }

            if let Some(active) = hang.as_ref() {
                let indent = leading_spaces(line.text);
                if indent >= active.width && indent < line.text.len() {
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
                flush_open(
                    input,
                    &mut prose,
                    &mut prose_span,
                    &mut term,
                    &mut hang,
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
                    &mut regions,
                );
                let marker = ByteSpan::new(line.start, line.start + indent);
                regions.push(SpannedRegion::structure(input, marker));
                hang = Some(Hang {
                    width: indent,
                    empty_body: false,
                });
                push_hang_body(&mut prose, &mut prose_span, &mut term, &line, indent, false);
                i += 1;
                continue;
            }

            push_prose_line(&mut prose, &mut prose_span, &line, true, true);
            term = None;
            i += 1;
        }

        flush_open(
            input,
            &mut prose,
            &mut prose_span,
            &mut term,
            &mut hang,
            &mut regions,
        );
        regions
    }
}

fn flush_open(
    input: &str,
    prose: &mut String,
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
    prose: &mut String,
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
    let mut stack: Vec<u8> = Vec::new();
    loop {
        if i >= bytes.len() || bytes[i] == b'\n' {
            return Some(i.max(start + 1));
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
                } else {
                    return Some(i);
                }
            }
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
}
