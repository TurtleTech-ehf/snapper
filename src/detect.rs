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
//! compiler rejects, so those stay plaintext. A bare show body may be an
//! element function after a selector (`#show heading: strong`). A show-all
//! body has to be a content callable (`#show: strong`) or a set rule.
//! `#show notes: bring the figures.` and `#show: tonight` do not score.

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
/// Pairing is one forward pass: an unmatched opener does not hide the
/// lines after it, and a `~~~` line is never a fence. The info string is
/// ignored, so ```typst is still a fence when a closer exists.
fn lines_outside_fences(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    let marks: Vec<Option<BacktickFence>> = lines.iter().copied().map(backtick_fence).collect();
    let mut skip = vec![false; lines.len()];
    let mut open: Option<(usize, usize)> = None;
    for (i, mark) in marks.iter().enumerate() {
        let Some(mark) = mark else {
            continue;
        };
        if let Some((open_at, open_len)) = open {
            if mark.closer && mark.len >= open_len {
                for slot in &mut skip[open_at..=i] {
                    *slot = true;
                }
                open = None;
            }
            continue;
        }
        open = Some((i, mark.len));
    }
    lines
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !skip[*idx])
        .map(|(_, line)| line)
        .collect()
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

fn has_typst(lines: &[&str]) -> bool {
    lines
        .iter()
        .enumerate()
        .any(|(i, line)| typst_statement(line) || multiline_show(lines, i))
}

/// A show whose body opens a group on this line and closes on a later one.
/// `#show heading: it => [` ... `]` is one rule.
fn multiline_show(lines: &[&str], start: usize) -> bool {
    let line = lines[start].trim_start();
    let Some(rest) = keyword_rest(line, "#show") else {
        return false;
    };
    if delimiter_depth(rest) <= 0 {
        return false;
    }
    let mut buf = rest.to_string();
    for line in &lines[start + 1..] {
        buf.push('\n');
        buf.push_str(line);
        let depth = delimiter_depth(&buf);
        if depth == 0 {
            return show_rule(&buf);
        }
        if depth < 0 {
            return false;
        }
    }
    false
}

fn delimiter_depth(text: &str) -> i32 {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut string = false;
    let mut idx = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if string {
            if byte == b'\\' && idx + 1 < bytes.len() {
                idx += 2;
                continue;
            }
            if byte == b'"' {
                string = false;
            }
            idx += 1;
            continue;
        }
        match byte {
            b'"' => {
                string = true;
                idx += 1;
            }
            b'`' => idx = raw_end(bytes, idx),
            b'(' | b'[' | b'{' => {
                depth += 1;
                idx += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                idx += 1;
            }
            _ => idx += 1,
        }
    }
    depth
}

/// Index just after a raw span that starts at a backtick.
///
/// Two backticks are an empty raw. A longer run ends at the same run, and
/// an unclosed run consumes the rest, so `` `]` `` does not change depth.
fn raw_end(bytes: &[u8], idx: usize) -> usize {
    let mut ticks = 0;
    let mut i = idx;
    while i < bytes.len() && bytes[i] == b'`' {
        ticks += 1;
        i += 1;
    }
    if ticks == 2 {
        return i;
    }
    let mut found = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            found += 1;
            if found == ticks {
                return i + 1;
            }
        } else {
            found = 0;
        }
        i += 1;
    }
    bytes.len()
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
            return true;
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

/// The right-hand side finishes the statement, or a delimiter is still open
/// so the statement continues on the next line. Trailing prose does not.
fn expr_ends_statement(rhs: &str) -> bool {
    let rhs = rhs.trim();
    if rhs.is_empty() {
        return false;
    }
    expr_consumes(rhs) || delimiter_depth(rhs) > 0
}

/// `#set text(...)`. Typst 0.15.1 requires `(` immediately after the name,
/// and the argument list has to finish the line unless it is still open.
fn set_statement(rest: &str) -> bool {
    let rest = rest.trim_start();
    if set_body(rest) {
        return true;
    }
    let Some(mut idx) = ident_len(rest) else {
        return false;
    };
    loop {
        if rest[idx..].starts_with('(') {
            return skip_group(rest, idx).is_none();
        }
        let dot = skip_ws(rest, idx);
        if rest[dot..].starts_with('.') {
            let next = skip_ws(rest, dot + 1);
            let Some(len) = ident_len(&rest[next..]) else {
                return false;
            };
            idx = next + len;
            continue;
        }
        return false;
    }
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
/// not `math.foo` or `calc.pow`). After a selector, a bare body may be an
/// element function (`#show heading: strong`, `#show heading: h`). A show-all
/// body is a content callable or a set rule (`#show: strong`), not `h`,
/// `line`, or `bibliography`. `#show: tonight` does not score.
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
        return is_global_selector(name);
    }
    expr_consumes(body)
}

fn is_global_selector(name: &str) -> bool {
    GLOBAL_SELECTORS.binary_search(&name).is_ok()
}

