//! Score a document's format from its text.
//!
//! [`Format::detect`] uses a recognized path extension first. This module
//! handles the content-only case: every line of the document, with no
//! 20-line cap, and never a line inside a backtick fence that actually closes.
//! A `~~~` line is not a fence. In Typst `~` is a non-breaking space, so a
//! `#set` between two `~~~` lines still scores.
//!
//! A line counts only when Typst 0.15.1 would accept that construct.
//! A syntax tree still accepts `#show: h` and `math.foo`, which the
//! compiler rejects, so those stay plaintext. A bare show body is a name
//! Typst 0.15.1 compiles there (`#show heading: strong`, not
//! `#show heading: h`). A show-all body is a content callable
//! (`#show: strong`) or a set rule. One code-mode scan tracks strings,
//! line comments, nested block comments, raw spans, and brackets.
//! Delimiters and `#` count outside strings and comments. An open group
//! or an unclosed string keeps every following line inside that span, so
//! a later `#set` under `#let x = "hello (` is not its own signal. An
//! unclosed block comment keeps those lines as well, and the opening
//! statement still counts when the expression in front of the comment is
//! already complete. A finished right-hand side may use Typst's operators
//! (`1 + 2`, `not false`). `#show notes: bring the figures.` and
//! `#show: tonight` do not score.

use crate::format::Format;

/// Global element functions Typst 0.15.1 accepts as a show selector.
///
/// `counter-display` and math-only names such as `equation` are not in the
/// global scope. `h` and `v` are.
const GLOBAL_SELECTORS: &[&str] = &[
    "align",
    "bibliography",
    "block",
    "box",
    "circle",
    "cite",
    "colbreak",
    "columns",
    "curve",
    "divider",
    "document",
    "ellipse",
    "emph",
    "enum",
    "figure",
    "footnote",
    "grid",
    "h",
    "heading",
    "hide",
    "highlight",
    "image",
    "line",
    "linebreak",
    "link",
    "list",
    "metadata",
    "move",
    "outline",
    "overline",
    "pad",
    "page",
    "pagebreak",
    "par",
    "parbreak",
    "place",
    "polygon",
    "quote",
    "raw",
    "rect",
    "ref",
    "repeat",
    "rotate",
    "scale",
    "skew",
    "smallcaps",
    "smartquote",
    "square",
    "stack",
    "strike",
    "strong",
    "sub",
    "super",
    "table",
    "terms",
    "text",
    "title",
    "underline",
    "v",
];

/// Bare names that compile as a show-all rule (`#show: strong`).
///
/// Spacing and constructors such as `h`, `v`, `line`, and `bibliography`
/// are elements, but Typst 0.15.1 rejects them as a show-all body.
const SHOW_ALL: &[&str] = &[
    "align",
    "block",
    "box",
    "circle",
    "columns",
    "ellipse",
    "emph",
    "enum",
    "figure",
    "footnote",
    "grid",
    "heading",
    "hide",
    "highlight",
    "list",
    "lower",
    "metadata",
    "move",
    "overline",
    "pad",
    "page",
    "par",
    "place",
    "quote",
    "rect",
    "repeat",
    "rotate",
    "scale",
    "skew",
    "smallcaps",
    "square",
    "stack",
    "strike",
    "strong",
    "sub",
    "super",
    "table",
    "text",
    "title",
    "underline",
    "upper",
];

/// Bare names that compile as `#show heading: <name>` when a heading exists.
///
/// `heading` itself is a show-all callable, and applying it to `heading`
/// exceeds the show-rule depth. `h` and `line` are selectors, not bodies.
const SELECTOR_BODIES: &[&str] = &[
    "align",
    "block",
    "box",
    "circle",
    "columns",
    "ellipse",
    "emph",
    "enum",
    "figure",
    "footnote",
    "grid",
    "hide",
    "highlight",
    "list",
    "lower",
    "metadata",
    "move",
    "overline",
    "pad",
    "page",
    "par",
    "place",
    "quote",
    "rect",
    "repeat",
    "rotate",
    "scale",
    "skew",
    "smallcaps",
    "square",
    "stack",
    "strike",
    "strong",
    "sub",
    "super",
    "table",
    "text",
    "title",
    "underline",
    "upper",
];

/// Elements of the `math` module. `math.foo` and `math.abs` are not selectors.
const MATH_SELECTORS: &[&str] = &[
    "accent",
    "attach",
    "binom",
    "cancel",
    "cases",
    "class",
    "equation",
    "frac",
    "limits",
    "lr",
    "mat",
    "mid",
    "op",
    "overbrace",
    "overbracket",
    "overline",
    "overparen",
    "overshell",
    "primes",
    "root",
    "scripts",
    "stretch",
    "text",
    "underbrace",
    "underbracket",
    "underline",
    "underparen",
    "undershell",
    "vec",
];

/// Elements of the `pdf` module. `calc`, `html`, and `sys` export none.
const PDF_SELECTORS: &[&str] = &["artifact", "attach"];

pub(crate) fn score_text(text: &str) -> Format {
    let lines = lines_outside_fences(text);
    let mut latex = false;
    let mut org_line = false;
    let mut org_body = false;
    let mut markdown = false;
    let mut rst = false;
    for line in &lines {
        if is_latex_line(line) {
            latex = true;
        }
        if line.starts_with("#+") || line.starts_with("* ") {
            org_line = true;
        }
        if line.contains(":PROPERTIES:") || line.contains(":END:") || line.contains("#+begin_") {
            org_body = true;
        }
        if line.starts_with("# ") || line.starts_with("## ") {
            markdown = true;
        }
        if is_rst_line(line) {
            rst = true;
        }
    }
    let typst = has_typst(&lines);
    // A positive tally is that format's score. When several formats score,
    // the earlier one wins, which keeps a Markdown heading ahead of a Typst
    // statement and a Typst statement ahead of an RST underline.
    if latex {
        Format::Latex
    } else if org_line && org_body {
        Format::Org
    } else if markdown {
        Format::Markdown
    } else if typst {
        Format::Typst
    } else if rst {
        Format::Rst
    } else {
        Format::Plaintext
    }
}

fn is_latex_line(line: &str) -> bool {
    line.contains("\\begin{") || line.contains("\\section{") || line.contains("\\documentclass")
}

fn is_rst_line(line: &str) -> bool {
    line.contains(".. ") || (line.len() > 3 && line.chars().all(|c| c == '=' || c == '-'))
}

struct BacktickFence {
    len: usize,
    /// A line of only backticks, long enough to close an opener.
    closer: bool,
}

