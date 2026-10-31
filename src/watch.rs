use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::ProjectConfig;
use crate::format::Format;
use crate::{FormatConfig, format_text};

/// Run the file watcher, reformatting files on change.
pub fn run_watch(
    patterns: &[String],
    format_override: Option<Format>,
    config_path: Option<&std::path::Path>,
    native: bool,
    use_pandoc_flag: bool,
) -> Result<()> {
    let project_config = ProjectConfig::resolve(config_path)?;
    let use_pandoc = if native {
        false
    } else if use_pandoc_flag {
        true
    } else {
        project_config.use_pandoc.unwrap_or(false)
    };

    // Resolve patterns to actual file paths
    let files = resolve_patterns(patterns)?;
    if files.is_empty() {
        anyhow::bail!("no files matched the given patterns");
    }

    eprintln!(
        "Watching {} file(s) for changes (Ctrl+C to stop):",
        files.len()
    );
    for f in &files {
        eprintln!("  {}", f.display());
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())
        .context("failed to create file watcher")?;

    // Watch each file's parent directory. `resolve_patterns` stores
    // canonical paths, so the parent exists and shares a prefix with the
    // events from a symlinked working directory.
    let mut watched_dirs = std::collections::HashSet::new();
    for file in &files {
        let parent = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .with_context(|| format!("no directory to watch for {}", file.display()))?;
        let dir = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        if watched_dirs.insert(dir.clone()) {
            watcher
                .watch(&dir, RecursiveMode::NonRecursive)
                .with_context(|| format!("failed to watch {}", dir.display()))?;
        }
    }

    let debounce = Duration::from_millis(200);
    let mut last_format: std::collections::HashMap<PathBuf, Instant> =
        std::collections::HashMap::new();

    loop {
        match rx.recv() {
            Ok(Ok(event)) => {
                if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    continue;
                }

                for path in &event.paths {
                    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

                    // Check if this file is in our watch list
                    let is_watched = files
                        .iter()
                        .any(|f| f.canonicalize().unwrap_or_else(|_| f.clone()) == canonical);
                    if !is_watched {
                        continue;
                    }

                    // Debounce
                    let now = Instant::now();
                    if let Some(last) = last_format.get(&canonical) {
                        if now.duration_since(*last) < debounce {
                            continue;
                        }
                    }
                    last_format.insert(canonical.clone(), now);

                    // Format
                    match format_file_in_place(path, format_override, &project_config, use_pandoc) {
                        Err(e) => eprintln!("  error formatting {}: {e}", path.display()),
                        Ok(true) => {
                            eprintln!("  formatted: {}", path.display());
                        }
                        Ok(false) => {}
                    }
                }
            }
            Ok(Err(e)) => eprintln!("  watch error: {e}"),
            Err(e) => {
                eprintln!("  channel error: {e}");
                break;
            }
        }
    }

    Ok(())
}

fn format_file_in_place(
    path: &std::path::Path,
    format_override: Option<Format>,
    project_config: &ProjectConfig,
    use_pandoc: bool,
) -> Result<bool> {
    if project_config.is_ignored(path) {
        return Ok(false);
    }

    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let format = match format_override.or_else(|| Format::recognized_from_path(path)) {
        Some(f) => f,
        None => return Ok(false),
    };
    let format_key = format.config_key();

    let config = FormatConfig {
        format,
        max_width: project_config.max_width_for_format(format_key).unwrap_or(0),
        neural_lang: project_config
            .lang
            .clone()
            .unwrap_or_else(|| "en".to_string()),
        extra_abbreviations: project_config.abbreviations_for_format(format_key),
        latex_verbatim_envs: project_config.latex_verbatim_envs(),
        latex_structure_envs: project_config.latex_structure_envs(),
        latex_verbatim_commands: project_config.latex_verbatim_commands(),
        clause_breaks: project_config.clause_breaks.unwrap_or(false),
        lowercase_starts: project_config.lowercase_starts_for_format(format_key),
        code: project_config.code.clone(),
        use_pandoc,
        ..Default::default()
    };

    let output = format_text(&input, &config)?;
    if output != input {
        std::fs::write(path, &output)
            .with_context(|| format!("failed to write {}", path.display()))?;
        return Ok(true);
    }

    Ok(false)
}

/// Resolve glob patterns to canonical file paths.
///
/// A relative pattern such as `a.md` has an empty parent. macOS FSEvents
/// rejects a watch on that empty path, since the path does not exist.
/// Linux inotify joins the empty path onto the working directory, so the
/// same call watches the right directory there. Canonical paths also keep
/// the watch and the events on one prefix when the working directory is a
/// symlink, which is how macOS lays out `/tmp` and `/var`.
fn resolve_patterns(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let cwd = std::env::current_dir().context("failed to get current directory")?;
    let mut files = Vec::new();
    for pattern in patterns {
        let path = PathBuf::from(pattern);
        if path.exists() && path.is_file() {
            files.push(canonical_file(&path, &cwd)?);
        } else {
            // Try glob expansion
            for p in glob::glob(pattern)
                .with_context(|| format!("invalid pattern: {pattern}"))?
                .flatten()
            {
                if p.is_file() {
                    files.push(canonical_file(&p, &cwd)?);
                }
            }
        }
    }
    Ok(files)
}

fn canonical_file(path: &Path, cwd: &Path) -> Result<PathBuf> {
    let absolute = cwd.join(path);
    absolute
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", absolute.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_file_resolves_to_a_directory_the_watcher_can_open() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "Start.\n").unwrap();
        let file = canonical_file(Path::new("a.md"), dir.path()).unwrap();
        let parent = file.parent().unwrap();
        assert!(!parent.as_os_str().is_empty());
        assert!(parent.exists());
        assert_eq!(parent, dir.path().canonicalize().unwrap());
        assert_eq!(file, dir.path().join("a.md").canonicalize().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_workdir_resolves_to_the_canonical_directory() {
        let real = tempfile::tempdir().unwrap();
        std::fs::write(real.path().join("a.md"), "Start.\n").unwrap();
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("tmp");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        let file = canonical_file(Path::new("a.md"), &link).unwrap();
        let parent = file.parent().unwrap();
        assert_eq!(parent, real.path().canonicalize().unwrap());
        assert_ne!(parent, link.as_path());
        assert!(parent.exists());
    }
}
