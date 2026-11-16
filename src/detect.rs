//! Score a document's format from its text.
//!
//! [`Format::detect`] uses a recognized path extension first. This module
//! handles the content-only case: every line of the document, with no
//! 20-line cap, and never a line inside a fence that actually closes.
//!
//! The `typst` crate is not a dependency, so Typst statements are checked
//! against the Typst 0.15.1 grammar directly. A `#show` rule needs a
//! selector and a body that grammar accepts, and the body has to be more
//! than a bare name. `#show notes: bring the figures.` and `#show: tonight`
//! do not score.

use crate::format::Format;

/// Element names exposed by Typst 0.15.1.
///
/// Each name is the `#[elem]` `name`, or the kebab-case of the struct with
/// the `Elem` suffix removed. A bare `#show` selector has to be one of these.
const ELEMENTS: &[&str] = &[
    "accent",
    "align",
    "align-point",
    "artifact",
    "asset",
    "attach",
    "bibliography",
    "binom",
    "block",
    "box",
    "cancel",
    "caption",
    "cases",
    "cell",
    "circle",
    "cite",
    "cite-group",
    "class",
    "close",
    "colbreak",
    "columns",
    "context",
    "counter-display",
    "counter-update",
    "csl-indent",
    "csl-light",
    "cubic",
    "curve",
    "direct-link",
    "divider",
    "document",
    "ellipse",
    "emph",
    "entry",
    "enum",
    "equation",
    "figure",
    "flush",
    "footer",
    "footnote",
    "footnote-container",
    "footnote-marker",
    "frac",
    "grid",
    "header",
    "heading",
    "hide",
    "highlight",
    "hline",
    "image",
    "inline",
    "item",
    "layout",
    "limits",
    "line",
    "linebreak",
    "link",
    "link-marker",
    "list",
    "lr",
    "mat",
    "metadata",
    "mid",
    "move",
    "op",
    "outline",
    "overbrace",
    "overbracket",
    "overline",
    "overparen",
    "overshell",
    "pad",
    "page",
    "pagebreak",
    "par",
    "par-line-marker",
    "parbreak",
    "pdf-marker-tag",
    "place",
    "polygon",
    "primes",
    "quad",
    "quote",
    "raw",
    "rect",
    "ref",
    "repeat",
    "root",
    "rotate",
    "scale",
    "scripts",
    "sequence",
    "skew",
    "smallcaps",
    "smartquote",
    "space",
    "square",
    "stack",
    "state-update",
    "stretch",
    "strike",
    "strong",
    "styled",
    "sub",
    "super",
    "symbol",
    "table",
    "tag",
    "target",
    "terms",
    "text",
    "title",
    "underbrace",
    "underbracket",
    "underline",
    "underparen",
    "undershell",
    "vec",
    "vline",
];

