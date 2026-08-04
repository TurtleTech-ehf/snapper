use imara_diff::{Algorithm, BasicLineDiffPrinter, Diff, InternedInput, UnifiedDiffConfig};

/// When colored unified-diff output should be emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Color when stdout is an interactive terminal.
    #[default]
    Auto,
    /// Always emit ANSI color codes.
    Always,
    /// Never emit ANSI color codes.
    Never,
}

impl ColorMode {
    /// Resolve whether to colorize given a TTY probe (or forced override).
    pub fn should_colorize(self, is_tty: bool) -> bool {
        match self {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_tty,
        }
    }
}

/// Apply ANSI styling to a unified-diff body (headers, hunks, adds, removals).
///
/// Lines that already lack a leading `+`/`-`/`@`/`---`/`+++` marker are left
/// unchanged. Empty input stays empty.
pub fn colorize_unified_diff(diff: &str) -> String {
    if diff.is_empty() {
        return String::new();
    }
    let mut output = String::with_capacity(diff.len() + 32);
    for line in diff.lines() {
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            output.push_str("\x1b[1m");
            output.push_str(line);
            output.push_str("\x1b[0m\n");
        } else if line.starts_with("@@") {
            output.push_str("\x1b[36m");
            output.push_str(line);
            output.push_str("\x1b[0m\n");
        } else if line.starts_with('+') {
            output.push_str("\x1b[32m");
            output.push_str(line);
            output.push_str("\x1b[0m\n");
        } else if line.starts_with('-') {
            output.push_str("\x1b[31m");
            output.push_str(line);
            output.push_str("\x1b[0m\n");
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if diff.ends_with('\n') && !output.ends_with('\n') {
        output.push('\n');
    }
    // Preserve trailing newline when present; unified diffs always end with \n
    // after the loop above. If the input had no trailing newline, trim once.
    if !diff.ends_with('\n') && output.ends_with('\n') {
        output.pop();
    }
    output
}

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
        .unified_diff(&BasicLineDiffPrinter(&input.interner), config, &input)
        .to_string();

    if body.is_empty() {
        return String::new();
    }
    format!("--- a/{}\n+++ b/{}\n{}", path, path, body)
}

/// Print a unified diff to stdout. No-op if texts are identical.
///
/// When `color` is true, headers / hunks / additions / removals are ANSI-styled.
pub fn print_diff(path: &str, original: &str, formatted: &str, color: bool) {
    let diff = unified_diff(path, original, formatted);
    if diff.is_empty() {
        return;
    }
    if color {
        print!("{}", colorize_unified_diff(&diff));
    } else {
        print!("{diff}");
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

    #[test]
    fn color_mode_resolution() {
        assert!(ColorMode::Always.should_colorize(false));
        assert!(ColorMode::Always.should_colorize(true));
        assert!(!ColorMode::Never.should_colorize(false));
        assert!(!ColorMode::Never.should_colorize(true));
        assert!(!ColorMode::Auto.should_colorize(false));
        assert!(ColorMode::Auto.should_colorize(true));
    }

    #[test]
    fn colorize_adds_ansi_for_headers_hunks_and_changes() {
        let plain = "--- a/t.md\n+++ b/t.md\n@@ -1 +1 @@\n-old line\n+new line\n context\n";
        let colored = colorize_unified_diff(plain);
        assert!(
            colored.contains("\x1b[1m--- a/t.md\x1b[0m"),
            "headers should be bold: {colored:?}"
        );
        assert!(
            colored.contains("\x1b[1m+++ b/t.md\x1b[0m"),
            "headers should be bold: {colored:?}"
        );
        assert!(
            colored.contains("\x1b[36m@@ -1 +1 @@\x1b[0m"),
            "hunk headers should be cyan: {colored:?}"
        );
        assert!(
            colored.contains("\x1b[31m-old line\x1b[0m"),
            "removals should be red: {colored:?}"
        );
        assert!(
            colored.contains("\x1b[32m+new line\x1b[0m"),
            "additions should be green: {colored:?}"
        );
        assert!(
            colored.contains(" context\n"),
            "context lines stay plain: {colored:?}"
        );
        assert!(
            colored.contains("\x1b["),
            "colored output must contain ANSI escapes"
        );
    }

    #[test]
    fn colorize_empty_is_empty() {
        assert!(colorize_unified_diff("").is_empty());
    }

    #[test]
    fn plain_unified_diff_has_no_ansi() {
        let diff = unified_diff("foo.md", "a\n", "b\n");
        assert!(
            !diff.contains('\x1b'),
            "raw unified_diff must not emit ANSI: {diff:?}"
        );
        assert!(!colorize_unified_diff(&diff).is_empty());
    }
}
