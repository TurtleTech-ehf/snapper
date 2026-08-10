//! Format-local render oracle used as a fail-closed backstop.
//!
//! After a fixpoint, `format_text` compares input and output under a
//! format-specific check. A mismatch returns the original document.
//! Tests disable the backstop and assert the oracle themselves.
//!
//! The oracle is a native region-kind + slice tree for every format:
//! Structure/Blank must be byte-identical, Code headers/footers must be
//! identical, Code bodies may change only in comment lines, and Prose may
//! only move inter-word whitespace. Markdown also compares pulldown-cmark
//! HTML (including `<pre>` contents unless the only delta is a comment
//! reflow already allowed by the code-byte check).

use crate::format::Format;
use crate::parser::{Region, parser_for_format};

/// True when `output` is a render-safe reflow of `original`.
pub fn matches(format: Format, original: &str, output: &str) -> bool {
    matches_ex(format, original, output, false)
}

/// `allow_code_body_rewrite` is set when an opt-in external formatter may
/// replace a code body. Comment-only reflow does not need it.
pub fn matches_ex(
    format: Format,
    original: &str,
    output: &str,
    allow_code_body_rewrite: bool,
) -> bool {
    if original == output {
        return true;
    }
    if !structure_tree_ok(format, original, output, allow_code_body_rewrite) {
        return false;
    }
    if format == Format::Markdown {
        return md_html_ok(original, output);
    }
    true
}

fn structure_tree_ok(
    format: Format,
    original: &str,
    output: &str,
    allow_code_body_rewrite: bool,
) -> bool {
    // Hang list/quote continuations (`> One.` / `> Two.`) reparse as more
    // regions than the source. Coalesce adjacent same-marker items so the
    // tree compares as one item with the same prose words.
    let a = coalesce_hang_items(&parser_for_format(format).parse(original));
    let b = coalesce_hang_items(&parser_for_format(format).parse(output));
    if a.len() != b.len() {
        return false;
    }
    for (ra, rb) in a.iter().zip(&b) {
        match (ra, rb) {
            (Region::Prose(x), Region::Prose(y)) => {
                if words(x) != words(y) {
                    return false;
                }
            }
            (Region::Structure(x), Region::Structure(y))
            | (Region::BlankLines(x), Region::BlankLines(y)) => {
                if x != y {
                    return false;
                }
            }
            (
                Region::Code {
                    lang: la,
                    header: ha,
                    body: ba,
                    footer: fa,
                },
                Region::Code {
                    lang: lb,
                    header: hb,
                    body: bb,
                    footer: fb,
                },
            ) => {
                if la != lb || ha != hb || fa != fb {
                    return false;
                }
                if !allow_code_body_rewrite && !code_body_ok(ba, bb) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn words(s: &str) -> Vec<&str> {
    s.split_whitespace().collect()
}

/// Join adjacent list/quote items that share a marker.
///
/// `> One.\n> Two.` parses as two Structure+Prose pairs; hanging indent
/// emitted that from one source item. Folding them keeps the oracle from
/// vetoing a render-preserving hang.
fn coalesce_hang_items(regions: &[Region]) -> Vec<Region> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < regions.len() {
        let Region::Structure(marker) = &regions[i] else {
            out.push(regions[i].clone());
            i += 1;
            continue;
        };
        if !crate::reflow::is_hanging_marker(marker) {
            out.push(regions[i].clone());
            i += 1;
            continue;
        }
        let marker = marker.clone();
        let mut prose = String::new();
        i += 1;
        loop {
            match regions.get(i) {
                Some(Region::Prose(p)) => {
                    if !prose.is_empty() {
                        prose.push(' ');
                    }
                    prose.push_str(p);
                    i += 1;
                }
                Some(Region::Structure(nl)) if nl == "\n" => {
                    let same_marker = matches!(
                        regions.get(i + 1),
                        Some(Region::Structure(m2)) if *m2 == marker
                    );
                    if same_marker {
                        i += 2;
                        continue;
                    }
                    out.push(Region::Structure(marker));
                    if !prose.is_empty() {
                        out.push(Region::Prose(prose));
                    }
                    out.push(Region::Structure(nl.clone()));
                    i += 1;
                    break;
                }
                _ => {
                    out.push(Region::Structure(marker));
                    if !prose.is_empty() {
                        out.push(Region::Prose(prose));
                    }
                    break;
                }
            }
        }
    }
    out
}

/// Code bodies stay slices except rewritten comment lines.
fn code_body_ok(original: &str, output: &str) -> bool {
    if original == output {
        return true;
    }
    non_comment_lines(original) == non_comment_lines(output)
}

fn non_comment_lines(body: &str) -> Vec<&str> {
    body.lines().filter(|l| !looks_like_comment(l)).collect()
}

fn looks_like_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//")
        || t.starts_with('#')
        || t.starts_with("--")
        || t.starts_with(';')
        || t.starts_with("/*")
        || t.starts_with('*')
        || t.starts_with("*/")
}

fn md_html_ok(original: &str, output: &str) -> bool {
    let ha = md_html(original);
    let hb = md_html(output);
    if normalize_ws(&ha) == normalize_ws(&hb) {
        return true;
    }
    // Full HTML (including `<pre>`) differed. Allow only when the
    // non-pre document matches and the code-byte check already passed
    // via the structure tree.
    normalize_ws(&html_without_pre_inner(&ha)) == normalize_ws(&html_without_pre_inner(&hb))
}

fn md_html(src: &str) -> String {
    use pulldown_cmark::{Options, Parser, html};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(src, opts);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Keep `<pre></pre>` tags so a missing/extra fence still fails, but drop
/// inner text that the code-byte check already judged.
fn html_without_pre_inner(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<pre") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(gt) = after.find('>') {
            out.push_str(&after[..=gt]);
            let inner = &after[gt + 1..];
            if let Some(end) = inner.find("</pre>") {
                out.push_str("</pre>");
                rest = &inner[end + 6..];
                continue;
            }
        }
        out.push_str(after);
        rest = "";
        break;
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

/// Word sequence of prose regions only.
pub fn prose_words(format: Format, text: &str) -> Vec<String> {
    let regions = parser_for_format(format).parse(text);
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
        assert!(matches(Format::Markdown, a, b));
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

    #[test]
    fn structure_tree_sees_invented_newline() {
        assert!(!matches(Format::Markdown, "## Title", "## Title\n"));
        assert!(!matches(Format::Org, "* TODO a", "* TODO a\n"));
    }

    #[test]
    fn hung_quote_matches_source_item() {
        assert!(matches(
            Format::Markdown,
            "> One. Two.\n",
            "> One.\n> Two.\n"
        ));
        assert!(matches(
            Format::Markdown,
            "> Quoted one. Quoted two.\n> > Nested one. Nested two.\n",
            "> Quoted one.\n> Quoted two.\n> > Nested one.\n> > Nested two.\n"
        ));
    }

    #[test]
    fn hung_list_matches_source_item() {
        assert!(matches(
            Format::Markdown,
            "- One. Two.\n",
            "- One.\n  Two.\n"
        ));
        assert!(matches(Format::Org, "- One. Two.\n", "- One.\n  Two.\n"));
    }
}
