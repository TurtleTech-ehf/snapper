//! Comment-aware reflow inside a `Region::Code`.
//!
//! The contract: take the raw `body` of a code block (between the fence
//! lines), reflow any prose carried inside comments per the language's
//! configured comment markers, and optionally pipe the result through an
//! external formatter. Lines that are not comments pass through unchanged
//! unless the formatter rewrites them.
//!
//! Indentation is preserved exactly. For a line that matches the
//! `line_comment` marker, the leading whitespace + marker + (optional one
//! space) are stripped, the remainder runs through the sentence splitter
//! with `Format::Plaintext` semantics, and each output sentence is re-emitted
//! with the original prefix.
//!
//! Block comments (`block_comment = ["open", "close"]`) treat the open and
//! close marker lines verbatim and reflow the prose between as a single
//! plaintext blob.
//!
//! The `// snapper:off` / `// snapper:on` pragma applies inside code blocks:
//! lines between the markers (inclusive of the pragma lines) emit verbatim.
//! The pragma matcher accepts the language's `line_comment` marker in
//! addition to the format-specific prefixes already recognised by
//! `parser::check_pragma`.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::config::CodeLang;
use crate::parser::check_pragma;
use crate::sentence::SentenceSplitter;

/// Wall-clock budget for the external formatter, in seconds.
pub const FORMATTER_TIMEOUT_SECS: u64 = 30;

/// Reflow the `body` of a `Region::Code`.
///
/// `cfg` carries the per-language marker configuration. `splitter` is the
/// active sentence splitter (used for comment prose). When `format_code`
/// is `true` and `cfg.formatter` is set, the post-comment-reflow body is
/// piped through that formatter; failures degrade gracefully by returning
/// the pre-formatter body and emitting a diagnostic on stderr.
pub fn reflow_code_body(
    lang: &str,
    body: &str,
    cfg: &CodeLang,
    splitter: &dyn SentenceSplitter,
    format_code: bool,
) -> String {
    // A grammar, where one exists, catches the comments the line-start rule
    // below cannot see. Its output feeds the scanner unchanged: re-splitting
    // a single sentence yields that sentence, so the second pass is a no-op
    // on anything the first already shaped.
    #[cfg(feature = "treesitter")]
    let body: &str = &{
        let frozen = frozen_lines(body, cfg);
        crate::ts_comments::reflow_grammar_comments(lang, body, cfg, splitter, &frozen)
            .unwrap_or_else(|| body.to_string())
    };
    #[cfg(not(feature = "treesitter"))]
    let _ = lang;

    let after_comment_reflow = reflow_comments(body, cfg, splitter);
    if format_code {
        if let Some(ref argv) = cfg.formatter {
            match run_formatter(&after_comment_reflow, argv) {
                Ok(out) => return out,
                Err(diag) => {
                    eprintln!("snapper: {diag}");
                    return after_comment_reflow;
                }
            }
        }
    }
    after_comment_reflow
}