/// Lines that are not inside a closed ``` fence.
///
/// One forward pass and one buffer. A closer drops the opener and its
/// interior with `truncate`, so an unmatched opener does not rescan the
/// tail. A `~~~` line is never a fence. The info string is ignored, so
/// ```typst is still a fence when a closer exists.
fn lines_outside_fences(text: &str) -> Vec<&str> {
    let mut kept = Vec::new();
    // Index of the unmatched opener in `kept`, and its backtick length.
    let mut open: Option<(usize, usize)> = None;
    for line in text.lines() {
        if let Some(mark) = backtick_fence(line) {
            if let Some((at, len)) = open {
                if mark.closer && mark.len >= len {
                    kept.truncate(at);
                    open = None;
                    continue;
                }
            } else {
                open = Some((kept.len(), mark.len));
            }
        }
        kept.push(line);
    }
    kept
}

fn backtick_fence(line: &str) -> Option<BacktickFence> {
    let trimmed = line.trim();
    let bytes = trimmed.as_bytes();
    if bytes.first() != Some(&b'`') {
        return None;
    }
    let len = bytes.iter().take_while(|&&b| b == b'`').count();
    if len < 3 {
        return None;
    }
    let info = &trimmed[len..];
    if info.contains('`') {
        return None;
    }
    Some(BacktickFence {
        len,
        closer: info.is_empty(),
    })
}

/// A column-0 Typst statement. The keyword ends on whitespace, `(`, or `:`.
/// A hyphen continues the identifier, so `#import-export` is not `#import`.
fn typst_statement(line: &str) -> bool {
    let line = line.trim_start();
    if let Some(rest) = keyword_rest(line, "#let") {
        return let_statement(rest);
    }
    if let Some(rest) = keyword_rest(line, "#set") {
        return set_statement(rest);
    }
    if let Some(rest) = keyword_rest(line, "#show") {
        return show_rule(rest);
    }
    if let Some(rest) = keyword_rest(line, "#import") {
        return quoted_module(rest);
    }
    if let Some(rest) = keyword_rest(line, "#include") {
        return module_path(rest, true);
    }
    if let Some(rest) = keyword_rest(line, "#context") {
        return context_expr(rest);
    }
    false
}

fn keyword_rest<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    match rest.chars().next() {
        Some(c) if c.is_whitespace() || c == '(' || c == ':' => Some(rest),
        _ => None,
    }
}

fn starts_typst_keyword(line: &str) -> bool {
    let line = line.trim_start();
    keyword_rest(line, "#let").is_some()
        || keyword_rest(line, "#set").is_some()
        || keyword_rest(line, "#show").is_some()
        || keyword_rest(line, "#import").is_some()
        || keyword_rest(line, "#include").is_some()
        || keyword_rest(line, "#context").is_some()
}

/// A statement counts when its code scan closes and the finished text is
/// valid. `#let ieee(\n body\n) = body` closes. An open group, an unclosed
/// string, or an unclosed block comment owns every following line until
/// that span ends, so `#set text(size: 12pt)` under `#let x = "hello (` is
/// inside the string. A group or string that is still open at the end of
/// the block drops the tail. A block comment may run to the end of the
/// file; the opening statement still counts when its expression was
/// already complete.
fn has_typst(lines: &[&str]) -> bool {
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_start();
        if !starts_typst_keyword(line) {
            i += 1;
            continue;
        }
        let mut scan = CodeScan::default();
        scan.feed(lines[i]);
        if !scan.owns_next() {
            if scan.finished_span() && typst_statement(line) {
                return true;
            }
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < lines.len() && scan.owns_next() {
            scan.feed("\n");
            scan.feed(lines[j]);
            j += 1;
        }
        let buf = lines[i..j].join("\n");
        if scan.group_still_open() {
            return false;
        }
        if typst_statement(buf.trim_start()) {
            return true;
        }
        // The comment swallowed the rest of this statement. Lines inside
        // it are not their own evidence.
        if scan.block_comment > 0 {
            return false;
        }
        i = j;
    }
    false
}

/// Code-mode scan shared by statement grouping and group matching.
///
/// Strings (with `\"` escapes), line comments, nested block comments, and
/// raw spans hide delimiters and `#`. Parentheses and braces count in
/// code. Content brackets count on their own, so a `)` inside markup is
/// text. `feed` continues across lines in one pass.
#[derive(Default)]
struct CodeScan {
    code: i32,
    content: i32,
    string: bool,
    escape: bool,
    line_comment: bool,
    block_comment: usize,
    raw_need: usize,
    raw_have: usize,
    code_hash: bool,
    extra_close: bool,
}

impl CodeScan {
    fn feed(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let mut idx = 0;
        while idx < bytes.len() {
            idx = self.step(bytes, idx);
        }
    }

    /// The next line is still inside this span.
    fn owns_next(&self) -> bool {
        if self.extra_close {
            return false;
        }
        self.code > 0
            || self.content > 0
            || self.string
            || self.block_comment > 0
            || self.raw_need != 0
    }

    /// Brackets, the string, or a raw span are still open. A block comment
    /// is trivia Typst accepts through the end of the file, so it does not
    /// by itself keep the statement unfinished.
    fn group_still_open(&self) -> bool {
        !self.extra_close
            && (self.code > 0 || self.content > 0 || self.string || self.raw_need != 0)
    }

    fn finished_span(&self) -> bool {
        !self.extra_close
            && self.code == 0
            && self.content == 0
            && !self.string
            && self.block_comment == 0
            && self.raw_need == 0
    }

    fn step(&mut self, bytes: &[u8], idx: usize) -> usize {
        if idx >= bytes.len() {
            return idx;
        }
        let byte = bytes[idx];
        if self.raw_need != 0 {
            return self.step_raw(byte, idx);
        }
        if self.string {
            return self.step_string(byte, idx);
        }
        if self.line_comment {
            if byte == b'\n' {
                self.line_comment = false;
            }
            return idx + 1;
        }
        if self.block_comment > 0 {
            return self.step_block_comment(bytes, byte, idx);
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'/') {
            self.line_comment = true;
            return idx + 2;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            self.block_comment = 1;
            return idx + 2;
        }
        if byte == b'"' {
            self.string = true;
            return idx + 1;
        }
        if byte == b'`' {
            return self.step_raw_open(bytes, idx);
        }
        if self.content > 0 {
            if byte == b'[' {
                self.content += 1;
            } else if byte == b']' {
                self.content -= 1;
            }
            return idx + 1;
        }
        self.step_code(byte, idx)
    }

