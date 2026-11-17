//! Diff-aware `--changed`: reflow and check only paragraphs that contain
//! a line git considers added or modified.
//!
//! The comparison is `git diff -U0 <ref>` against the work tree, plus
//! untracked files. A paragraph is a run of non-blank lines. Editing one
//! line of a paragraph reflows that paragraph, because a sentence break
//! moves the lines around it. Paragraphs with no added or modified line
//! stay byte for byte.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::format::Format;
use crate::{FormatConfig, format_range, format_text};

/// Which lines of a file `--changed` is allowed to rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touch {
    /// Every line. Used for files that do not exist at the base ref.
    All,
    /// 1-indexed lines added or modified in the work tree.
    Lines(Vec<usize>),
}

/// 1-indexed inclusive line numbers added on the new side of a unified diff.
///
/// Counts come from `@@ -old +new @@` headers. A missing count means one
/// line. A new-side count of zero is a deletion and contributes nothing.
pub fn added_lines(diff: &str) -> Vec<usize> {
    let mut lines = BTreeSet::new();
    for header in diff.lines().filter_map(hunk_header) {
        let (start, count) = header;
        if count == 0 {
            continue;
        }
        for line in start..start.saturating_add(count) {
            lines.insert(line);
        }
    }
    lines.into_iter().collect()
}

/// Paragraph ranges (1-indexed, inclusive) that contain any `touched` line.
///
/// Blank lines separate paragraphs and are not part of a range. A touched
/// line past the end of `text` is ignored.
pub fn paragraphs_touching(text: &str, touched: &[usize]) -> Vec<(usize, usize)> {
    if touched.is_empty() {
        return Vec::new();
    }
    let touched: BTreeSet<usize> = touched.iter().copied().collect();
    let lines: Vec<&str> = text.lines().collect();
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim().is_empty() {
            index += 1;
            continue;
        }
        let start = index;
        while index < lines.len() && !lines[index].trim().is_empty() {
            index += 1;
        }
        let start_no = start + 1;
        let end_no = index;
        if (start_no..=end_no).any(|line| touched.contains(&line)) {
            ranges.push((start_no, end_no));
        }
    }
    ranges
}

/// Line numbers whose diagnostics count for this touch set.
///
/// `None` means every line. `Some` is every line of a touched paragraph,
/// which is wider than the git hunk when the edit sits inside a paragraph.
pub fn diagnostic_lines(text: &str, touch: &Touch) -> Option<Vec<usize>> {
    match touch {
        Touch::All => None,
        Touch::Lines(lines) => {
            let mut out = Vec::new();
            for (start, end) in paragraphs_touching(text, lines) {
                out.extend(start..=end);
            }
            Some(out)
        }
    }
}

/// Reflow `touch` and leave every other paragraph as it was.
pub fn reflow_touch(input: &str, config: &FormatConfig, touch: &Touch) -> Result<String> {
    match touch {
        Touch::All => format_text(input, config),
        Touch::Lines(lines) if lines.is_empty() => Ok(input.to_string()),
        Touch::Lines(lines) => {
            let ranges = paragraphs_touching(input, lines);
            let mut output = input.to_string();
            // Later ranges first so an earlier range keeps its line numbers.
            for (start, end) in ranges.into_iter().rev() {
                output = format_range(&output, config, start, end)?;
            }
            Ok(output)
        }
    }
}

/// Prose files that differ from `base` or are untracked in the repo at `cwd`.
pub fn changed_prose_files(base: &str) -> Result<Vec<PathBuf>> {
    validate_ref(base)?;
    let top = toplevel(&std::env::current_dir().context("failed to read the working directory")?)?;
    let mut names = BTreeSet::new();
    let diff = git(
        &top,
        &[
            "diff",
            "-z",
            "--name-only",
            "--diff-filter=ACMR",
            "--find-renames",
            base,
        ],
    )?;
    names.extend(null_split(&diff));
    let untracked = git(&top, &["ls-files", "-z", "--others", "--exclude-standard"])?;
    names.extend(null_split(&untracked));
    let mut paths = Vec::new();
    for name in names {
        if name.is_empty() {
            continue;
        }
        let path = top.join(&name);
        if path.is_file() && Format::recognized_from_path(&path).is_some() {
            paths.push(path);
        }
    }
    Ok(paths)
}

/// Lines of `path` that `--changed` may rewrite, compared with `base`.
pub fn touch_for_path(base: &str, path: &Path) -> Result<Touch> {
    validate_ref(base)?;
    let abs = std::path::absolute(path)
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    let start = abs.parent().unwrap_or(Path::new("."));
    let top = toplevel(start)?;
    let rel = repo_relative(&abs, &top)?;
    let rel_s = rel.to_string_lossy().replace('\\', "/");
    let diff = git(
        &top,
        &[
            "diff",
            "-U0",
            "--no-color",
            "--find-renames",
            base,
            "--",
            &rel_s,
        ],
    )?;
    if diff_has_file(&diff) {
        if diff.contains("Binary files") && !diff.contains("@@") {
            return Ok(Touch::All);
        }
        return Ok(Touch::Lines(added_lines(&diff)));
    }
    if exists_at_ref(&top, base, &rel_s)? {
        Ok(Touch::Lines(Vec::new()))
    } else {
        Ok(Touch::All)
    }
}