fn is_show_all(name: &str) -> bool {
    SHOW_ALL.binary_search(&name).is_ok()
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
fn expr_consumes(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    if let Some(end) = numeric_end(text) {
        return text[end..].trim().is_empty();
    }
    if text.starts_with('[') || text.starts_with('{') {
        return group_consumes(text);
    }
    if text.starts_with('"') {
        return string_end(text).is_some_and(|end| text[end..].trim().is_empty());
    }
    if let Some(rest) = keyword_rest(text, "set") {
        return set_body(rest);
    }
    if text.starts_with('(') {
        let Some(end) = skip_group(text, 0) else {
            return false;
        };
        let after = text[end..].trim_start();
        if let Some(rhs) = after.strip_prefix("=>") {
            return expr_consumes(rhs);
        }
        return text[end..].trim().is_empty();
    }
    let Some(len) = ident_len(text) else {
        return false;
    };
    let after = text[len..].trim_start();
    if let Some(rhs) = after.strip_prefix("=>") {
        return expr_consumes(rhs);
    }
    if after.starts_with('.') || after.starts_with('(') {
        return chain_consumes(text);
    }
    text[len..].trim().is_empty()
}

fn set_body(rest: &str) -> bool {
    let rest = rest.trim_start();
    let Some(mut idx) = ident_len(rest) else {
        return false;
    };
    loop {
        if rest[idx..].starts_with('(') {
            let Some(end) = skip_group(rest, idx) else {
                return false;
            };
            return rest[end..].trim().is_empty();
        }
        let dot = skip_ws(rest, idx);
        if rest[dot..].starts_with('.') {
            let next = skip_ws(rest, dot + 1);
            let Some(len) = ident_len(&rest[next..]) else {
                return false;
            };
            idx = next + len;
            continue;
        }
        return false;
    }
}

fn group_consumes(text: &str) -> bool {
    let Some(end) = skip_group(text, 0) else {
        return false;
    };
    text[end..].trim().is_empty()
}

fn chain_consumes(text: &str) -> bool {
    let Some(mut idx) = ident_len(text) else {
        return false;
    };
    let mut saw_suffix = false;
    loop {
        if text[idx..].starts_with('(') {
            let Some(end) = skip_group(text, idx) else {
                return false;
            };
            idx = end;
            saw_suffix = true;
            continue;
        }
        let next = skip_ws(text, idx);
        if text[next..].starts_with('.') {
            let after = skip_ws(text, next + 1);
            let Some(len) = ident_len(&text[after..]) else {
                return false;
            };
            idx = after + len;
            saw_suffix = true;
            continue;
        }
        break;
    }
    saw_suffix && text[idx..].trim().is_empty()
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
    let mut idx = 1;
    while idx < bytes.len() {
        if bytes[idx] == b'\\' {
            idx += 2;
            continue;
        }
        if bytes[idx] == b'"' {
            return Some(idx + 1);
        }
        idx += 1;
    }
    None
}

fn skip_group(text: &str, open_idx: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let open = *bytes.get(open_idx)?;
    let close = match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        _ => return None,
    };
    let mut depth = 0i32;
    let mut idx = open_idx;
    let mut string = false;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if string {
            if byte == b'\\' && idx + 1 < bytes.len() {
                idx += 2;
                continue;
            }
            if byte == b'"' {
                string = false;
            }
            idx += 1;
            continue;
        }
        if byte == b'"' {
            string = true;
            idx += 1;
            continue;
        }
        if byte == b'`' {
            idx = raw_end(bytes, idx);
            continue;
        }
        if byte == open {
            depth += 1;
            idx += 1;
            continue;
        }
        if byte == close {
            depth -= 1;
            idx += 1;
            if depth == 0 {
                return Some(idx);
            }
            continue;
        }
        idx += 1;
    }
    None
}

/// End of a Typst number, including a real unit suffix (`pt`, `em`, `%`).
/// `1and` is not a number: the bad suffix stays outside the token.
fn numeric_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.first().is_none_or(|byte| !byte.is_ascii_digit()) {
        return None;
    }
    let mut idx = 1;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx + 1 < bytes.len() && bytes[idx] == b'.' && bytes[idx + 1].is_ascii_digit() {
        idx += 2;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
    }
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
    text[idx..]
        .find(|c: char| !c.is_whitespace())
        .map_or(text.len(), |offset| idx + offset)
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
        assert_text("#let alert(body, fill: red) = {\n", Format::Typst);
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
        assert_text(let_between, Format::Typst);
    }

    #[test]
    fn let_destructuring_is_typst() {
        assert_text("#let (a, b) = (1, 2)\n", Format::Typst);
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
            ("#show heading: h\n", Format::Typst),
            ("#show heading: line\n", Format::Typst),
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
        let elapsed = started.elapsed();
        assert_eq!(format, Format::Typst);
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "1 MiB of unclosed fences took {elapsed:?}"
        );
    }
}
