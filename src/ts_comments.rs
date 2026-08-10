//! Comment discovery through tree-sitter grammars.
//!
//! The scanner in [`crate::code_block`] recognises a comment only when the
//! marker opens the line. That rule is what keeps the `//` inside
//! `let s = "// not a comment.";` from being treated as prose, but it also
//! means a comment trailing real code is never reflowed. A grammar knows
//! which bytes are a comment and which are a string, so this pass rewrites
//! the comments it finds and leaves every other byte alone.
//!
//! Line comments keep the per-line marker shape. Block comments stay on the
//! scanner: languages that use `*/` end a comment at the first closer, even
//! when that closer sits inside a string, so a grammar span cannot reach the
//! real end. Python docstrings are string nodes, not comments, and also stay
//! on the scanner.

use std::collections::HashSet;

use tree_sitter::Parser;

use crate::config::CodeLang;
use crate::sentence::SentenceSplitter;

/// Resolve a fence language name to a grammar. Names follow the strings
/// users write on a fence, not the crate names.
fn language_for(lang: &str) -> Option<tree_sitter::Language> {
    let key = lang.trim().to_ascii_lowercase();
    let language = match key.as_str() {
        "rust" | "rs" => tree_sitter_rust::LANGUAGE.into(),
        "python" | "py" => tree_sitter_python::LANGUAGE.into(),
        "javascript" | "js" | "jsx" => tree_sitter_javascript::LANGUAGE.into(),
        "c" => tree_sitter_c::LANGUAGE.into(),
        "cpp" | "c++" | "cxx" | "cc" => tree_sitter_cpp::LANGUAGE.into(),
        "go" | "golang" => tree_sitter_go::LANGUAGE.into(),
        "bash" | "sh" | "shell" | "zsh" => tree_sitter_bash::LANGUAGE.into(),
        "html" => tree_sitter_html::LANGUAGE.into(),
        _ => return None,
    };
    Some(language)
}

/// Rewrite line comments in `body`, returning `None` when no grammar covers
/// the language or the source does not parse well enough to trust.
///
/// `frozen` carries zero-based line numbers the caller has ruled out, which
/// is how `snapper:off` regions and the pragma lines themselves survive.
pub(crate) fn reflow_grammar_comments(
    lang: &str,
    body: &str,
    cfg: &CodeLang,
    splitter: &dyn SentenceSplitter,
    frozen: &HashSet<usize>,
) -> Option<String> {
    let language = language_for(lang)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(body, None)?;

    let block_open = cfg
        .block_comment
        .as_ref()
        .map(|pair| pair[0].clone())
        .unwrap_or_default();

    // Collect comment spans first; splicing happens back-to-front so earlier
    // byte offsets stay valid.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut cursor = tree.walk();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.kind().contains("comment") {
            let text = &body[node.byte_range()];
            let is_block = node.kind().contains("block")
                || (!block_open.is_empty() && text.starts_with(&block_open));
            if !is_block && !frozen.contains(&node.start_position().row) {
                spans.push((node.start_byte(), node.end_byte()));
            }
            continue;
        }
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    spans.sort_unstable();

    let mut out = body.to_string();
    for (start, end) in spans.into_iter().rev() {
        let text = &body[start..end];
        // A comment the grammar reports as spanning lines is left alone; the
        // per-line shape below would not describe it.
        if text.contains('\n') {
            continue;
        }
        let Some(replacement) = rewrite_one(body, start, text, splitter) else {
            continue;
        };
        out.replace_range(start..end, &replacement);
    }
    Some(out)
}

/// Build the replacement text for a single-line comment at `start`.
fn rewrite_one(
    body: &str,
    start: usize,
    text: &str,
    splitter: &dyn SentenceSplitter,
) -> Option<String> {
    // The marker is the leading run of punctuation, so `///` and `//!` keep
    // their own shape instead of being cut back to `//`.
    let marker_len = text
        .find(|c: char| c.is_whitespace() || c.is_alphanumeric())
        .unwrap_or(text.len());
    if marker_len == 0 {
        return None;
    }
    let marker = &text[..marker_len];
    let prose = text[marker_len..].trim();
    if prose.is_empty() {
        return None;
    }

    let sentences = splitter.split(prose);
    if sentences.len() < 2 {
        return None;
    }

    // Continuation lines align under the comment, so a trailing comment
    // stays visually attached to the code it annotates. Tabs in the prefix
    // are preserved as tabs to keep that alignment under any tab width.
    let line_start = body[..start].rfind('\n').map_or(0, |i| i + 1);
    let pad: String = body[line_start..start]
        .chars()
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();

    let mut replacement = String::with_capacity(text.len() + sentences.len() * (pad.len() + 4));
    for (i, sentence) in sentences.iter().enumerate() {
        if i > 0 {
            replacement.push('\n');
            replacement.push_str(&pad);
        }
        replacement.push_str(marker);
        replacement.push(' ');
        replacement.push_str(sentence);
    }
    Some(replacement)
}