    fn step_raw(&mut self, byte: u8, idx: usize) -> usize {
        if byte == b'`' {
            self.raw_have += 1;
            if self.raw_have == self.raw_need {
                self.raw_need = 0;
                self.raw_have = 0;
            }
        } else {
            self.raw_have = 0;
        }
        idx + 1
    }

    fn step_string(&mut self, byte: u8, idx: usize) -> usize {
        if self.escape {
            self.escape = false;
            return idx + 1;
        }
        if byte == b'\\' {
            self.escape = true;
            return idx + 1;
        }
        if byte == b'"' {
            self.string = false;
        }
        idx + 1
    }

    fn step_block_comment(&mut self, bytes: &[u8], byte: u8, idx: usize) -> usize {
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            self.block_comment += 1;
            return idx + 2;
        }
        if byte == b'*' && bytes.get(idx + 1) == Some(&b'/') {
            self.block_comment -= 1;
            return idx + 2;
        }
        idx + 1
    }

    fn step_raw_open(&mut self, bytes: &[u8], idx: usize) -> usize {
        let mut end = idx + 1;
        while end < bytes.len() && bytes[end] == b'`' {
            end += 1;
        }
        let ticks = end - idx;
        // Two backticks are an empty raw. A longer run ends at the same
        // run, and an unclosed run consumes the rest, so `` `]` `` does
        // not change depth.
        if ticks != 2 {
            self.raw_need = ticks;
            self.raw_have = 0;
        }
        end
    }

    fn step_code(&mut self, byte: u8, idx: usize) -> usize {
        if byte == b'#' {
            self.code_hash = true;
            return idx + 1;
        }
        match byte {
            b'(' | b'{' => self.code += 1,
            b'[' => self.content += 1,
            b')' | b'}' => {
                self.code -= 1;
                if self.code < 0 {
                    self.extra_close = true;
                }
            }
            b']' => self.extra_close = true,
            _ => {}
        }
        idx + 1
    }
}

/// End of the group that opens at `open_idx`.
///
/// `reject_hash` is for code groups: a `#` in code rejects the group, and
/// a `#` inside a comment, string, raw span, or content block does not.
fn group_end(text: &str, open_idx: usize, reject_hash: bool) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut scan = CodeScan::default();
    let mut idx = open_idx;
    loop {
        if idx >= bytes.len() {
            return None;
        }
        idx = scan.step(bytes, idx);
        if scan.extra_close || (reject_hash && scan.code_hash) {
            return None;
        }
        let balanced = scan.code == 0 && scan.content == 0;
        let hidden = scan.string || scan.block_comment > 0 || scan.raw_need != 0;
        if balanced && !hidden {
            return Some(idx);
        }
    }
}

fn let_statement(rest: &str) -> bool {
    let rest = rest.trim_start();
    if rest.starts_with('(') {
        let Some(end) = skip_group(rest, 0) else {
            return false;
        };
        let Some(rhs) = rest[end..].trim_start().strip_prefix('=') else {
            return false;
        };
        return expr_ends_statement(rhs);
    }
    let Some(len) = ident_len(rest) else {
        return false;
    };
    let after = &rest[len..];
    if after.starts_with('(') {
        let Some(end) = skip_group(rest, len) else {
            return false;
        };
        let Some(rhs) = rest[end..].trim_start().strip_prefix('=') else {
            return false;
        };
        return expr_ends_statement(rhs);
    }
    let Some(rhs) = after.trim_start().strip_prefix('=') else {
        return false;
    };
    expr_ends_statement(rhs)
}

/// The right-hand side is one expression that ends the statement.
fn expr_ends_statement(rhs: &str) -> bool {
    let rhs = rhs.trim();
    !rhs.is_empty() && expr_consumes(rhs)
}

/// `#set text(...)`. Typst 0.15.1 requires `(` immediately after the name.
/// The argument list has to close; an unclosed `(` is not evidence.
fn set_statement(rest: &str) -> bool {
    set_body(rest.trim_start())
}

/// `#import "path"` / `#include "path"`. A label (`#include <stdio.h>`) is not
/// a Typst path, and the quote has to follow whitespace. After the string,
/// import allows the end of the line, `: items`, or `as name`. Include allows
/// only the end of the line. Trailing prose does not score.
fn quoted_module(rest: &str) -> bool {
    module_path(rest, false)
}

fn module_path(rest: &str, include: bool) -> bool {
    let rest = rest.trim_start();
    let Some(end) = string_end(rest) else {
        return false;
    };
    let after = rest[end..].trim_start();
    if include {
        return after.is_empty();
    }
    import_after_path(after)
}

fn import_after_path(after: &str) -> bool {
    if after.is_empty() {
        return true;
    }
    if let Some(rest) = keyword_rest(after, "as") {
        let rest = rest.trim_start();
        let Some(len) = ident_len(rest) else {
            return false;
        };
        let rest = rest[len..].trim_start();
        if rest.is_empty() {
            return true;
        }
        if let Some(items) = rest.strip_prefix(':') {
            return import_items(items);
        }
        return false;
    }
    if let Some(items) = after.strip_prefix(':') {
        return import_items(items);
    }
    false
}

fn import_items(text: &str) -> bool {
    let text = text.trim_start();
    if text.starts_with('(') {
        let Some(end) = skip_group(text, 0) else {
            return false;
        };
        if !text[end..].trim().is_empty() {
            return false;
        }
        let inner = text[1..end - 1].trim();
        return inner.is_empty() || import_items(inner);
    }
    if let Some(rest) = text.strip_prefix('*') {
        return rest.trim().is_empty();
    }
    let mut rest = text;
    loop {
        let Some(len) = ident_len(rest) else {
            return false;
        };
        rest = rest[len..].trim_start();
        while let Some(stripped) = rest.strip_prefix('.') {
            rest = stripped.trim_start();
            let Some(len) = ident_len(rest) else {
                return false;
            };
            rest = rest[len..].trim_start();
        }
        if let Some(renamed) = keyword_rest(rest, "as") {
            let renamed = renamed.trim_start();
            let Some(len) = ident_len(renamed) else {
                return false;
            };
            rest = renamed[len..].trim_start();
        }
        if rest.is_empty() {
            return true;
        }
        if let Some(next) = rest.strip_prefix(',') {
            rest = next.trim_start();
            if rest.is_empty() {
                return true;
            }
            continue;
        }
        return false;
    }
}

/// `#context counter(page)` or `#context text.lang`, not `#context opens`.
fn context_expr(rest: &str) -> bool {
    let rest = rest.trim_start();
    if rest.starts_with(['(', '[', '{']) {
        return true;
    }
    let Some(len) = ident_len(rest) else {
        return false;
    };
    let after = rest[len..].trim_start();
    after.starts_with('(') || after.starts_with('.')
}