/// Run the comment-reflow pass. Non-comment lines stay original slices;
/// only comment spans are rewritten.
fn reflow_comments(body: &str, cfg: &CodeLang, splitter: &dyn SentenceSplitter) -> String {
    let lines = crate::parser::iter_lines(body);
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    let mut pragma_off = false;

    while i < lines.len() {
        let line = lines[i];
        let slice = &body[line.start..line.end];

        if let Some(on) = check_pragma_for(line.text, cfg) {
            pragma_off = !on;
            out.push_str(slice);
            i += 1;
            continue;
        }
        if pragma_off {
            out.push_str(slice);
            i += 1;
            continue;
        }

        if let Some(ref pair) = cfg.block_comment {
            let [open, close] = [pair[0].as_str(), pair[1].as_str()];
            if !open.is_empty() {
                if let Some((indent, after_open)) = split_at_marker(line.text, open) {
                    let trimmed_after = after_open.trim_start();
                    if !close.is_empty() {
                        if let Some(idx) = find_close(trimmed_after, close, cfg) {
                            let interior = &trimmed_after[..idx];
                            emit_block_comment(&mut out, indent, open, close, interior, splitter);
                            i += 1;
                            continue;
                        }
                    }
                    let mut interior = after_open.to_string();
                    let mut close_indent: Option<String> = None;
                    let mut closed_at = None;
                    for (j, next) in lines.iter().enumerate().skip(i + 1) {
                        if let Some(idx) = find_close(next.text, close, cfg) {
                            let pre = &next.text[..idx];
                            let pre_trim = pre.trim();
                            if !pre_trim.is_empty() {
                                if !interior.is_empty() && !interior.ends_with(' ') {
                                    interior.push(' ');
                                }
                                interior.push_str(pre_trim);
                            }
                            close_indent = Some(
                                next.text[..next.text.len() - next.text.trim_start().len()]
                                    .to_string(),
                            );
                            closed_at = Some(j);
                            break;
                        }
                        let stripped = next.text.trim_start();
                        let stripped = stripped
                            .strip_prefix("* ")
                            .or_else(|| stripped.strip_prefix('*'))
                            .unwrap_or(stripped);
                        if !interior.is_empty() && !interior.ends_with(' ') {
                            interior.push(' ');
                        }
                        interior.push_str(stripped.trim());
                    }
                    if let Some(j) = closed_at {
                        let ci = close_indent.unwrap_or_else(|| indent.to_string());
                        emit_block_comment_multi(
                            &mut out,
                            indent,
                            open,
                            close,
                            &ci,
                            interior.trim(),
                            splitter,
                        );
                        i = j + 1;
                        continue;
                    }
                    // Unterminated: copy original slices through EOF.
                    for keep in &lines[i..] {
                        out.push_str(&body[keep.start..keep.end]);
                    }
                    break;
                }
            }
        }

        if let Some(ref marker) = cfg.line_comment {
            if let Some((indent, marker, rest)) = strip_line_comment(line.text, marker) {
                let prose = rest.trim();
                if prose.is_empty() {
                    out.push_str(slice);
                    i += 1;
                    continue;
                }
                let sentences = splitter.split(prose);
                if sentences.len() <= 1 {
                    out.push_str(slice);
                    i += 1;
                    continue;
                }
                for (k, s) in sentences.iter().enumerate() {
                    out.push_str(indent);
                    out.push_str(marker);
                    out.push(' ');
                    out.push_str(s);
                    if k + 1 < sentences.len() {
                        out.push('\n');
                    } else {
                        out.push_str(
                            &body[line.terminator_span().start..line.terminator_span().end],
                        );
                    }
                }
                i += 1;
                continue;
            }
        }

        if let Some(ref marker) = cfg.line_comment {
            if let Some(at) = trailing_comment_at(line.text, marker, cfg) {
                if let Some(rewritten) = rewrite_trailing(line.text, at, marker, splitter) {
                    out.push_str(&rewritten);
                    out.push_str(&body[line.terminator_span().start..line.terminator_span().end]);
                    i += 1;
                    continue;
                }
            }
        }

        out.push_str(slice);
        i += 1;
    }
    out
}

/// Zero-based line numbers no comment pass may rewrite: the `snapper:off`
/// and `snapper:on` pragma lines themselves, and everything between them.
#[cfg(feature = "treesitter")]
fn frozen_lines(body: &str, cfg: &CodeLang) -> std::collections::HashSet<usize> {
    let mut frozen = std::collections::HashSet::new();
    let mut off = false;
    for (i, line) in body.lines().enumerate() {
        if let Some(on) = check_pragma_for(line, cfg) {
            frozen.insert(i);
            off = !on;
            continue;
        }
        if off {
            frozen.insert(i);
        }
    }
    frozen
}

/// Recognise the snapper pragma carried inside a code-block comment.
/// Accepts the language's `line_comment` marker in addition to the
/// format-specific prefixes already recognised by `parser::check_pragma`.
fn check_pragma_for(line: &str, cfg: &CodeLang) -> Option<bool> {
    if let Some(b) = check_pragma(line) {
        return Some(b);
    }
    let trimmed = line.trim();
    if let Some(ref marker) = cfg.line_comment {
        if let Some(rest) = trimmed.strip_prefix(marker.as_str()) {
            let rest = rest.trim();
            if rest == "snapper:off" {
                return Some(false);
            }
            if rest == "snapper:on" {
                return Some(true);
            }
        }
    }
    None
}