pub(crate) fn score_text(text: &str) -> Format {
    let lines = lines_outside_fences(text);
    let mut latex = false;
    let mut org_line = false;
    let mut org_body = false;
    let mut markdown = false;
    let mut typst = false;
    let mut rst = false;
    for line in lines {
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
        if typst_statement(line) {
            typst = true;
        }
    }
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

struct OpenFence {
    marker: u8,
    len: usize,
}

/// Lines that are not inside a closed ``` or ~~~ fence.
///
/// The info string is ignored, so ```typst is still a fence. A marker with
/// no matching closer is ordinary text, and the lines after it are scored.
fn lines_outside_fences(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    let mut skip = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if let Some(open) = fence_opens(lines[i]) {
            if let Some(rel) = lines[i + 1..]
                .iter()
                .position(|line| fence_closes(line, &open))
            {
                let end = i + 1 + rel;
                for slot in &mut skip[i..=end] {
                    *slot = true;
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    lines
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !skip[*idx])
        .map(|(_, line)| line)
        .collect()
}

fn fence_opens(line: &str) -> Option<OpenFence> {
    let trimmed = line.trim();
    let bytes = trimmed.as_bytes();
    let marker = *bytes.first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let len = bytes.iter().take_while(|&&b| b == marker).count();
    if len < 3 {
        return None;
    }
    let info = &trimmed[len..];
    if marker == b'`' && info.contains('`') {
        return None;
    }
    Some(OpenFence { marker, len })
}

fn fence_closes(line: &str, open: &OpenFence) -> bool {
    let bytes = line.trim().as_bytes();
    bytes.len() >= open.len && bytes.iter().all(|&b| b == open.marker)
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
        return quoted_module(rest);
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

fn let_statement(rest: &str) -> bool {
    let rest = rest.trim_start();
    let Some(len) = ident_len(rest) else {
        return false;
    };
    let after = rest[len..].trim_start();
    after.starts_with('=') || after.starts_with('(')
}

fn set_statement(rest: &str) -> bool {
    let rest = rest.trim_start();
    let Some(mut idx) = ident_len(rest) else {
        return false;
    };
    loop {
        idx = skip_ws(rest, idx);
        if rest[idx..].starts_with('.') {
            let next = skip_ws(rest, idx + 1);
            let Some(len) = ident_len(&rest[next..]) else {
                return false;
            };
            idx = next + len;
            continue;
        }
        return rest[idx..].starts_with('(');
    }
}

/// `#import "path"` / `#include "path"`. A label (`#include <stdio.h>`) is not
/// a Typst path, and the quote has to follow whitespace.
fn quoted_module(rest: &str) -> bool {
    rest.trim_start().starts_with('"')
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
/// A bare selector has to be a Typst 0.15.1 element (`heading`, `raw`).
/// The body has to parse as one expression and must not be only a name,
/// so `#show: tonight` and `#show notes: bring the figures.` do not score.
fn show_rule(rest: &str) -> bool {
    let rest = rest.trim_start();
    if let Some(body) = rest.strip_prefix(':') {
        return show_body(body);
    }
    let Some((end, bare)) = parse_selector(rest) else {
        return false;
    };
    if let Some(name) = bare {
        if !is_element(name) {
            return false;
        }
    }
    let Some(body) = rest[end..].trim_start().strip_prefix(':') else {
        return false;
    };
    show_body(body)
}

fn show_body(body: &str) -> bool {
    let body = body.trim();
    if body.is_empty() || is_bare_ident(body) {
        return false;
    }
    expr_consumes(body)
}

fn is_element(name: &str) -> bool {
    ELEMENTS.binary_search(&name).is_ok()
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
        idx = skip_ws(rest, idx);
        if rest[idx..].starts_with('.') {
            let next = skip_ws(rest, idx + 1);
            let Some(len) = ident_len(&rest[next..]) else {
                return false;
            };
            idx = next + len;
            continue;
        }
        if rest[idx..].starts_with('(') {
            let Some(end) = skip_group(rest, idx) else {
                return false;
            };
            return rest[end..].trim().is_empty();
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
        idx = skip_ws(text, idx);
        if text[idx..].starts_with('.') {
            let next = skip_ws(text, idx + 1);
            let Some(len) = ident_len(&text[next..]) else {
                return false;
            };
            idx = next + len;
            saw_suffix = true;
            continue;
        }
        if text[idx..].starts_with('(') {
            let Some(end) = skip_group(text, idx) else {
                return false;
            };
            idx = end;
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
    let (end, extended) = extend_chain(text, len)?;
    let bare = if extended { None } else { Some(name) };
    Some((end, bare))
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
        if text[next..].starts_with('(') {
            idx = skip_group(text, next)?;
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
        assert!(ELEMENTS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(is_element("heading"));
        assert!(is_element("raw"));
        assert!(is_element("text"));
        assert!(!is_element("notes"));
        assert!(!is_element("tonight"));
        assert!(!is_element("ieee"));
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

        let tilde = "\
~~~c
#let ieee(
~~~
See the sample. Next sentence.
";
        assert_text(tilde, Format::Plaintext);

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
}