/// `#show: body` or `#show <selector>: body`.
///
/// A bare selector is a global element (`heading`, `h`). A field selector's
/// head is that element, or an element of `math` or `pdf` (`math.equation`,
/// not `math.foo` or `calc.pow`). A bare body has to be a name that compiles
/// there (`#show heading: strong`, not `#show heading: h` or `#show heading: line`).
/// A show-all body is a content callable or a set rule (`#show: strong`).
/// `#show: tonight` does not score.
fn show_rule(rest: &str) -> bool {
    let rest = rest.trim_start();
    if let Some(body) = rest.strip_prefix(':') {
        return show_body(body, true);
    }
    let Some((end, bare)) = parse_selector(rest) else {
        return false;
    };
    if let Some(name) = bare {
        if !is_global_selector(name) {
            return false;
        }
    }
    let Some(body) = rest[end..].trim_start().strip_prefix(':') else {
        return false;
    };
    show_body(body, false)
}

fn show_body(body: &str, show_all: bool) -> bool {
    let body = body.trim();
    if body.is_empty() {
        return false;
    }
    if is_bare_ident(body) {
        let Some(len) = ident_len(body) else {
            return false;
        };
        let name = &body[..len];
        if show_all {
            return is_show_all(name);
        }
        return is_selector_body(name);
    }
    expr_consumes(body)
}

fn is_global_selector(name: &str) -> bool {
    GLOBAL_SELECTORS.binary_search(&name).is_ok()
}

fn is_show_all(name: &str) -> bool {
    SHOW_ALL.binary_search(&name).is_ok()
}

fn is_selector_body(name: &str) -> bool {
    SELECTOR_BODIES.binary_search(&name).is_ok()
}

fn module_selector(module: &str, elem: &str) -> bool {
    let list = match module {
        "math" => MATH_SELECTORS,
        "pdf" => PDF_SELECTORS,
        _ => return false,
    };
    list.binary_search(&elem).is_ok()
}

fn is_bare_ident(text: &str) -> bool {
    ident_len(text).is_some_and(|len| text[len..].trim().is_empty())
}

/// `true` when `text` is one expression that reaches the end of the line.
///
/// Binary and unary operators follow Typst 0.15.1 precedence: `* /`, then
/// `+ -`, then comparisons, `in`, and `not in`, then `and`, then `or`.
/// `not`, `+`, and `-` are unary. A trailing word with no operator stays
/// outside the expression.
fn expr_consumes(text: &str) -> bool {
    let text = text.trim();
    let Some(end) = expr_prec(text, 0, 0) else {
        return false;
    };
    skip_ws(text, end) == text.len()
}

struct CodeOp {
    end: usize,
    prec: u8,
}

fn expr_prec(text: &str, idx: usize, min_prec: u8) -> Option<usize> {
    let start = skip_ws(text, idx);
    if start >= text.len() {
        return None;
    }
    let mut idx = if let Some(op) = unary_op(text, start) {
        expr_prec(text, op.end, op.prec)?
    } else {
        primary(text, start)?
    };
    loop {
        let next = skip_ws(text, idx);
        let Some(op) = binary_op(text, next) else {
            return Some(idx);
        };
        if op.prec < min_prec {
            return Some(idx);
        }
        idx = expr_prec(text, op.end, op.prec + 1)?;
    }
}

fn unary_op(text: &str, idx: usize) -> Option<CodeOp> {
    let rest = text.get(idx..)?;
    if rest.starts_with('+') || rest.starts_with('-') {
        return Some(CodeOp {
            end: idx + 1,
            prec: 7,
        });
    }
    let (word, len) = word_at(text, idx)?;
    if word == "not" {
        return Some(CodeOp {
            end: idx + len,
            prec: 4,
        });
    }
    None
}

fn binary_op(text: &str, idx: usize) -> Option<CodeOp> {
    let rest = text.get(idx..)?;
    for (symbol, prec) in [
        ("==", 4u8),
        ("!=", 4),
        ("<=", 4),
        (">=", 4),
        ("+", 5),
        ("-", 5),
        ("*", 6),
        ("/", 6),
        ("<", 4),
        (">", 4),
    ] {
        if rest.starts_with(symbol) {
            return Some(CodeOp {
                end: idx + symbol.len(),
                prec,
            });
        }
    }
    let (word, len) = word_at(text, idx)?;
    if word == "not" {
        let after = skip_ws(text, idx + len);
        let (next, next_len) = word_at(text, after)?;
        if next == "in" {
            return Some(CodeOp {
                end: after + next_len,
                prec: 4,
            });
        }
        return None;
    }
    let prec = match word {
        "and" => 3,
        "or" => 2,
        "in" => 4,
        _ => return None,
    };
    Some(CodeOp {
        end: idx + len,
        prec,
    })
}

fn word_at(text: &str, idx: usize) -> Option<(&str, usize)> {
    let rest = text.get(idx..)?;
    let len = ident_len(rest)?;
    Some((&rest[..len], len))
}

fn primary(text: &str, idx: usize) -> Option<usize> {
    let idx = skip_ws(text, idx);
    if idx >= text.len() {
        return None;
    }
    let rest = &text[idx..];
    if let Some(end) = numeric_end(rest) {
        return Some(idx + end);
    }
    if rest.starts_with('[') {
        return skip_group(text, idx);
    }
    if rest.starts_with('(') || rest.starts_with('{') {
        let end = code_group_end(text, idx)?;
        if rest.starts_with('(') {
            let after = skip_ws(text, end);
            if text[after..].starts_with("=>") {
                return expr_prec(text, after + 2, 0);
            }
        }
        return Some(end);
    }
    if rest.starts_with('"') {
        return string_end(rest).map(|end| idx + end);
    }
    if let Some((word, len)) = word_at(text, idx) {
        if word == "set" {
            return set_end(text, skip_ws(text, idx + len));
        }
    }
    let (name, len) = word_at(text, idx)?;
    if is_operator_keyword(name) {
        return None;
    }
    if is_literal_keyword(name) {
        return Some(idx + len);
    }
    let mut end = idx + len;
    let mut extended = false;
    loop {
        if text[end..].starts_with('(') {
            end = code_group_end(text, end)?;
            extended = true;
            continue;
        }
        let dot = skip_ws(text, end);
        if text[dot..].starts_with('.') {
            let after = skip_ws(text, dot + 1);
            let field = ident_len(text.get(after..)?)?;
            end = after + field;
            extended = true;
            continue;
        }
        break;
    }
    if !extended {
        let after = skip_ws(text, end);
        if text[after..].starts_with("=>") {
            return expr_prec(text, after + 2, 0);
        }
    }
    Some(end)
}

