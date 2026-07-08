//! Subprocess pandoc backend (`pandoc -t json`).
//!
//! Still uses [`super::ast::regions_from_pandoc_json`] for classification so
//! structure truth is the AST, not a second heuristic pass. Failures are
//! explicit errors (no silent all-prose fallback).

use std::io::Write;
use std::process::{Command, Stdio};

use thiserror::Error;

use super::ast::regions_from_pandoc_json;
use crate::parser::Region;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("pandoc CLI unavailable or failed to start: {0}")]
    Spawn(String),
    #[error("pandoc CLI exited with failure: {0}")]
    Exit(String),
    #[error("pandoc CLI returned invalid AST: {0}")]
    InvalidAst(String),
}

/// Check if a `pandoc` executable is on PATH.
pub fn pandoc_cli_available() -> bool {
    Command::new("pandoc")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Run `pandoc -f <format> -t json` and classify the AST into regions.
pub fn parse_via_cli(input: &str, format: &str) -> Result<Vec<Region>, CliError> {
    let mut child = Command::new("pandoc")
        .args(["-f", format, "-t", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| CliError::Spawn(e.to_string()))?;

    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(input.as_bytes())
            .map_err(|e| CliError::Spawn(format!("write stdin: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| CliError::Spawn(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Exit(format!(
            "status {}: {stderr}",
            output.status
        )));
    }

    let json = String::from_utf8(output.stdout)
        .map_err(|e| CliError::InvalidAst(format!("stdout not UTF-8: {e}")))?;
    regions_from_pandoc_json(&json).map_err(CliError::InvalidAst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_availability_check_does_not_panic() {
        let _ = pandoc_cli_available();
    }

    #[test]
    fn cli_unknown_format_is_explicit_error() {
        if !pandoc_cli_available() {
            return;
        }
        let err = parse_via_cli("Hello.", "not-a-real-pandoc-format-xyz").unwrap_err();
        match err {
            CliError::Exit(_) | CliError::Spawn(_) | CliError::InvalidAst(_) => {}
        }
    }
}
