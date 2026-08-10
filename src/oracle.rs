//! Format-local render oracle used as a fail-closed backstop.
//!
//! After a fixpoint, `format_text` compares input and output under a
//! format-specific check. A mismatch returns the original document.
//! Tests disable the backstop and assert the oracle themselves.

use crate::format::Format;
use crate::parser::Region;

/// True when `output` is a render-safe reflow of `original`.
pub fn matches(format: Format, original: &str, output: &str) -> bool {
    if original == output {
        return true;
    }
    match format {
        Format::Markdown => md_html_normalized(original) == md_html_normalized(output),
        Format::Org | Format::Latex | Format::Rst | Format::Plaintext => {
            prose_words(format, original) == prose_words(format, output)
        }
    }
}

fn md_html_normalized(src: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(src, opts);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    // Fenced/indented code may have comment reflow (`// a. b.` → two
    // `//` lines). That is an intentional rewrite; the oracle still
    // guards prose render around the blocks.
    let without_pre = strip_pre(&html_output);
    normalize_ws(&without_pre)
}

fn strip_pre(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<pre") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(end) = after.find("</pre>") {
            out.push_str("<pre></pre>");
            rest = &after[end + 6..];
        } else {
            out.push_str(after);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Word sequence of prose regions only (structure/code/blank ignored).
pub fn prose_words(format: Format, text: &str) -> Vec<String> {
    let regions = crate::parser::parser_for_format(format).parse(text);
    let mut words = Vec::new();
    for r in regions {
        if let Region::Prose(p) = r {
            words.extend(p.split_whitespace().map(str::to_string));
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md_html_ignores_soft_breaks() {
        let a = "Hello world. Next sentence.";
        let b = "Hello world.\nNext sentence.";
        assert_eq!(md_html_normalized(a), md_html_normalized(b));
    }

    #[test]
    fn prose_words_stable_across_reflow() {
        let a = "Hello world. Next sentence.";
        let b = "Hello world.\nNext sentence.\n";
        assert_eq!(
            prose_words(Format::Plaintext, a),
            prose_words(Format::Plaintext, b)
        );
    }
}