/// Byte offset of a comment marker that follows code on the same line, or
/// `None` when the line has no such marker outside a string.
///
/// Quote tracking is what separates `x = 1; // note` from
/// `let s = "// not a comment";`. It is deliberately conservative: an
/// unbalanced quote (a Rust lifetime, an apostrophe in a shell word) leaves
/// the rest of the line looking quoted, so a marker after it is missed
/// rather than mistaken.
fn trailing_comment_at(line: &str, marker: &str, cfg: &CodeLang) -> Option<usize> {
    if marker.is_empty() {
        return None;
    }
    let quotes = cfg.quote_chars();
    let escape = cfg.escape_char();
    let block_open = cfg
        .block_comment
        .as_ref()
        .map(|pair| pair[0].as_str())
        .unwrap_or("");

    let bytes = line.as_bytes();
    let mut in_string: Option<char> = None;
    let mut i = 0;
    let mut seen_code = false;

    while i < bytes.len() {
        let rest = &line[i..];
        let ch = rest.chars().next()?;

        match in_string {
            Some(delim) => {
                if ch == escape {
                    i += ch.len_utf8();
                    if let Some(next) = line[i..].chars().next() {
                        i += next.len_utf8();
                    }
                    continue;
                }
                if ch == delim {
                    in_string = None;
                }
            }
            None => {
                if quotes.contains(&ch) {
                    in_string = Some(ch);
                    seen_code = true;
                } else if !block_open.is_empty() && rest.starts_with(block_open) {
                    // A block comment on a code line is the scanner's blind
                    // spot either way; leave the whole line alone.
                    return None;
                } else if rest.starts_with(marker) {
                    return if seen_code { Some(i) } else { None };
                } else if !ch.is_whitespace() {
                    seen_code = true;
                }
            }
        }
        i += ch.len_utf8();
    }
    None
}

/// Rewrite a line whose comment starts at `at`, keeping the first sentence
/// beside the code and aligning the rest under the marker. Returns `None`
/// when the comment holds a single sentence and the line stands as written.
fn rewrite_trailing(
    line: &str,
    at: usize,
    marker: &str,
    splitter: &dyn SentenceSplitter,
) -> Option<String> {
    let (code, comment) = line.split_at(at);
    let (_, found, rest) = strip_line_comment(comment, marker)?;
    let prose = rest.trim();
    if prose.is_empty() {
        return None;
    }
    let sentences = splitter.split(prose);
    if sentences.len() < 2 {
        return None;
    }

    let pad: String = code
        .chars()
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    let mut out = String::with_capacity(line.len() + sentences.len() * (pad.len() + 4));
    for (i, sentence) in sentences.iter().enumerate() {
        if i == 0 {
            out.push_str(code);
        } else {
            out.push('\n');
            out.push_str(&pad);
        }
        out.push_str(found);
        out.push(' ');
        out.push_str(sentence);
    }
    Some(out)
}

/// Byte offset of `close` on `line`, skipping matches that sit inside a
/// string. When `close` is itself a quote sequence (`"""`, `'''`), the first
/// match is the closer and quoting does not apply.
fn find_close(line: &str, close: &str, cfg: &CodeLang) -> Option<usize> {
    if close.is_empty() {
        return None;
    }
    let quotes = cfg.quote_chars();
    if close.chars().all(|c| quotes.contains(&c)) {
        return line.find(close);
    }
    let escape = cfg.escape_char();
    let bytes = line.as_bytes();
    let mut in_string: Option<char> = None;
    let mut i = 0;
    while i < bytes.len() {
        let rest = &line[i..];
        let ch = rest.chars().next()?;
        match in_string {
            Some(delim) => {
                if ch == escape {
                    i += ch.len_utf8();
                    if let Some(next) = line[i..].chars().next() {
                        i += next.len_utf8();
                    }
                    continue;
                }
                if ch == delim {
                    in_string = None;
                }
            }
            None => {
                if rest.starts_with(close) {
                    return Some(i);
                }
                if quotes.contains(&ch) {
                    in_string = Some(ch);
                }
            }
        }
        i += ch.len_utf8();
    }
    None
}

