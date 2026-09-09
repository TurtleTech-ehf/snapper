//! Keep RST comments and snapper pragmas on the pandoc write path.
//!
//! Pandoc's reader deletes RST `..` comments and comment-wrapped
//! `snapper:off` / `snapper:on`. This pre-pass replaces those spans with
//! inert tokens, the AST walker turns token Paras into `RawBlock`s, and
//! the post-pass restores any token the writer left as text.

use pandoc_ast::{Block, Format, Inline, Pandoc};

/// Source with dropped spans replaced by tokens, plus the mapping back.
#[derive(Debug, Clone)]
pub(crate) struct Protected {
    pub source: String,
    pub parts: Vec<(String, String)>,
}

/// Lift RST comments and snapper off/on regions out of `input`.
pub(crate) fn protect_dropped_text(input: &str, format: &str) -> Protected {
    let lines: Vec<&str> = input.split_inclusive('\n').collect();
    let rst = is_rst_pandoc_format(format);
    let mut out = String::with_capacity(input.len());
    let mut parts = Vec::new();
    let mut i = 0usize;

    while i < lines.len() {
        let payload = line_payload(lines[i]);

        if crate::parser::check_pragma(payload) == Some(false) {
            let start = i;
            i += 1;
            while i < lines.len()
                && crate::parser::check_pragma(line_payload(lines[i])) != Some(true)
            {
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            emit_span(&lines[start..i], input, &mut out, &mut parts);
            continue;
        }

        if crate::parser::check_pragma(payload) == Some(true) {
            emit_span(&lines[i..i + 1], input, &mut out, &mut parts);
            i += 1;
            continue;
        }

        if rst && crate::parser::rst::is_rst_dropped_comment_opener(payload.trim_start()) {
            let start = i;
            let leading = payload.len() - payload.trim_start().len();
            let comment_indent = leading + 1;
            i += 1;
            while i < lines.len() {
                let body = line_payload(lines[i]);
                if body.trim().is_empty() || leading_ws(body) >= comment_indent {
                    i += 1;
                    continue;
                }
                break;
            }
            emit_span(&lines[start..i], input, &mut out, &mut parts);
            continue;
        }

        out.push_str(lines[i]);
        i += 1;
    }

    Protected { source: out, parts }
}

/// Put original spans back. Safe if a token never appeared in `output`.
pub(crate) fn restore_dropped_text(output: &str, parts: &[(String, String)]) -> String {
    if parts.is_empty() {
        return output.to_string();
    }
    let mut out = output.to_string();
    for (token, original) in parts {
        out = out.replace(token, original);
    }
    out
}

/// Replace token-only Para/Plain nodes with `RawBlock` so the writer
/// emits the original markup instead of a placeholder paragraph.
pub(crate) fn inject_raw_blocks(doc: &mut Pandoc, parts: &[(String, String)], format: &str) {
    if parts.is_empty() {
        return;
    }
    let raw_fmt = Format(base_pandoc_format(format).to_string());
    replace_keep_blocks(&mut doc.blocks, parts, &raw_fmt);
}

pub(crate) fn is_rst_pandoc_format(format: &str) -> bool {
    matches!(base_pandoc_format(format), "rst" | "rest")
}

fn base_pandoc_format(format: &str) -> &str {
    format.split(['+', '-']).next().unwrap_or(format)
}

fn emit_span(
    span_lines: &[&str],
    input: &str,
    out: &mut String,
    parts: &mut Vec<(String, String)>,
) {
    let original = span_lines.concat();
    let token = unique_token(parts.len(), input);
    out.push_str(&token);
    parts.push((token, original));
}

fn unique_token(index: usize, input: &str) -> String {
    let mut n = index;
    loop {
        let token = format!("SNAPPERKEEP{n}END");
        if !input.contains(&token) {
            return token;
        }
        n = n.saturating_add(1_000_003);
    }
}

fn line_payload(line: &str) -> &str {
    line.strip_suffix('\n').unwrap_or(line)
}

fn leading_ws(s: &str) -> usize {
    s.len() - s.trim_start().len()
}

fn para_keep_token(inlines: &[Inline]) -> Option<String> {
    let mut s = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(t) => s.push_str(t),
            Inline::Space | Inline::SoftBreak => {}
            _ => return None,
        }
    }
    let trimmed = s.trim();
    if trimmed.starts_with("SNAPPERKEEP") && trimmed.ends_with("END") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn replace_keep_blocks(blocks: &mut [Block], parts: &[(String, String)], raw_fmt: &Format) {
    for block in blocks.iter_mut() {
        match block {
            Block::Para(ins) | Block::Plain(ins) => {
                if let Some(tok) = para_keep_token(ins) {
                    if let Some((_, original)) = parts.iter().find(|(t, _)| *t == tok) {
                        *block = Block::RawBlock(raw_fmt.clone(), original.clone());
                    }
                }
            }
            Block::BlockQuote(inner) | Block::Div(_, inner) | Block::Figure(_, _, inner) => {
                replace_keep_blocks(inner, parts, raw_fmt);
            }
            Block::BulletList(items) | Block::OrderedList(_, items) => {
                for item in items {
                    replace_keep_blocks(item, parts, raw_fmt);
                }
            }
            Block::DefinitionList(defs) => {
                for (_term, definitions) in defs {
                    for def in definitions {
                        replace_keep_blocks(def, parts, raw_fmt);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_restore_rst_comment_fixture_is_identity() {
        let input = include_str!("../../../tests/fixtures/pandoc_ast/rst_comment.rst");
        let protected = protect_dropped_text(input, "rst");
        assert!(
            !protected.parts.is_empty(),
            "fixture must lift RST comments"
        );
        assert!(
            !protected.source.contains("This comment must not vanish."),
            "comment body must leave the pandoc input:\n{}",
            protected.source
        );
        assert!(
            !protected.source.contains("recognized comment"),
            "recognized comment must leave the pandoc input:\n{}",
            protected.source
        );
        let back = restore_dropped_text(&protected.source, &protected.parts);
        assert_eq!(back, input);
    }

    #[test]
    fn protect_restore_markdown_pragma_fixture_is_identity() {
        let input = include_str!("../../../tests/fixtures/pandoc_ast/snapper_off.md");
        let protected = protect_dropped_text(input, "markdown");
        assert!(!protected.parts.is_empty(), "fixture must lift snapper:off");
        assert!(
            !protected.source.contains("snapper:off"),
            "pragma must leave the pandoc input:\n{}",
            protected.source
        );
        assert!(
            !protected.source.contains("Keep this. Exactly here."),
            "off-region body must not be reflowed by pandoc:\n{}",
            protected.source
        );
        let back = restore_dropped_text(&protected.source, &protected.parts);
        assert_eq!(back, input);
    }

    #[test]
    fn protect_restore_rst_pragma_fixture_is_identity() {
        let input = include_str!("../../../tests/fixtures/pandoc_ast/snapper_off.rst");
        let protected = protect_dropped_text(input, "rst");
        assert!(!protected.parts.is_empty());
        assert!(!protected.source.contains("snapper:off"));
        let back = restore_dropped_text(&protected.source, &protected.parts);
        assert_eq!(back, input);
    }

    #[test]
    fn prose_only_is_unchanged() {
        let input = "Hello world. Second sentence.\n";
        let protected = protect_dropped_text(input, "rst");
        assert!(protected.parts.is_empty());
        assert_eq!(protected.source, input);
    }

    #[test]
    fn token_avoids_source_collision() {
        let input = "SNAPPERKEEP0END\n.. a comment.\n";
        let protected = protect_dropped_text(input, "rst");
        assert_eq!(protected.parts.len(), 1);
        assert_ne!(protected.parts[0].0, "SNAPPERKEEP0END");
        assert!(!input.contains(&protected.parts[0].0));
    }
}
