//! Keep RST comments and snapper pragmas through pandoc write-through.
//!
//! Pandoc's readers delete RST `..` comments and comment-shaped
//! `snapper:off` / `snapper:on` (org `#`, latex `%`, html `<!-- -->`).
//! Replace each span with a unique sentinel paragraph, write, then put
//! the original text back. Off-region bodies travel inside the sentinel
//! so they are not reflowed.

use std::ops::Range;

use crate::parser::iter_lines;

const SENTINEL_PREFIX: &str = "SNAPPERKEEP";

/// Shielded source plus the map used to restore original text.
#[derive(Debug, Clone)]
pub(crate) struct Shield {
    pub source: String,
    tokens: Vec<(String, String)>,
}

impl Shield {
    /// Identity when the source has nothing the reader would drop.
    pub(crate) fn apply(input: &str, format: &str) -> Self {
        let spans = collect_spans(input, format);
        if spans.is_empty() {
            return Self {
                source: input.to_string(),
                tokens: Vec::new(),
            };
        }
        let mut source = input.to_string();
        let mut tokens = Vec::with_capacity(spans.len());
        for (i, span) in spans.into_iter().enumerate().rev() {
            let raw = &input[span.start..span.end];
            let (orig, had_nl) = strip_one_trailing_newline(raw);
            let sentinel = unique_sentinel(input, i, &tokens);
            let repl = if had_nl {
                format!("{sentinel}\n")
            } else {
                sentinel.clone()
            };
            source.replace_range(span.start..span.end, &repl);
            tokens.push((sentinel, orig.to_string()));
        }
        tokens.reverse();
        Self { source, tokens }
    }

    pub(crate) fn restore(&self, written: &str) -> String {
        let mut out = written.to_string();
        for (sentinel, original) in &self.tokens {
            out = out.replace(sentinel, original);
        }
        out
    }
}

pub(crate) fn is_rst_pandoc_format(format: &str) -> bool {
    let base = format.split(['+', '-']).next().unwrap_or(format);
    matches!(base, "rst" | "rest")
}

fn strip_one_trailing_newline(s: &str) -> (&str, bool) {
    if let Some(stripped) = s.strip_suffix("\r\n") {
        (stripped, true)
    } else if let Some(stripped) = s.strip_suffix('\n') {
        (stripped, true)
    } else {
        (s, false)
    }
}