/// Split a line at the first occurrence of `marker`. Returns
/// `(indent, after_marker)` where `indent` is the leading whitespace
/// preserved verbatim. Returns `None` if `marker` is not the first
/// non-whitespace token.
fn split_at_marker<'a>(line: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    let leading = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(leading);
    rest.strip_prefix(marker).map(|after| (indent, after))
}

/// Strip a line-comment prefix from `line` if present. Returns
/// `(indent, marker_as_written, body_after_marker_and_one_optional_space)`.
///
/// The marker returned is the one on the page, not the one in the config: a
/// doc comment repeats or decorates the configured marker (`///`, `//!`,
/// `;;;`), and re-emitting such a line under the short form would push the
/// extra characters into the prose.
fn strip_line_comment<'a>(line: &'a str, marker: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let leading = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(leading);
    rest.strip_prefix(marker)?;

    let mut end = marker.len();
    if let Some(last) = marker.chars().last() {
        let bytes = rest.as_bytes();
        while end < bytes.len() && bytes[end] == last as u8 {
            end += 1;
        }
        // `//!` is a marker; `#!` opening the first line of a shell block is
        // a shebang, so only multi-character markers absorb the bang.
        if marker.len() > 1 && end < bytes.len() && bytes[end] == b'!' {
            end += 1;
        }
    }
    let (found, after) = rest.split_at(end);

    // Accept (but don't require) a single separating space; further leading
    // whitespace is preserved as part of the prose so quoted code blocks like
    // `//   code` round-trip.
    let after = after.strip_prefix(' ').unwrap_or(after);
    Some((indent, found, after))
}

/// Emit a same-line `/* ... */`-style comment as three lines:
/// `indent + open\n + indent + " " + sentence\n... + indent + close\n`.
/// Interior reflows as plaintext via the sentence splitter.
fn emit_block_comment(
    out: &mut String,
    indent: &str,
    open: &str,
    close: &str,
    interior: &str,
    splitter: &dyn SentenceSplitter,
) {
    out.push_str(indent);
    out.push_str(open);
    out.push('\n');
    let sentences = splitter.split(interior.trim());
    for s in &sentences {
        out.push_str(indent);
        out.push(' ');
        out.push_str(s);
        out.push('\n');
    }
    out.push_str(indent);
    out.push_str(close);
    out.push('\n');
}

/// Emit a multi-line block comment: open marker stays on its original line,
/// interior reflows, close marker on its own line at `close_indent`.
fn emit_block_comment_multi(
    out: &mut String,
    indent: &str,
    open: &str,
    close: &str,
    close_indent: &str,
    interior: &str,
    splitter: &dyn SentenceSplitter,
) {
    out.push_str(indent);
    out.push_str(open);
    out.push('\n');
    let sentences = splitter.split(interior);
    for s in &sentences {
        out.push_str(indent);
        out.push(' ');
        out.push_str(s);
        out.push('\n');
    }
    out.push_str(close_indent);
    out.push_str(close);
    out.push('\n');
}