fn repo_relative(path: &Path, top: &Path) -> Result<PathBuf> {
    if let Ok(rel) = path.strip_prefix(top) {
        return Ok(rel.to_path_buf());
    }
    let path_c = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let top_c = top.canonicalize().unwrap_or_else(|_| top.to_path_buf());
    path_c
        .strip_prefix(&top_c)
        .map(|rel| rel.to_path_buf())
        .with_context(|| {
            format!(
                "{} is outside the git repository {}",
                path.display(),
                top.display()
            )
        })
}

fn validate_ref(base: &str) -> Result<()> {
    if base.is_empty()
        || base.starts_with('-')
        || base
            .chars()
            .any(|c| c.is_whitespace() || c == '\0' || c == ':')
    {
        bail!("invalid git ref {base:?} for --changed");
    }
    Ok(())
}

fn toplevel(start: &Path) -> Result<PathBuf> {
    let stdout = git(start, &["rev-parse", "--show-toplevel"]).with_context(|| {
        format!(
            "not a git repository (needed for --changed; looked from {})",
            start.display()
        )
    })?;
    let line = stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .context("git rev-parse --show-toplevel returned no path")?;
    Ok(PathBuf::from(line))
}

fn exists_at_ref(top: &Path, base: &str, rel: &str) -> Result<bool> {
    let spec = format!("{base}:{rel}");
    let output = Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["cat-file", "-e", &spec])
        .output()
        .with_context(|| format!("failed to run git cat-file -e {spec}"))?;
    Ok(output.status.success())
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn null_split(text: &str) -> Vec<String> {
    text.split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn diff_has_file(diff: &str) -> bool {
    diff.lines().any(|line| line.starts_with("diff --git "))
}

/// New-side `(start, count)` from a hunk header, if `line` is one.
fn hunk_header(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix("@@ ")?;
    let header = rest.split(" @@").next()?;
    let plus = header
        .split_whitespace()
        .find(|part| part.starts_with('+'))?;
    let spec = plus.strip_prefix('+')?;
    let (start, count) = match spec.split_once(',') {
        Some((start, count)) => (start.parse().ok()?, count.parse().ok()?),
        None => (spec.parse().ok()?, 1),
    };
    Some((start, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunk_headers_collect_the_new_side() {
        let diff = "\
diff --git a/paper.md b/paper.md
index 111..222 100644
--- a/paper.md
+++ b/paper.md
@@ -1,2 +1,3 @@
+added
@@ -10 +12,2 @@
@@ -20,1 +22,0 @@
@@ -30 +30 @@
";
        assert_eq!(added_lines(diff), vec![1, 2, 3, 12, 13, 30]);
    }

    #[test]
    fn a_new_file_hunk_starts_at_one() {
        let diff = "@@ -0,0 +1,2 @@\n+one\n+two\n";
        assert_eq!(added_lines(diff), vec![1, 2]);
    }

    #[test]
    fn paragraphs_skip_blanks_and_untouched_blocks() {
        let text = "One. Two.\n\nKeep this.\nStill this.\n\nTail.\n";
        assert_eq!(paragraphs_touching(text, &[1]), vec![(1, 1)]);
        assert_eq!(paragraphs_touching(text, &[4]), vec![(3, 4)]);
        assert_eq!(paragraphs_touching(text, &[]), Vec::<(usize, usize)>::new());
        assert_eq!(paragraphs_touching(text, &[3, 6]), vec![(3, 4), (6, 6)]);
    }

    #[test]
    fn diagnostic_lines_cover_the_paragraph() {
        let text = "One. Two.\nMore.\n\nOther.\n";
        let touch = Touch::Lines(vec![2]);
        assert_eq!(diagnostic_lines(text, &touch), Some(vec![1, 2]));
        assert_eq!(diagnostic_lines(text, &Touch::All), None);
        assert_eq!(
            diagnostic_lines(text, &Touch::Lines(vec![])),
            Some(Vec::<usize>::new())
        );
    }

    #[test]
    fn a_ref_that_looks_like_a_flag_is_rejected() {
        assert!(validate_ref("--cached").is_err());
        assert!(validate_ref("origin/main").is_ok());
        assert!(validate_ref("HEAD~1").is_ok());
        assert!(validate_ref("HEAD:file").is_err());
    }

    #[test]
    fn reflow_touch_leaves_an_untouched_paragraph() {
        let input = "Old fused sentence. Still old.\n\nFresh sentence.\n";
        let config = crate::FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let kept = reflow_touch(input, &config, &Touch::Lines(vec![3])).unwrap();
        assert_eq!(kept, input);
        let fixed = reflow_touch(input, &config, &Touch::Lines(vec![1])).unwrap();
        assert_eq!(
            fixed,
            "Old fused sentence.\nStill old.\n\nFresh sentence.\n"
        );
    }
}