fn is_literal_keyword(name: &str) -> bool {
    matches!(name, "none" | "auto" | "true" | "false")
}

fn is_operator_keyword(name: &str) -> bool {
    matches!(
        name,
        "and"
            | "or"
            | "not"
            | "in"
            | "let"
            | "show"
            | "context"
            | "if"
            | "else"
            | "for"
            | "while"
            | "break"
            | "continue"
            | "return"
            | "import"
            | "include"
            | "as"
    )
}

fn set_body(rest: &str) -> bool {
    let rest = rest.trim_start();
    set_end(rest, 0).is_some_and(|end| skip_ws(rest, end) == rest.len())
}

fn set_end(text: &str, idx: usize) -> Option<usize> {
    let mut idx = idx + ident_len(text.get(idx..)?)?;
    loop {
        if text[idx..].starts_with('(') {
            return code_group_end(text, idx);
        }
        let dot = skip_ws(text, idx);
        if text[dot..].starts_with('.') {
            let next = skip_ws(text, dot + 1);
            let len = ident_len(text.get(next..)?)?;
            idx = next + len;
            continue;
        }
        return None;
    }
}

/// End index of a selector, and the bare name when the selector is only an ident.
fn parse_selector(text: &str) -> Option<(usize, Option<&str>)> {
    if text.starts_with('"') {
        let end = string_end(text)?;
        let (end, _) = extend_chain(text, end)?;
        return Some((end, None));
    }
    if text.starts_with('<') {
        let end = label_end(text)?;
        return Some((end, None));
    }
    let len = ident_len(text)?;
    let name = &text[..len];
    if is_global_selector(name) {
        let end = selector_methods(text, len)?;
        let bare = if end == skip_ws(text, len) {
            Some(name)
        } else {
            None
        };
        return Some((end, bare));
    }
    if name == "math" || name == "pdf" {
        let end = module_element(text, len)?;
        let end = selector_methods(text, end)?;
        return Some((end, None));
    }
    None
}

/// `.where(...)`, then `.or(...)` / `.and(...)` on that selector.
fn selector_methods(text: &str, start: usize) -> Option<usize> {
    let mut idx = start;
    let mut saw_where = false;
    loop {
        let next = skip_ws(text, idx);
        if !text[next..].starts_with('.') {
            return Some(next);
        }
        let after = skip_ws(text, next + 1);
        let len = ident_len(&text[after..])?;
        let method = &text[after..after + len];
        let call = after + len;
        if !text[call..].starts_with('(') {
            return None;
        }
        let allowed = method == "where" || (saw_where && (method == "or" || method == "and"));
        if !allowed {
            return None;
        }
        if method == "where" {
            saw_where = true;
        }
        idx = skip_group(text, call)?;
    }
}

fn module_element(text: &str, start: usize) -> Option<usize> {
    let module = &text[..start];
    let dot = skip_ws(text, start);
    if !text[dot..].starts_with('.') {
        return None;
    }
    let after = skip_ws(text, dot + 1);
    let len = ident_len(&text[after..])?;
    let elem = &text[after..after + len];
    if !module_selector(module, elem) {
        return None;
    }
    Some(after + len)
}

/// Index just before the colon, and whether a field or call was consumed.
fn extend_chain(text: &str, start: usize) -> Option<(usize, bool)> {
    let mut idx = start;
    let mut extended = false;
    loop {
        let next = skip_ws(text, idx);
        if text[next..].starts_with('.') {
            let after = skip_ws(text, next + 1);
            let len = ident_len(&text[after..])?;
            idx = after + len;
            extended = true;
            continue;
        }
        if text[idx..].starts_with('(') {
            idx = skip_group(text, idx)?;
            extended = true;
            continue;
        }
        return Some((next, extended));
    }
}

fn label_end(text: &str) -> Option<usize> {
    let inner = text.strip_prefix('<')?;
    let end = inner.find('>')?;
    let name = &inner[..end];
    if ident_len(name) == Some(name.len()) {
        Some(end + 2)
    } else {
        None
    }
}

fn string_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut scan = CodeScan::default();
    let mut idx = 0;
    while idx < bytes.len() && (idx == 0 || scan.string) {
        idx = scan.step(bytes, idx);
    }
    if scan.string { None } else { Some(idx) }
}

/// End of a code group. A `#` in code is rejected; a `#` inside a content
/// block, string, raw span, or comment is kept. Typst reports `#set`
/// inside `(...)` as "the character `#` is not valid in code".
fn code_group_end(text: &str, open_idx: usize) -> Option<usize> {
    match text.as_bytes().get(open_idx).copied() {
        Some(b'(' | b'{') => group_end(text, open_idx, true),
        _ => None,
    }
}

fn skip_group(text: &str, open_idx: usize) -> Option<usize> {
    match text.as_bytes().get(open_idx).copied() {
        Some(b'(' | b'[' | b'{') => group_end(text, open_idx, false),
        _ => None,
    }
}

/// End of a Typst number, including a real unit suffix (`pt`, `em`, `%`).
/// `1and` is not a number: the bad suffix stays outside the token.
fn numeric_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut idx = if bytes[0].is_ascii_digit() {
        let mut idx = 1;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        // `2.` is a float. `1.foo` keeps the dot for a field, and `..` is not
        // a decimal point.
        if idx < bytes.len() && bytes[idx] == b'.' && !text[idx..].starts_with("..") {
            let after = idx + 1;
            let ident_start = text[after..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_');
            if !ident_start {
                idx = after;
                while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                    idx += 1;
                }
            }
        }
        idx
    } else if bytes[0] == b'.' && bytes.get(1).is_some_and(|byte| byte.is_ascii_digit()) {
        let mut idx = 2;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        idx
    } else {
        return None;
    };
    if idx < bytes.len() && matches!(bytes[idx], b'e' | b'E') && !text[idx..].starts_with("em") {
        let mut exp = idx + 1;
        if exp < bytes.len() && matches!(bytes[exp], b'+' | b'-') {
            exp += 1;
        }
        if exp >= bytes.len() || !bytes[exp].is_ascii_digit() {
            return None;
        }
        exp += 1;
        while exp < bytes.len() && bytes[exp].is_ascii_digit() {
            exp += 1;
        }
        idx = exp;
    }
    if let Some(unit) = unit_len(&text[idx..]) {
        idx += unit;
    }
    Some(idx)
}

