use std::path::PathBuf;
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
) -> Result<()> {
    let project_config = ProjectConfig::resolve(config_path).unwrap_or_default();

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

    // Watch each file's parent directory
    let mut watched_dirs = std::collections::HashSet::new();
    for file in &files {
        if let Some(parent) = file.parent() {
            let dir = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            if watched_dirs.insert(dir.clone()) {
                watcher
                    .watch(&dir, RecursiveMode::NonRecursive)
                    .with_context(|| format!("failed to watch {}", dir.display()))?;
            }
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
                    match format_file_in_place(path, format_override, &project_config) {
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
) -> Result<bool> {
    if project_config.is_ignored(path) {
        return Ok(false);
    }

    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let format = format_override.unwrap_or_else(|| {
        let detected = Format::from_path(path);
        if detected != Format::Plaintext {
            detected
        } else {
            project_config
                .default_format
                .as_deref()
                .map(Format::from_extension)
                .unwrap_or(Format::Plaintext)
        }
    });
    let format_key = format.config_key();

    let config = FormatConfig {
        format,
        max_width: project_config.max_width_for_format(format_key).unwrap_or(0),
        neural_lang: project_config
            .lang
            .clone()
            .unwrap_or_else(|| "en".to_string()),
        extra_abbreviations: project_config.abbreviations_for_format(format_key),
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

/// Resolve glob patterns to file paths.
fn resolve_patterns(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for pattern in patterns {
        let path = PathBuf::from(pattern);
        if path.exists() && path.is_file() {
            files.push(path);
        } else {
            // Try glob expansion
            for p in glob::glob(pattern)
                .with_context(|| format!("invalid pattern: {pattern}"))?
                .flatten()
            {
                if p.is_file() {
                    files.push(p);
                }
            }
        }
    }
    Ok(files)
}