/// Pipe `body` through the formatter `argv` via stdin/stdout.
/// Returns the formatter's stdout on success. Returns `Err(message)` on
/// any failure mode (binary missing, non-zero exit, timeout, I/O); the
/// caller is expected to log the message and fall back to the input.
///
/// The wait is implemented with a watchdog thread that calls `Child::kill`
/// after `FORMATTER_TIMEOUT_SECS`. On the happy path the watchdog is
/// signalled to exit via an mpsc channel and joins immediately.
pub fn run_formatter(body: &str, argv: &[String]) -> Result<String, String> {
    if argv.is_empty() {
        return Err("formatter argv is empty".to_string());
    }
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Err(format!("formatter not found: {}", argv[0]));
            }
            return Err(format!("formatter spawn failed: {}: {e}", argv[0]));
        }
    };

    // Write stdin in a worker thread so the parent can poll for timeout.
    if let Some(mut stdin) = child.stdin.take() {
        let body_owned = body.to_string();
        let _ = thread::spawn(move || {
            let _ = stdin.write_all(body_owned.as_bytes());
            // stdin drops at end of scope, signalling EOF to the child.
        });
    }

    // Watchdog: kill the child after FORMATTER_TIMEOUT_SECS unless told to stop.
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let child_id = child.id();
    let watchdog = thread::spawn(move || {
        match done_rx.recv_timeout(Duration::from_secs(FORMATTER_TIMEOUT_SECS)) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Normal completion path; nothing to do.
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Best-effort SIGKILL by PID. On unix this is a kill(2);
                // we avoid pulling in nix and rely on the platform tool.
                #[cfg(unix)]
                unsafe {
                    libc_kill(child_id as i32);
                }
                #[cfg(not(unix))]
                {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/PID", &child_id.to_string()])
                        .output();
                }
            }
        }
    });

    // Wait for the child. On timeout the watchdog SIGKILLs and `wait`
    // returns with a non-zero status.
    let output = child.wait_with_output();
    // Signal the watchdog regardless of outcome so it joins promptly.
    let _ = done_tx.send(());
    let _ = watchdog.join();

    let output = match output {
        Ok(o) => o,
        Err(e) => return Err(format!("formatter wait failed: {}: {e}", argv[0])),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "formatter {} exited non-zero (status {:?}): {}",
            argv[0],
            output.status.code(),
            stderr.trim()
        ));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| format!("formatter {} produced non-UTF-8 output: {e}", argv[0]))
}

/// SIGKILL via libc. The cross-platform stdlib has no `kill_by_pid`, but
/// `libc::kill` is stable. We declare the extern manually to avoid a new
/// always-on dependency.
#[cfg(unix)]
unsafe fn libc_kill(pid: i32) {
    // `extern "C"` declarations are unsafe-by-association; we wrap the call.
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGKILL: i32 = 9;
    unsafe {
        let _ = kill(pid, SIGKILL);
    }
}

/// Read helper used in tests to capture formatter stdout. Exposed here so
/// the integration tests can share the pattern without re-deriving it.
#[doc(hidden)]
pub fn read_to_string(mut r: impl Read) -> std::io::Result<String> {
    let mut s = String::new();
    r.read_to_string(&mut s)?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::unicode::UnicodeSentenceSplitter;

    fn rust_cfg() -> CodeLang {
        CodeLang {
            line_comment: Some("//".to_string()),
            block_comment: Some(["/*".to_string(), "*/".to_string()]),
            ..Default::default()
        }
    }

    #[test]
    fn line_comment_two_sentences_split() {
        let body = "// First sentence. Second sentence.\nfn main() {}\n";
        let out = reflow_comments(body, &rust_cfg(), &UnicodeSentenceSplitter::new());
        assert_eq!(
            out,
            "// First sentence.\n// Second sentence.\nfn main() {}\n"
        );
    }

    #[test]
    fn indented_comment_preserved() {
        let body = "    // First. Second.\n    fn x() {}\n";
        let out = reflow_comments(body, &rust_cfg(), &UnicodeSentenceSplitter::new());
        assert_eq!(out, "    // First.\n    // Second.\n    fn x() {}\n");
    }

    #[test]
    fn non_comment_passes_through() {
        let body = "fn main() { println!(\"hi\"); }\n";
        let out = reflow_comments(body, &rust_cfg(), &UnicodeSentenceSplitter::new());
        assert_eq!(out, body);
    }

    #[test]
    fn block_comment_one_liner_splits() {
        let body = "/* First. Second. */\n";
        let out = reflow_comments(body, &rust_cfg(), &UnicodeSentenceSplitter::new());
        assert_eq!(out, "/*\n First.\n Second.\n*/\n");
    }

    #[test]
    fn pragma_freezes_run() {
        let body = "// snapper:off\n// Long.\n// Off.\n// snapper:on\n// Reflow this. Now.\n";
        let out = reflow_comments(body, &rust_cfg(), &UnicodeSentenceSplitter::new());
        let expected = concat!(
            "// snapper:off\n",
            "// Long.\n",
            "// Off.\n",
            "// snapper:on\n",
            "// Reflow this.\n",
            "// Now.\n",
        );
        assert_eq!(out, expected);
    }
}