fn unit_len(text: &str) -> Option<usize> {
    const UNITS: &[&str] = &["pt", "mm", "cm", "in", "deg", "rad", "em", "fr", "%"];
    for unit in UNITS {
        let Some(rest) = text.strip_prefix(unit) else {
            continue;
        };
        let boundary = rest
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric());
        if boundary {
            return Some(unit.len());
        }
    }
    None
}

fn skip_ws(text: &str, idx: usize) -> usize {
    let bytes = text.as_bytes();
    let mut scan = CodeScan::default();
    let mut i = idx;
    while i < bytes.len() {
        if scan.line_comment || scan.block_comment > 0 {
            let next = scan.step(bytes, i);
            if next == i {
                break;
            }
            i = next;
            continue;
        }
        let Some(ch) = text[i..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            i += ch.len_utf8();
            continue;
        }
        if bytes[i] == b'/' && matches!(bytes.get(i + 1), Some(&b'/') | Some(&b'*')) {
            i = scan.step(bytes, i);
            continue;
        }
        break;
    }
    i
}

/// Length of a Typst identifier. A hyphen is part of the name when a
/// letter, digit, or underscore follows it, as in `script-size`.
fn ident_len(text: &str) -> Option<usize> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut len = first.len_utf8();
    while let Some(ch) = text[len..].chars().next() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            len += ch.len_utf8();
            continue;
        }
        if ch == '-' {
            let after = &text[len + ch.len_utf8()..];
            let Some(next) = after.chars().next() else {
                break;
            };
            if next.is_ascii_alphanumeric() || next == '_' {
                len += ch.len_utf8() + next.len_utf8();
                continue;
            }
        }
        break;
    }
    Some(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn assert_text(text: &str, format: Format) {
        assert_eq!(Format::detect(None, text), format, "{text:?}");
    }

    #[test]
    fn element_names_match_typst_0_15() {
        assert!(GLOBAL_SELECTORS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(SHOW_ALL.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(MATH_SELECTORS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(is_global_selector("heading"));
        assert!(is_global_selector("h"));
        assert!(is_global_selector("v"));
        assert!(is_global_selector("strong"));
        assert!(!is_global_selector("counter-display"));
        assert!(!is_global_selector("equation"));
        assert!(!is_global_selector("notes"));
        assert!(is_show_all("strong"));
        assert!(is_show_all("emph"));
        assert!(!is_show_all("h"));
        assert!(!is_show_all("v"));
        assert!(!is_show_all("bibliography"));
        assert!(!is_show_all("line"));
        assert!(module_selector("math", "equation"));
        assert!(!module_selector("math", "foo"));
        assert!(!module_selector("math", "abs"));
        assert!(!module_selector("calc", "pow"));
    }

    #[test]
    fn fixture_files_are_typst() {
        // `#set` in these files sits past the old 20-line window.
        for sample in [
            include_str!("../tests/fixtures/sample.typ"),
            include_str!("../tests/fixtures/expected.typ"),
            include_str!("../examples/paper.typ"),
        ] {
            assert_eq!(Format::detect(None, sample), Format::Typst);
            let set_at = sample
                .lines()
                .position(|line| line.contains("#set text"))
                .expect("fixture has #set");
            assert!(set_at >= 20, "line {}", set_at + 1);
        }
    }

    #[test]
    fn path_extension_wins_over_content() {
        assert_eq!(
            Format::detect(Some(Path::new("examples/paper.typ")), "# Title\n"),
            Format::Typst
        );
        assert_eq!(
            Format::detect(Some(Path::new("notes.md")), "#let ieee(\n"),
            Format::Markdown
        );
        assert_eq!(
            Format::detect(Some(Path::new("notes.txt")), "#set text(size: 12pt)\n"),
            Format::Plaintext
        );
        assert_eq!(
            Format::detect(Some(Path::new("src/main.rs")), "#set text(size: 12pt)\n"),
            Format::Typst
        );
        assert_eq!(
            Format::detect(Some(Path::new("paper.typ")), ""),
            Format::Typst
        );
    }

    #[test]
    fn template_openings_are_typst() {
        let charged_ieee = "\
// IEEE-style article.
#let ieee(
  title: [Paper Title],
  body
) = {}
";
        let unequivocal_ams = "\
// Sizes used across the template.
#let script-size = 7.97224pt
#let footnote-size = 8.50012pt
";
        let appreciated_letter = "\
// A simple letter.
#let letter(
  sender: none,
  body
) = {}
";
        let wonderous_book = "\
// A fiction book.
#let book(
  title: [Book title],
  body,
) = {}
";
        for sample in [
            charged_ieee,
            unequivocal_ams,
            appreciated_letter,
            wonderous_book,
        ] {
            assert_text(sample, Format::Typst);
        }
        assert_text("#let alert(body, fill: red) = {\n", Format::Plaintext);
        assert_text("#let alert(body, fill: red) = {}\n", Format::Typst);
        assert_text("#let s = 1\nHello there.\n", Format::Typst);
        assert_text("#set text(size: 12pt)\nHello there.\n", Format::Typst);
        assert_text("#show heading.where(level: 1): it => it\n", Format::Typst);
        assert_text("#show heading: it => it\n", Format::Typst);
        assert_text("#show: doc => doc\n", Format::Typst);
        assert_text("#show: ieee.with(title: [Paper])\n", Format::Typst);
        assert_text("#show raw: set text(size: 9pt)\n", Format::Typst);
        assert_text("#import \"chapter.typ\": item\n", Format::Typst);
        assert_text("#include \"chapter.typ\"\n", Format::Typst);
        assert_text("#context counter(page)\n", Format::Typst);
        assert_text("#context text.lang\n", Format::Typst);
    }

    #[test]
    fn a_statement_past_the_old_window_still_counts() {
        let mut lines = vec!["Just a sentence. Another one.".to_string(); 24];
        lines.push("#let script-size = 7.97224pt".to_string());
        assert_text(&lines.join("\n"), Format::Typst);

        let mut headings = vec!["Hello there.".to_string(); 24];
        headings.push("# A late heading".to_string());
        assert_text(&headings.join("\n"), Format::Markdown);
    }

    #[test]
    fn hyphenated_words_and_prose_are_not_typst() {
        assert_text(
            "#import-export is the tag. More follows.\n",
            Format::Plaintext,
        );
        assert_text("#context-menu opens on right click.\n", Format::Plaintext);
        assert_text("#include <stdio.h>\nint main;\n", Format::Plaintext);
        assert_text("#include<stdio.h>\nint main;\n", Format::Plaintext);
        assert_text("#context opens on right click. Next.\n", Format::Plaintext);
        assert_text("#let me explain the result. Next.\n", Format::Plaintext);
        assert_text("#set the table for dinner. Next.\n", Format::Plaintext);
        assert_text("#show your work in full. Next.\n", Format::Plaintext);
        assert_text(
            "#importing a module is discussed here.\n",
            Format::Plaintext,
        );
        assert_text("#show notes: bring the figures.\n", Format::Plaintext);
        assert_text("#show: tonight\n", Format::Plaintext);
        assert_text("#show notes: it => it\n", Format::Plaintext);
        assert_text("#show notes : it => it\n", Format::Plaintext);
    }

    #[test]
    fn closed_fences_do_not_decide_the_format() {
        let c_fence = "\
```c
#include <stdio.h>
#include<stdio.h>
int main(void) { return 0; }
```
Run the program. Then stop.
";
        assert_text(c_fence, Format::Plaintext);

        let typst_fence = "\
```typst
#let ieee(
#show heading.where(level: 1): it => it
#show notes: it => it
```
See the sample. Next sentence.
";
        assert_text(typst_fence, Format::Plaintext);

        let show_fence = "\
```
#show notes: it => it
```
Hello world. This stays prose.
";
        assert_text(show_fence, Format::Plaintext);

        let heading_inside = "\
```md
# Title
```
Hello world. This stays prose.
";
        assert_text(heading_inside, Format::Plaintext);

        let latex_inside = "\
```tex
\\documentclass{article}
```
Hello world. This stays prose.
";
        assert_text(latex_inside, Format::Plaintext);

        let outside = "\
```c
#include <stdio.h>
```
#let script-size = 7.97224pt
";
        assert_text(outside, Format::Typst);

        let heading_inside_statement_outside = "\
```md
# Title
```
#set text(size: 12pt)
";
        assert_text(heading_inside_statement_outside, Format::Typst);

        // A tilde line does not close a backtick fence.
        let mismatched = "\
```c
#include <stdio.h>
~~~
#let script-size = 1pt
```
After the fence.
";
        assert_text(mismatched, Format::Plaintext);
    }

    #[test]
    fn an_unclosed_fence_marker_is_not_a_fence() {
        let open_let = "\
```typst
#let ieee(
  body
) = {}
";
        assert_text(open_let, Format::Typst);

        let open_heading = "\
```md
# Title
Hello from inside an unclosed fence.
";
        assert_text(open_heading, Format::Markdown);

        let open_latex = "\
```tex
\\documentclass{article}
Hello.
";
        assert_text(open_latex, Format::Latex);
    }

    #[test]
    fn markdown_headings_win_over_typst_statements() {
        assert_text("# Title\n\n#let me explain. Next.\n", Format::Markdown);
        assert_text("# Title\n\n#set the table. Next.\n", Format::Markdown);
        assert_text("## Notes\n\n#show your work. Next.\n", Format::Markdown);
        let notes = "\
# Notes

Call #let ieee( when defining a helper. Next sentence.
The binding #let script-size = 1pt is just an example.
";
        assert_text(notes, Format::Markdown);
        let mixed = "\
# Title

#let ieee(
  body
) = {}
#let letter(
#let book(
";
        assert_text(mixed, Format::Markdown);
    }

    #[test]
    fn other_formats_still_score() {
        assert_text("\\documentclass{article}\n# Title\nHello.\n", Format::Latex);
        assert_text(
            "* Heading\n:PROPERTIES:\n:ID: abc\n:END:\n# Title\n",
            Format::Org,
        );
        assert_text("# Title\n\nHello there.\n", Format::Markdown);
        assert_text("Title\n====\n\nA paragraph. Next.\n", Format::Rst);
        assert_text("#let script-size = 1pt\n====\n", Format::Typst);
    }

    #[test]
    fn show_body_may_be_a_bare_element() {
        assert_text("#show heading: strong\n", Format::Typst);
        assert_text("#show \"hello\": strong\n", Format::Typst);
        assert_text("#show: tonight\n", Format::Plaintext);
    }

    #[test]
    fn spacing_elements_are_show_selectors() {
        assert_text("#show h: it => it\n", Format::Typst);
        assert_text("#show v: it => it\n", Format::Typst);
    }

    #[test]
    fn show_content_block_may_span_lines() {
        let rule = "\
#show heading: it => [
  The measured curve.
]
";
        assert_text(rule, Format::Typst);
    }

    #[test]
    fn dotted_selector_head_must_be_known() {
        assert_text("#show notes.where(level: 1): it => it\n", Format::Plaintext);
        assert_text("#show heading.where(level: 1): it => it\n", Format::Typst);
        assert_text("#show math.equation: it => it\n", Format::Typst);
    }

    #[test]
    fn tilde_lines_do_not_hide_typst() {
        let between = "\
~~~
#set text(size: 12pt)
~~~
See the sample.
";
        assert_text(between, Format::Typst);
        let let_between = "\
~~~c
#let ieee(
~~~
See the sample. Next sentence.
";
        assert_text(let_between, Format::Plaintext);
    }

    #[test]
    fn let_destructuring_is_typst() {
        assert_text("#let (a, b) = (1, 2)\n", Format::Typst);
    }

    #[test]
    fn an_open_call_counts_only_when_it_closes() {
        let closed = "\
#let ieee(
  body
) = body
";
        assert_text(closed, Format::Typst);
        assert_text("#let ieee( bring the figures.\n", Format::Plaintext);
        assert_text("#let x = (1\n", Format::Plaintext);
        assert_text("#set text( bring the figures.\n", Format::Plaintext);
        assert_text(
            "#let ieee( bring the figures.\n#set text(size: 12pt)\n",
            Format::Plaintext,
        );
        assert_text("#let x = (1\n#set text(size: 12pt)\n", Format::Plaintext);
        assert_text(
            "#set text( bring the figures.\n#let x = 1\n",
            Format::Plaintext,
        );
        assert_text(
            "#let ieee(\n  body\n) = body\n#set text(size: 12pt)\n",
            Format::Typst,
        );
        assert_text("#let x = (1\n)\n#set text(size: 12pt)\n", Format::Typst);
        assert_text(
            "#set text(size: 12pt)\n#let ieee( bring the figures.\n",
            Format::Typst,
        );
    }

    #[test]
    fn charged_ieee_lib_scores_from_content_without_a_typ_extension() {
        // MIT-0 @preview/charged-ieee:0.1.4 lib.typ. A // comment in the
        // caption block carries a URL fragment `#discussion_r...`. The
        // path has no extension, so the `.typ` suffix cannot decide.
        let src = include_str!("../tests/fixtures/charged-ieee-lib.typ");
        assert!(src.contains("#discussion_r2112947947"));
        assert_eq!(Format::detect(None, src), Format::Typst);
        assert_eq!(
            Format::detect(Some(Path::new("charged-ieee/lib")), src),
            Format::Typst
        );
    }

    #[test]
    fn expr_probe_table_matches_detection() {
        let table = include_str!("../tests/fixtures/typst-0.15.1-expressions.tsv");
        for line in table.lines() {
            if line.is_empty() || !line.contains('\t') || line.starts_with("source\t") {
                continue;
            }
            let mut cols = line.split('\t');
            let source = cols.next().expect("source").replace("\\n", "\n");
            let compiles = cols.next().expect("compiles");
            let detect = cols.next().expect("detect");
            assert!(
                compiles == "yes" || compiles == "no",
                "compiles column {compiles:?}"
            );
            let want = if detect == "typst" {
                Format::Typst
            } else if detect == "plaintext" {
                Format::Plaintext
            } else {
                panic!("detect column {detect:?}");
            };
            assert_text(&source, want);
        }
    }

    #[test]
    fn show_body_probe_table_matches_detection() {
        let table = include_str!("../tests/fixtures/typst-0.15.1-show-bodies.tsv");
        for line in table.lines() {
            if line.is_empty() || line.starts_with('#') || line.starts_with("name\t") {
                continue;
            }
            let mut cols = line.split('\t');
            let name = cols.next().expect("name");
            let show_all = cols.next().expect("show-all") == "yes";
            let selector_body = cols.next().expect("selector body") == "yes";
            assert_eq!(is_show_all(name), show_all, "show-all list {name}");
            assert_eq!(
                is_selector_body(name),
                selector_body,
                "selector-body list {name}"
            );
            let show_all_src = format!("#show: {name}\n");
            let selector_src = format!("#show heading: {name}\n");
            assert_eq!(
                Format::detect(None, &show_all_src) == Format::Typst,
                show_all,
                "{show_all_src:?}"
            );
            assert_eq!(
                Format::detect(None, &selector_src) == Format::Typst,
                selector_body,
                "{selector_src:?}"
            );
        }
    }

    #[test]
    fn evidence_matches_what_typst_compiles() {
        let cases = [
            ("#let (a, b) = (1, 2)\n", Format::Typst),
            ("#let (me) = explain the figures.\n", Format::Plaintext),
            (
                "#let (a, b) = (1, 2) and then more prose.\n",
                Format::Plaintext,
            ),
            (
                "#let script-size = 1pt and more prose.\n",
                Format::Plaintext,
            ),
            ("#show: strong\n", Format::Typst),
            ("#show: emph\n", Format::Typst),
            ("#show: h\n", Format::Plaintext),
            ("#show: v\n", Format::Plaintext),
            ("#show: bibliography\n", Format::Plaintext),
            ("#show: line\n", Format::Plaintext),
            ("#show: counter-display\n", Format::Plaintext),
            ("#show math.foo: it => it\n", Format::Plaintext),
            ("#show calc.pow: it => it\n", Format::Plaintext),
            ("#show math.equation: it => it\n", Format::Typst),
            ("#show heading.where(level: 1): it => it\n", Format::Typst),
            ("#show heading: h\n", Format::Plaintext),
            ("#show heading: line\n", Format::Plaintext),
            ("#let ieee( bring the figures.\n", Format::Plaintext),
            ("#let x = (1\n", Format::Plaintext),
            ("#set text( bring the figures.\n", Format::Plaintext),
            ("#let x = 1 + 2\n", Format::Typst),
            ("#let x = not false\n", Format::Typst),
            ("#let x = 1 not in (2, 3)\n", Format::Typst),
            ("#let x = 1 +\n", Format::Plaintext),
            ("#let x = 1 + 2 more\n", Format::Plaintext),
        ];
        for (text, format) in cases {
            assert_text(text, format);
        }
    }

    #[test]
    fn show_raw_bracket_does_not_close_the_block() {
        let rule = "\
#show heading: it => [
`]`
]
";
        assert_text(rule, Format::Typst);
    }

    #[test]
    fn import_and_include_reject_trailing_prose() {
        assert_text("#import \"chapter.typ\": item\n", Format::Typst);
        assert_text("#import \"chapter.typ\" as chapter\n", Format::Typst);
        assert_text("#include \"chapter.typ\"\n", Format::Typst);
        assert_text(
            "#import \"chapter.typ\" is how modules load.\n",
            Format::Plaintext,
        );
        assert_text(
            "#include \"chapter.typ\" is how modules load.\n",
            Format::Plaintext,
        );
        assert_text("#include \"chapter.typ\": item\n", Format::Plaintext);
        assert_text("#include \"chapter.typ\" as chapter\n", Format::Plaintext);
    }

    #[test]
    fn set_argument_list_is_immediate() {
        assert_text("#set text(size: 12pt)\n", Format::Typst);
        assert_text("#set text (size: 12pt)\n", Format::Plaintext);
    }

    #[test]
    fn unclosed_fences_detect_a_megabyte_quickly() {
        let line = "```not-a-closer\n";
        let mut text = String::with_capacity(1024 * 1024 + line.len() + 32);
        while text.len() < 1024 * 1024 {
            text.push_str(line);
        }
        text.push_str("#set text(size: 12pt)\n");
        let started = std::time::Instant::now();
        let format = Format::detect(None, &text);
        let one = started.elapsed();
        assert_eq!(format, Format::Typst);
        assert!(
            one.as_secs_f64() < 1.0,
            "1 MiB of unclosed fences took {one:?}"
        );

        let mut larger = String::with_capacity(8 * 1024 * 1024 + line.len());
        while larger.len() < 8 * 1024 * 1024 {
            larger.push_str(line);
        }
        larger.push_str("#set text(size: 12pt)\n");
        let started = std::time::Instant::now();
        let format = Format::detect(None, &larger);
        let eight = started.elapsed();
        assert_eq!(format, Format::Typst);
        assert!(
            eight.as_secs_f64() < 2.0
                && eight.as_secs_f64() < one.as_secs_f64().mul_add(20.0, 0.25),
            "8 MiB took {eight:?} after 1 MiB took {one:?}"
        );
    }

    #[test]
    fn an_unclosed_let_stays_linear() {
        let mut text = String::from("#let ieee(\n");
        let line = "bring the figures.\n";
        while text.len() < 1024 * 1024 {
            text.push_str(line);
        }
        let started = std::time::Instant::now();
        let format = Format::detect(None, &text);
        let elapsed = started.elapsed();
        assert_eq!(format, Format::Plaintext);
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "1 MiB after an unclosed let took {elapsed:?}"
        );
    }
}
