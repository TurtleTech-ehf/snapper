//! Write a pandoc JSON AST through the installed pandoc writer.
//!
//! The FFI library is reader-only. Reconstruction of source bytes is
//! impossible (no offsets) and is not attempted: `pandoc -f json -t <format>
//! --wrap=preserve` owns emission. SoftBreaks inserted by AST reflow become
//! sentence line breaks.

use std::io::Write;
use std::process::{Command, Stdio};

use super::cli::CliError;

/// Render `json` (a pandoc AST) as `format` via the CLI writer.
pub fn write_via_cli(json: &str, format: &str) -> Result<String, CliError> {
    let mut child = Command::new("pandoc")
        .args(["-f", "json", "-t", format, "--wrap=preserve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| CliError::Spawn(e.to_string()))?;

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
}
