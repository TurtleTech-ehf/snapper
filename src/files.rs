use anyhow::{Context, Result};
use std::path::Path;

use crate::FormatConfig;
use crate::diff;
use crate::format::Format;

/// Format a file on disk, returning the formatted text.
/// Auto-detects format from the file extension.
pub fn format_file(path: &str, max_width: usize) -> Result<String> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path))?;
    let fmt = Format::from_path(Path::new(path));
    let config = FormatConfig {
        format: fmt,
        max_width,
        ..Default::default()
    };
    crate::format_text(&content, &config)
}

/// Format a file with extra abbreviations from a project config.
pub fn format_file_with(
    path: &str,
    max_width: usize,
    extra_abbreviations: Vec<String>,
) -> Result<String> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path))?;
    let fmt = Format::from_path(Path::new(path));
    let config = FormatConfig {
        format: fmt,
        max_width,
        extra_abbreviations,
        ..Default::default()
    };
    crate::format_text(&content, &config)
}

/// Check if a file needs formatting. Returns `None` if already formatted,
/// or `Some(unified_diff)` if changes are needed.
pub fn check_file(path: &str, max_width: usize) -> Result<Option<String>> {
    let original =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path))?;
    let formatted = format_file(path, max_width)?;
    if original == formatted {
        return Ok(None);
    }
    Ok(Some(diff::unified_diff(path, &original, &formatted)))
}

/// Format a file in-place. Returns `true` if the file was changed.
pub fn format_in_place(path: &str, max_width: usize) -> Result<bool> {
    let original =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path))?;
    let formatted = format_file(path, max_width)?;
    if original == formatted {
        return Ok(false);
    }
    std::fs::write(path, &formatted).with_context(|| format!("failed to write {}", path))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_formatted() {
        let tmp = std::env::temp_dir().join("snap_files_ok.md");
        std::fs::write(&tmp, "Hello world.\nThis is a test.\n").unwrap();
        let result = check_file(tmp.to_str().unwrap(), 0).unwrap();
        assert!(result.is_none());
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn needs_formatting() {
        let tmp = std::env::temp_dir().join("snap_files_needs.md");
        std::fs::write(&tmp, "Hello world. This is a test. Another sentence.\n").unwrap();
        let result = check_file(tmp.to_str().unwrap(), 0).unwrap();
        assert!(result.is_some());
        let diff = result.unwrap();
        assert!(diff.contains("--- a/"));
        assert!(diff.contains("+Hello world."));
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn format_in_place_changes_file() {
        let tmp = std::env::temp_dir().join("snap_files_inplace.md");
        std::fs::write(&tmp, "Hello world. This is a test.\n").unwrap();
        let changed = format_in_place(tmp.to_str().unwrap(), 0).unwrap();
        assert!(changed);
        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("Hello world.\n"));
        assert!(content.contains("This is a test.\n"));
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn format_in_place_noop_when_formatted() {
        let tmp = std::env::temp_dir().join("snap_files_noop.md");
        std::fs::write(&tmp, "Hello world.\nThis is a test.\n").unwrap();
        let changed = format_in_place(tmp.to_str().unwrap(), 0).unwrap();
        assert!(!changed);
        std::fs::remove_file(&tmp).ok();
    }
}