fn unique_sentinel(input: &str, index: usize, existing: &[(String, String)]) -> String {
    for nonce in 0u32.. {
        let candidate = format!("{SENTINEL_PREFIX}{index:04}N{nonce:08x}");
        if input.contains(&candidate) {
            continue;
        }
        if existing.iter().any(|(s, _)| s == &candidate) {
            continue;
        }
        return candidate;
    }
    unreachable!("u32 nonce space exhausted");
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

fn collect_spans(input: &str, format: &str) -> Vec<Range<usize>> {
    let mut spans = pragma_spans(input);
    if is_rst_pandoc_format(format) {
        for span in rst_comment_spans(input) {
            if !spans.iter().any(|p| overlaps(p, &span)) {
                spans.push(span);
            }
        }
    }
    spans.sort_by_key(|s| s.start);
    spans
}

/// File-level `snapper:off` … `snapper:on` (or off-to-EOF). Skips fenced
/// / verbatim / org-src bodies so in-code markers stay in the block.
fn pragma_spans(input: &str) -> Vec<Range<usize>> {
    let lines = iter_lines(input);
    let mut spans = Vec::new();
    let mut off_start: Option<usize> = None;
    let mut in_code = false;
    let mut fence_mark: Option<String> = None;
    let mut rst_code_indent: Option<usize> = None;

    for line in &lines {
        if toggle_code(
            line.text,
            &mut in_code,
            &mut fence_mark,
            &mut rst_code_indent,
        ) {
            continue;
        }
        if in_code {
            continue;
        }
        match crate::parser::check_pragma(line.text) {
            Some(false) => {
                if off_start.is_none() {
                    off_start = Some(line.start);
                }
            }
            Some(true) => {
                let start = off_start.take().unwrap_or(line.start);
                spans.push(start..line.end);
            }
            None => {}
        }
    }
    if let Some(start) = off_start {
        spans.push(start..input.len());
    }
    spans
}

fn rst_comment_spans(input: &str) -> Vec<Range<usize>> {
    let lines = iter_lines(input);
    let mut spans = Vec::new();
    let mut i = 0;
    let mut in_code = false;
    let mut fence_mark: Option<String> = None;
    let mut rst_code_indent: Option<usize> = None;

    while i < lines.len() {
        let line = lines[i];
        if toggle_code(
            line.text,
            &mut in_code,
            &mut fence_mark,
            &mut rst_code_indent,
        ) {
            i += 1;
            continue;
        }
        if in_code {
            i += 1;
            continue;
        }
        let trimmed = line.text.trim_start();
        if crate::parser::rst::is_rst_dropped_comment_opener(trimmed) {
            let leading = line.text.len() - trimmed.len();
            let comment_indent = leading + 1;
            let mut j = i + 1;
            while j < lines.len() {
                let body = lines[j].text;
                let lead = body.len() - body.trim_start().len();
                if body.trim().is_empty() || lead >= comment_indent {
                    j += 1;
                    continue;
                }
                break;
            }
            let end = lines[j.saturating_sub(1)].end;
            spans.push(line.start..end);
            i = j;
            continue;
        }
        i += 1;
    }
    spans
}

/// Returns true when this line opened or closed a code/verbatim region.
fn toggle_code(
    text: &str,
    in_code: &mut bool,
    fence_mark: &mut Option<String>,
    rst_code_indent: &mut Option<usize>,
) -> bool {
    let trimmed = text.trim_start();
    let leading = text.len() - trimmed.len();

    if let Some(indent) = *rst_code_indent {
        if text.trim().is_empty() || leading >= indent {
            return false;
        }
        *rst_code_indent = None;
        *in_code = false;
        // Fall through: this line may be a new opener or a comment.
    }

    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        let mark = if trimmed.starts_with("```") {
            "```"
        } else {
            "~~~"
        };
        if !*in_code {
            *in_code = true;
            *fence_mark = Some(mark.to_string());
            return true;
        }
        if fence_mark.as_deref() == Some(mark) {
            *in_code = false;
            *fence_mark = None;
            return true;
        }
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if !*in_code {
        if lower.starts_with("#+begin_src")
            || lower.starts_with("#+begin_example")
            || lower.starts_with("#+begin_export")
            || lower.starts_with("\\begin{verbatim}")
            || lower.starts_with("\\begin{lstlisting}")
            || lower.starts_with("\\begin{minted}")
        {
            *in_code = true;
            *fence_mark = Some(lower.chars().take(24).collect());
            return true;
        }
        if lower.starts_with(".. code-block::")
            || lower.starts_with(".. sourcecode::")
            || lower.starts_with(".. code::")
        {
            *in_code = true;
            *rst_code_indent = Some(leading + 3);
            return true;
        }
        return false;
    }

    if lower.starts_with("#+end_src")
        || lower.starts_with("#+end_example")
        || lower.starts_with("#+end_export")
        || lower.starts_with("\\end{verbatim}")
        || lower.starts_with("\\end{lstlisting}")
        || lower.starts_with("\\end{minted}")
    {
        *in_code = false;
        *fence_mark = None;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_when_nothing_to_shield() {
        let s = Shield::apply("Hello world.\n", "rst");
        assert_eq!(s.source, "Hello world.\n");
        assert!(s.tokens.is_empty());
        assert_eq!(s.restore("Hello world.\n"), "Hello world.\n");
    }

    #[test]
    fn rst_comment_span_covers_bare_dotdot_body() {
        let input = "Hello.\n\n..\n   Secret.\n\nBye.\n";
        let spans = rst_comment_spans(input);
        assert_eq!(spans.len(), 1);
        assert!(input[spans[0].clone()].contains(".."));
        assert!(input[spans[0].clone()].contains("Secret."));
        assert!(!input[spans[0].clone()].contains("Bye."));
    }

    #[test]
    fn rst_recognized_comment_is_a_span() {
        let input = ".. This is a comment.\n";
        let spans = rst_comment_spans(input);
        assert_eq!(spans.len(), 1);
        assert!(input[spans[0].clone()].contains("This is a comment."));
    }

    #[test]
    fn rst_targets_and_directives_are_not_comment_spans() {
        assert!(rst_comment_spans(".. _label:\n").is_empty());
        assert!(rst_comment_spans(".. note::\n   Body.\n").is_empty());
        assert!(rst_comment_spans(".. [1] cite\n").is_empty());
    }

    #[test]
    fn pragma_span_covers_off_region() {
        let input =
            "Hello.\n<!-- snapper:off -->\nKeep this. Exactly here.\n<!-- snapper:on -->\nAfter.\n";
        let spans = pragma_spans(input);
        assert_eq!(spans.len(), 1);
        let chunk = &input[spans[0].clone()];
        assert!(chunk.contains("<!-- snapper:off -->"));
        assert!(chunk.contains("Keep this. Exactly here."));
        assert!(chunk.contains("<!-- snapper:on -->"));
        assert!(!chunk.contains("After."));
    }

    #[test]
    fn pragma_inside_markdown_fence_is_ignored() {
        let input = "```\n<!-- snapper:off -->\ncode\n<!-- snapper:on -->\n```\n";
        assert!(pragma_spans(input).is_empty());
    }

    #[test]
    fn shield_restore_roundtrip_rst_comments() {
        let input =
            "Hello world. Second.\n\n..\n   This comment must not vanish.\n\n.. recognized.\n";
        let s = Shield::apply(input, "rst");
        assert!(s.source.contains(SENTINEL_PREFIX));
        assert!(!s.source.contains("This comment must not vanish."));
        assert_eq!(s.restore(&s.source), input);
    }

    #[test]
    fn shield_restore_roundtrip_markdown_pragma() {
        let input =
            "Hello.\n<!-- snapper:off -->\nKeep this. Exactly here.\n<!-- snapper:on -->\nAfter.\n";
        let s = Shield::apply(input, "markdown");
        assert!(s.source.contains(SENTINEL_PREFIX));
        assert!(!s.source.contains("snapper:off"));
        assert_eq!(s.restore(&s.source), input);
    }

    #[test]
    fn restore_from_written_paragraph() {
        let input = ".. Secret payload.\n";
        let s = Shield::apply(input, "rst");
        let written = format!("Intro.\n{}\nOutro.\n", s.tokens[0].0);
        let out = s.restore(&written);
        assert!(out.contains(".. Secret payload."));
        assert!(!out.contains(SENTINEL_PREFIX));
    }
}
