use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::format::Format;
use crate::sdiff::sentence_diff;

/// Get the list of changed files between a git ref and the working tree.
fn changed_files(git_ref: &str) -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", git_ref])
        .output()
        .context("failed to run git diff --name-only")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect())
}

/// Get the contents of a file at a specific git ref.
fn file_at_ref(git_ref: &str, path: &Path) -> Result<String> {
    let spec = format!("{git_ref}:{}", path.display());
    let output = Command::new("git")
        .args(["show", &spec])
        .output()
        .with_context(|| format!("failed to run git show {spec}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git show {spec} failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run sentence-level diff against a git ref for one or more files.
pub fn run_git_diff(
    git_ref: &str,
    files: &[PathBuf],
    format: Option<Format>,
    color: bool,
) -> Result<bool> {
    let files_to_diff = if files.is_empty() {
        // Auto-detect changed files
        let changed = changed_files(git_ref)?;
        // Filter to prose file extensions
        changed
            .into_iter()
            .filter(|p| {
                matches!(
                    Format::from_path(p),
                    Format::Org | Format::Latex | Format::Markdown
                )
            })
            .collect::<Vec<_>>()
    } else {
        files.to_vec()
    };

    if files_to_diff.is_empty() {
        eprintln!("No prose files changed.");
        return Ok(false);
    }

    let mut any_diff = false;

    for path in &files_to_diff {
        // Get the old version from git
        let old_content = match file_at_ref(git_ref, path) {
            Ok(c) => c,
            Err(_) => {
                // File didn't exist at that ref (new file), skip
                continue;
            }
        };

        // Write old content to a temp file for sdiff
        let tmp = tempfile::NamedTempFile::new().context("failed to create temp file")?;
        std::fs::write(tmp.path(), &old_content)?;

        let fmt = format.or_else(|| Some(Format::from_path(path)));
        let diff_output = sentence_diff(tmp.path(), path, fmt, color)?;

        if !diff_output.is_empty() {
            // Replace temp path with git ref in the header
            let display = diff_output
                .replace(
                    &format!("a/{}", tmp.path().display()),
                    &format!("a/{} ({git_ref})", path.display()),
                )
                .replace(
                    &format!("b/{}", tmp.path().display()),
                    &format!("b/{}", path.display()),
                );
            print!("{display}");
            any_diff = true;
        }
    }

    if !any_diff {
        eprintln!("No sentence-level differences.");
    }

    Ok(any_diff)
}
