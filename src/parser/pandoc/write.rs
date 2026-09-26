//! Write a pandoc JSON AST through an in-process writer when the loaded
//! `libsnapper_pandoc` exports one, otherwise through the installed pandoc CLI.
//!
//! Today's library is reader-only. Reconstruction of source bytes is
//! impossible (no offsets) and is not attempted: `pandoc -f json -t <format>
//! --wrap=preserve` owns emission unless an FFI writer is present. SoftBreaks
//! inserted by AST reflow become sentence line breaks.
//!
//! A missing CLI writer is an explicit PATH error — never a silent spawn.

use std::io::Write;
use std::process::{Command, Stdio};

use super::cli::CliError;

/// Help/error contract: write still needs `pandoc` on PATH when the FFI
/// library has no writer. Parse may already have used in-process FFI.
pub const WRITER_NEEDS_PATH: &str =
    "writer still needs pandoc on PATH (libsnapper_pandoc is reader-only)";

/// Render `json` (a pandoc AST) as `format`.
///
/// Prefers [`super::ffi::write_via_ffi`] when the loaded library exports
/// `snapper_pandoc_write`. Otherwise requires the `pandoc` CLI and names
/// that requirement on failure.
pub fn write_ast(json: &str, format: &str) -> Result<String, CliError> {
    if super::ffi::ffi_write_available() {
        return super::ffi::write_via_ffi(json, format)
            .map_err(|e| CliError::Spawn(format!("{WRITER_NEEDS_PATH}: {e}")));
    }
    write_via_cli(json, format)
}

/// Render `json` (a pandoc AST) as `format` via the CLI writer.
pub fn write_via_cli(json: &str, format: &str) -> Result<String, CliError> {
    let mut child = Command::new("pandoc")
        .args(["-f", "json", "-t", format, "--wrap=preserve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| CliError::Spawn(format!("{WRITER_NEEDS_PATH}: {e}")))?;

    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(json.as_bytes())
            .map_err(|e| CliError::Spawn(format!("write stdin: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| CliError::Spawn(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Exit(format!(
            "writer status {}: {stderr}",
            output.status
        )));
    }

    String::from_utf8(output.stdout)
        .map_err(|e| CliError::InvalidAst(format!("writer stdout not UTF-8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::pandoc::cli::pandoc_cli_available;

    #[test]
    fn writer_empty_json_is_explicit_error_or_empty() {
        if !pandoc_cli_available() {
            return;
        }
        let err = write_via_cli("not-json", "markdown").unwrap_err();
        match err {
            CliError::Exit(_) | CliError::Spawn(_) | CliError::InvalidAst(_) => {}
        }
    }

    #[test]
    fn writer_needs_path_contract_is_explicit() {
        assert!(WRITER_NEEDS_PATH.contains("PATH"));
        assert!(WRITER_NEEDS_PATH.contains("writer"));
        assert!(WRITER_NEEDS_PATH.contains("reader-only"));
    }

    #[test]
    fn writer_missing_pandoc_is_explicit_path_error() {
        if pandoc_cli_available() {
            return;
        }
        let err = write_via_cli("{}", "markdown").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("PATH") && msg.contains("reader-only"),
            "expected explicit writer PATH error, got: {msg}"
        );
    }
}
