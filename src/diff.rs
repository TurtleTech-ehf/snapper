use imara_diff::{Algorithm, BasicLineDiffPrinter, Diff, InternedInput, UnifiedDiffConfig};

/// Produce a unified diff between original and formatted text using
/// imara-diff's histogram algorithm (same as git's default).
///
/// Returns an empty string if the texts are identical.
pub fn unified_diff(path: &str, original: &str, formatted: &str) -> String {
    let input = InternedInput::new(original, formatted);
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    let mut config = UnifiedDiffConfig::default();
    config.context_len(3);

    let body = diff
        .unified_diff(
            &BasicLineDiffPrinter(&input.interner),
            config,
            &input,
        )
        .to_string();

    if body.is_empty() {
        return String::new();
    }
    format!("--- a/{}\n+++ b/{}\n{}", path, path, body)
}

/// Print a unified diff to stdout. No-op if texts are identical.
pub fn print_diff(path: &str, original: &str, formatted: &str) {
    let diff = unified_diff(path, original, formatted);
    if !diff.is_empty() {
        print!("{}", diff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_texts_produce_empty_diff() {
        let text = "Hello world.\nLine two.\n";
        assert!(unified_diff("test.md", text, text).is_empty());
    }

    #[test]
    fn split_lines_show_correctly() {
        let old = "First.\nSecond line. Third line.\nFourth.\n";
        let new = "First.\nSecond line.\nThird line.\nFourth.\n";
        let diff = unified_diff("test.md", old, new);
        assert!(diff.contains("-Second line. Third line."));
        assert!(diff.contains("+Second line."));
        assert!(diff.contains("+Third line."));
        assert!(diff.contains(" First."));
        assert!(diff.contains(" Fourth."));
    }

    #[test]
    fn preserves_structural_context() {
        let old = "# Heading\n\nHello world. This is a test.\n\n# Other\n";
        let new = "# Heading\n\nHello world.\nThis is a test.\n\n# Other\n";
        let diff = unified_diff("test.md", old, new);
        assert!(diff.contains(" # Heading"));
        assert!(diff.contains("-Hello world. This is a test."));
        assert!(diff.contains("+Hello world."));
    }

    #[test]
    fn file_headers_present() {
        let diff = unified_diff("foo.org", "a\n", "b\n");
        assert!(diff.starts_with("--- a/foo.org\n+++ b/foo.org\n"));
    }
}
