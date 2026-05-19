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
    body: &str,
    cfg: &CodeLang,
    splitter: &dyn SentenceSplitter,
    format_code: bool,
) -> String {
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

/// Run the comment-reflow pass. Pure function; no I/O.
fn reflow_comments(body: &str, cfg: &CodeLang, splitter: &dyn SentenceSplitter) -> String {
    let mut out = String::with_capacity(body.len());
    let mut iter = body.lines().peekable();
    let mut pragma_off = false;
    // Track whether the original body ended with a trailing newline so we
    // can reproduce it byte-identically.
    let trailing_newline = body.ends_with('\n');

    while let Some(line) = iter.next() {
        // Pragma check first; lines between off/on are verbatim.
        if let Some(on) = check_pragma_for(line, cfg) {
            pragma_off = !on;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if pragma_off {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Try block-comment open. If matched, accumulate to the close marker
        // and reflow the interior as plaintext.
        if let Some(ref pair) = cfg.block_comment {
            let [open, close] = [pair[0].as_str(), pair[1].as_str()];
            if !open.is_empty() {
                if let Some((indent, after_open)) = split_at_marker(line, open) {
                    // Same-line open + close (e.g. `/* one sentence. */`)?
                    let trimmed_after = after_open.trim_start();
                    if !close.is_empty() {
                        if let Some(idx) = trimmed_after.find(close) {
                            let interior = &trimmed_after[..idx];
                            // Emit: indent + open\n + reflowed interior\n + indent + close\n
                            emit_block_comment(
                                &mut out,
                                indent,
                                open,
                                close,
                                interior,
                                splitter,
                            );
                            continue;
                        }
                    }
                    // Multi-line block comment: gather body until close marker.
                    let mut interior = after_open.to_string();
                    let mut close_indent: Option<String> = None;
                    let mut closed = false;
                    while let Some(next) = iter.next() {
                        if let Some(idx) = next.find(close) {
                            // Found close. Anything before it (on this line)
                            // joins the interior; the close marker stays on
                            // its own emitted line at its original indent.
                            let pre = &next[..idx];
                            let pre_trim = pre.trim();
                            if !pre_trim.is_empty() {
                                if !interior.is_empty() && !interior.ends_with(' ') {
                                    interior.push(' ');
                                }
                                interior.push_str(pre_trim);
                            }
                            close_indent = Some(
                                next[..next.len() - next.trim_start().len()].to_string(),
                            );
                            closed = true;
                            break;
                        }
                        let stripped = next.trim_start();
                        // Strip a leading `*` decoration commonly used in
                        // C/Java/JS doc comments, plus one optional space.
                        let stripped = stripped
                            .strip_prefix("* ")
                            .or_else(|| stripped.strip_prefix('*'))
                            .unwrap_or(stripped);
                        if !interior.is_empty() && !interior.ends_with(' ') {
                            interior.push(' ');
                        }
                        interior.push_str(stripped.trim());
                    }
                    if closed {
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
                        continue;
                    }
                    // Unterminated block comment: emit interior we accumulated
                    // and bail out (best-effort; keep input shape).
                    out.push_str(line);
                    out.push('\n');
                    if !interior.is_empty() {
                        out.push_str(interior.trim_end());
                        out.push('\n');
                    }
                    continue;
                }
            }
        }

        // Line-comment reflow.
        if let Some(ref marker) = cfg.line_comment {
            if let Some((indent, rest)) = strip_line_comment(line, marker) {
                let prose = rest.trim();
                if prose.is_empty() {
                    out.push_str(line);
                    out.push('\n');
                    continue;
                }
                // If this comment line is the pragma itself we already handled
                // it above. Reflow as plaintext.
                let sentences = splitter.split(prose);
                if sentences.is_empty() {
                    out.push_str(line);
                    out.push('\n');
                    continue;
                }
                for s in &sentences {
                    out.push_str(indent);
                    out.push_str(marker);
                    out.push(' ');
                    out.push_str(s);
                    out.push('\n');
                }
                continue;
            }
        }

        // Non-comment line: passthrough.
        out.push_str(line);
        out.push('\n');
    }

    // Reproduce trailing-newline shape of the input.
    if !trailing_newline {
        if out.ends_with('\n') {
            out.pop();
        }
    }
    out
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
/// `(indent, body_after_marker_and_one_optional_space)`.
fn strip_line_comment<'a>(line: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    let leading = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(leading);
    let after = rest.strip_prefix(marker)?;
    // Accept (but don't require) a single separating space; further leading
    // whitespace is preserved as part of the prose so quoted code blocks like
    // `//   code` round-trip.
    let after = after.strip_prefix(' ').unwrap_or(after);
    Some((indent, after))
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
            formatter: None,
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
