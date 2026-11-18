use std::collections::HashMap;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use rayon::prelude::*;

use snapper_fmt::check::{DiagnosticKind, collect_diagnostics, resolve_long_threshold};
use snapper_fmt::cli::{Cli, ColorWhen, Commands, OutputFormat, parse_range};
use snapper_fmt::config::ProjectConfig;
use snapper_fmt::diff::ColorMode;
use snapper_fmt::format::Format;
use snapper_fmt::output::{CheckResult, output_json, output_sarif};
use snapper_fmt::sentence::SentenceSplitter;
use snapper_fmt::{
    FormatConfig, build_splitter, format_range, format_text, format_text_with_splitter,
};

/// Resolve whether colored diff output should be used.
///
/// `no_color` (legacy subcommand flag) forces off; otherwise `--color` decides
/// with auto = stdout is a TTY and `NO_COLOR` unset.
fn resolve_color(when: ColorWhen, no_color: bool) -> bool {
    if no_color {
        return false;
    }
    let mode = ColorMode::from(when);
    // `auto` honors the NO_COLOR convention (https://no-color.org): any
    // non-empty value disables color; explicit `always` still wins.
    if mode == ColorMode::Auto && std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return false;
    }
    mode.should_colorize(io::stdout().is_terminal())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    if let Some(ref cmd) = cli.command {
        match cmd {
            Commands::Init { dry_run } => return snapper_fmt::init::run_init(*dry_run),
            Commands::Sdiff {
                old,
                new,
                format,
                no_color,
            } => {
                let fmt = format.map(Format::from_arg);
                let color = resolve_color(cli.color, *no_color);
                let result = snapper_fmt::sdiff::sentence_diff(old, new, fmt, color)?;
                if result.is_empty() {
                    eprintln!("No sentence-level differences.");
                } else {
                    print!("{result}");
                    process::exit(1);
                }
                return Ok(());
            }
            Commands::GitDiff {
                git_ref,
                files,
                format,
                no_color,
            } => {
                let fmt = format.map(Format::from_arg);
                let color = resolve_color(cli.color, *no_color);
                let has_diff = snapper_fmt::git_diff::run_git_diff(git_ref, files, fmt, color)?;
                if has_diff {
                    process::exit(1);
                }
                return Ok(());
            }
            Commands::Lsp => {
                #[cfg(feature = "lsp")]
                {
                    let rt =
                        tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
                    rt.block_on(snapper_fmt::lsp::run_lsp());
                    return Ok(());
                }
                #[cfg(not(feature = "lsp"))]
                {
                    eprintln!("error: snapper was built without the 'lsp' feature");
                    eprintln!("rebuild with: cargo install snapper-fmt --features lsp");
                    process::exit(1);
                }
            }
            Commands::Mcp => {
                #[cfg(feature = "mcp")]
                {
                    let rt =
                        tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
                    rt.block_on(snapper_fmt::mcp::run_mcp())?;
                    return Ok(());
                }
                #[cfg(not(feature = "mcp"))]
                {
                    eprintln!("error: snapper was built without the 'mcp' feature");
                    eprintln!("rebuild with: cargo install snapper-fmt --features mcp");
                    process::exit(1);
                }
            }
            Commands::Watch { patterns, format } => {
                #[cfg(feature = "watch")]
                {
                    let fmt = format.map(Format::from_arg);
                    return snapper_fmt::watch::run_watch(
                        patterns,
                        fmt,
                        cli.config.as_deref(),
                        cli.native,
                        cli.use_pandoc,
                    );
                }
                #[cfg(not(feature = "watch"))]
                {
                    let _ = (patterns, format);
                    eprintln!("error: snapper was built without the 'watch' feature");
                    eprintln!("rebuild with: cargo install snapper-fmt --features watch");
                    process::exit(1);
                }
            }
        }
    }

    let project_config =
        ProjectConfig::resolve(cli.config.as_deref()).context("failed to load .snapperrc.toml")?;

    if cli.files.is_empty() && !cli.changed {
        // Read from stdin
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .context("failed to read stdin")?;

        if let Some(path) = cli.stdin_filepath.as_deref() {
            if project_config.file_skipped(path) {
                if cli.check || cli.diff {
                    return Ok(());
                }
                if let Some(ref out) = cli.output {
                    fs::write(out, &input)
                        .with_context(|| format!("failed to write {}", out.display()))?;
                } else {
                    print!("{input}");
                }
                return Ok(());
            }
        }

        // Format detection: --format > --stdin-filepath > plaintext
        let format = resolve_format(
            cli.format.map(Format::from_arg),
            cli.stdin_filepath.as_deref(),
            &project_config,
        )?;
        let config =
            build_format_config(&cli, &project_config, format, cli.stdin_filepath.as_deref());

        let output = if let Some(ref range_str) = cli.range {
            let (start, end) =
                parse_range(range_str).context("invalid range format, expected START:END")?;
            format_range(&input, &config, start, end)?
        } else {
            format_text(&input, &config)?
        };

        if cli.check {
            let splitter = build_splitter(&config).context("failed to build sentence splitter")?;
            let threshold = resolve_long_threshold(
                config.max_width,
                project_config.long_threshold_for_path(cli.stdin_filepath.as_deref()),
            );
            let diagnostics =
                collect_diagnostics(&input, format, splitter.as_ref(), threshold, Some(&config));
            let would = output != input;
            let long_fail =
                cli.strict_long && diagnostics.iter().any(|d| d.kind == DiagnosticKind::Long);
            let file = cli
                .stdin_filepath
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<stdin>".to_string());
            let check_results = if would || !diagnostics.is_empty() {
                vec![CheckResult {
                    file,
                    original_lines: input.lines().count(),
                    formatted_lines: output.lines().count(),
                    would_reformat: would,
                    diagnostics,
                }]
            } else {
                Vec::new()
            };
            match cli.output_format {
                OutputFormat::Json => output_json(&check_results),
                OutputFormat::Sarif => output_sarif(&check_results),
                OutputFormat::Text => {
                    if would {
                        eprintln!("would reformat: <stdin>");
                    }
                    if let Some(r) = check_results.first() {
                        for d in &r.diagnostics {
                            eprintln!("<stdin>:{}:{}: {}", d.line, d.kind.as_str(), d.excerpt);
                        }
                    }
                }
            }
            if would || long_fail {
                process::exit(1);
            }
        } else if cli.diff {
            let color = resolve_color(cli.color, false);
            snapper_fmt::diff::print_diff("<stdin>", &input, &output, color);
            if output != input {
                process::exit(1);
            }
        } else if let Some(ref path) = cli.output {
            fs::write(path, &output)
                .with_context(|| format!("failed to write {}", path.display()))?;
        } else {
            print!("{output}");
        }
    } else {
        // Process files in parallel whenever there is more than one path.
        // Splitters are built once per distinct config key (format/lang/neural/…)
        // so multi-file and --neural do not reload models per path.
        let discovered = if cli.files.is_empty() {
            let found = snapper_fmt::changed::changed_prose_files(&cli.changed_from)?;
            if found.is_empty() {
                eprintln!("No prose files changed.");
                return Ok(());
            }
            found
        } else {
            Vec::new()
        };
        let file_args: &[PathBuf] = if cli.files.is_empty() {
            &discovered
        } else {
            &cli.files
        };
        let use_parallel = file_args.len() > 1;
        let mut splitter_cache: HashMap<SplitterKey, Arc<dyn SentenceSplitter>> = HashMap::new();

        let paths: Vec<&Path> = file_args
            .iter()
            .map(PathBuf::as_path)
            .filter(|path| !should_skip_path(path, &cli, &project_config))
            .collect();

        // Always pre-warm the cache on the main thread (Sync model load).
        for path in &paths {
            let format = resolve_format(
                cli.format.map(Format::from_arg),
                Some(path),
                &project_config,
            )?;
            let config = build_format_config(&cli, &project_config, format, Some(path));
            let key = SplitterKey::from_config(&config);
            if let std::collections::hash_map::Entry::Vacant(e) = splitter_cache.entry(key) {
                let s = build_splitter(&config).context("failed to build sentence splitter")?;
                e.insert(Arc::from(s));
            }
        }
        let cache = Arc::new(splitter_cache);

        let processed: Vec<Result<FileOutcome>> = if use_parallel {
            paths
                .par_iter()
                .map(|path| process_file(path, &cli, &project_config, &cache))
                .collect()
        } else {
            paths
                .iter()
                .map(|path| process_file(path, &cli, &project_config, &cache))
                .collect()
        };
        let mut results = Vec::new();
        let mut skipped_outside = false;
        for result in processed {
            match result {
                Ok(outcome) => results.push(outcome),
                Err(err) if outside_repo(&err) => {
                    eprintln!("warning: {err:#}");
                    skipped_outside = true;
                }
                Err(err) => return Err(err),
            }
        }

        let mut any_changed = false;
        let mut check_failed = false;
        let mut check_results: Vec<CheckResult> = Vec::new();

        let color = resolve_color(cli.color, false);
        for outcome in &results {
            let path_str = &outcome.display;
            let input = &outcome.input;
            let output = &outcome.output;
            if cli.diff {
                if output != input {
                    snapper_fmt::diff::print_diff(path_str, input, output, color);
                    any_changed = true;
                }
            } else if cli.check {
                let path = outcome.path.as_path();
                let format = resolve_format(
                    cli.format.map(Format::from_arg),
                    Some(path),
                    &project_config,
                )?;
                let config = build_format_config(&cli, &project_config, format, Some(path));
                let key = SplitterKey::from_config(&config);
                let splitter = cache
                    .get(&key)
                    .ok_or_else(|| anyhow::anyhow!("splitter cache miss for {path_str}"))?;
                let threshold = resolve_long_threshold(
                    config.max_width,
                    project_config.long_threshold_for_path(Some(path)),
                );
                let diagnostics =
                    collect_diagnostics(input, format, splitter.as_ref(), threshold, Some(&config));
                let diagnostics = filter_scope(diagnostics, outcome.scope.as_deref());
                let would = output != input;
                let long_fail =
                    cli.strict_long && diagnostics.iter().any(|d| d.kind == DiagnosticKind::Long);
                if would || !diagnostics.is_empty() {
                    match cli.output_format {
                        OutputFormat::Text => {
                            if would {
                                eprintln!("would reformat: {path_str}");
                            }
                            for d in &diagnostics {
                                eprintln!(
                                    "{path_str}:{}:{}: {}",
                                    d.line,
                                    d.kind.as_str(),
                                    d.excerpt
                                );
                            }
                        }
                        _ => check_results.push(CheckResult {
                            file: path_str.clone(),
                            original_lines: input.lines().count(),
                            formatted_lines: output.lines().count(),
                            would_reformat: would,
                            diagnostics,
                        }),
                    }
                }
                if would || long_fail {
                    check_failed = true;
                }
            } else if cli.in_place {
                if output != input {
                    if cli.changed {
                        if let Err(err) =
                            snapper_fmt::changed::write_inside_repo(&outcome.path, output)
                        {
                            if outside_repo(&err) {
                                eprintln!("warning: {err:#}");
                                skipped_outside = true;
                            } else {
                                return Err(err);
                            }
                        }
                    } else {
                        fs::write(&outcome.path, output).with_context(|| {
                            format!("failed to write {}", outcome.path.display())
                        })?;
                    }
                }
            } else if let Some(ref out_path) = cli.output {
                fs::write(out_path, output)
                    .with_context(|| format!("failed to write {}", out_path.display()))?;
            } else {
                print!("{output}");
            }
        }

        // Structured output for check mode (empty array/run is still valid JSON).
        if cli.check {
            match cli.output_format {
                OutputFormat::Json => output_json(&check_results),
                OutputFormat::Sarif => output_sarif(&check_results),
                OutputFormat::Text => {} // already printed above
            }
        }

        if cli.diff && any_changed {
            process::exit(1);
        }
        if cli.check && check_failed {
            process::exit(1);
        }
        if skipped_outside {
            process::exit(1);
        }
    }

    Ok(())
}

fn outside_repo(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<snapper_fmt::changed::RepoPathError>()
            .is_some_and(snapper_fmt::changed::RepoPathError::is_outside)
    })
}

/// Key for reusing sentence splitters across files with the same split policy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SplitterKey {
    format: Format,
    use_neural: bool,
    neural_lang: String,
    neural_model: Option<std::path::PathBuf>,
    extras: Vec<String>,
    lowercase_starts: bool,
}

impl SplitterKey {
    fn from_config(config: &FormatConfig) -> Self {
        Self {
            format: config.format,
            use_neural: config.use_neural,
            neural_lang: config.neural_lang.clone(),
            neural_model: config.neural_model_path.clone(),
            extras: config.extra_abbreviations.clone(),
            lowercase_starts: config.lowercase_starts,
        }
    }
}

/// One formatted file. `scope` is `None` when every line counts, or the
/// 1-indexed lines of paragraphs `--changed` is allowed to report.
struct FileOutcome {
    /// Path used for reading and `--in-place` writes.
    path: PathBuf,
    /// Repository-relative path shown in check, diff, and JSON output.
    display: String,
    input: String,
    output: String,
    scope: Option<Vec<usize>>,
}

fn filter_scope(
    diagnostics: Vec<snapper_fmt::check::LineDiagnostic>,
    scope: Option<&[usize]>,
) -> Vec<snapper_fmt::check::LineDiagnostic> {
    let Some(scope) = scope else {
        return diagnostics;
    };
    diagnostics
        .into_iter()
        .filter(|d| scope.binary_search(&d.line).is_ok())
        .collect()
}

/// Process a single file: read, format, return the path and both texts.
fn process_file(
    path: &Path,
    cli: &Cli,
    project_config: &ProjectConfig,
    splitter_cache: &HashMap<SplitterKey, Arc<dyn SentenceSplitter>>,
) -> Result<FileOutcome> {
    let path_str = repo_relative_display(path);
    let input =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    let format = resolve_format(cli.format.map(Format::from_arg), Some(path), project_config)?;
    let config = build_format_config(cli, project_config, format, Some(path));
    let key = SplitterKey::from_config(&config);
    let splitter = splitter_cache
        .get(&key)
        .ok_or_else(|| anyhow::anyhow!("splitter cache miss for {path_str}"))?;

    if cli.changed {
        let touch = snapper_fmt::changed::touch_for_path(&cli.changed_from, path)?;
        let scope = snapper_fmt::changed::diagnostic_lines(&input, &touch);
        let output = snapper_fmt::changed::reflow_touch(&input, &config, &touch)?;
        return Ok(FileOutcome {
            path: path.to_path_buf(),
            display: path_str,
            input,
            output,
            scope,
        });
    }

    let output = if let Some(ref range_str) = cli.range {
        let (start, end) =
            parse_range(range_str).context("invalid range format, expected START:END")?;
        // Range path still uses config; splitter reuse is best-effort via full-file path.
        let _ = splitter;
        format_range(&input, &config, start, end)?
    } else {
        format_text_with_splitter(&input, &config, splitter.as_ref())?
    };

    Ok(FileOutcome {
        path: path.to_path_buf(),
        display: path_str,
        input,
        output,
        scope: None,
    })
}

/// Path printed for a file. Inside a git checkout this is relative to the
/// repository root, so a run from a subdirectory does not print an absolute path.
fn repo_relative_display(path: &Path) -> String {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let start = abs.parent().unwrap_or(Path::new("."));
    let top = process::Command::new("git")
        .arg("-C")
        .arg(start)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|stdout| {
            stdout
                .lines()
                .next()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
        });
    if let Some(top) = top {
        if let Ok(rel) = abs.strip_prefix(&top) {
            let text = rel.to_string_lossy().replace('\\', "/");
            if !text.is_empty() {
                return text;
            }
        }
        if let (Ok(abs_c), Ok(top_c)) = (abs.canonicalize(), top.canonicalize()) {
            if let Ok(rel) = abs_c.strip_prefix(&top_c) {
                let text = rel.to_string_lossy().replace('\\', "/");
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    path.display().to_string()
}

fn build_format_config(
    cli: &Cli,
    project_config: &ProjectConfig,
    format: Format,
    file_path: Option<&Path>,
) -> FormatConfig {
    let format_key = format.config_key();
    let max_width = resolve_max_width(
        cli.max_width,
        project_config.max_width_for_path(format_key, file_path),
        file_path,
    );
    let neural_lang = cli
        .lang
        .clone()
        .or_else(|| project_config.lang.clone())
        .unwrap_or_else(|| "en".to_string());

    // CLI --clause-breaks wins when set; otherwise a path rule, then the
    // top-level key (default false).
    let clause_breaks = cli.clause_breaks || project_config.clause_breaks_for_path(file_path);
    let lowercase_starts =
        cli.lowercase_starts || project_config.lowercase_starts_for_path(format_key, file_path);

    FormatConfig {
        format,
        max_width,
        use_neural: cli.neural,
        neural_lang,
        neural_model_path: cli.model_path.clone(),
        extra_abbreviations: project_config.abbreviations_for_path(format_key, file_path),
        latex_verbatim_envs: project_config.latex_verbatim_envs(),
        latex_structure_envs: project_config.latex_structure_envs(),
        latex_verbatim_commands: project_config.latex_verbatim_commands(),
        use_pandoc: cli.resolve_use_pandoc_config(project_config.use_pandoc),
        #[cfg(feature = "pandoc")]
        pandoc_backend: cli
            .pandoc_backend
            .parse()
            .unwrap_or(snapper_fmt::parser::pandoc::PandocBackend::Cli),
        code: project_config.code.clone(),
        format_code: cli.format_code,
        clause_breaks,
        lowercase_starts,
        ..Default::default()
    }
}

fn resolve_format(
    cli_format: Option<Format>,
    path: Option<&Path>,
    project_config: &ProjectConfig,
) -> Result<Format> {
    if let Some(format) = cli_format {
        return Ok(format);
    }

    if let Some(path) = path {
        if let Some(name) = project_config
            .matching_rule(path)
            .and_then(|rule| rule.format.as_deref())
        {
            return Format::from_config_key(name).with_context(|| {
                format!(
                    "{}: unknown format {name:?} in a [[path]] rule",
                    path.display()
                )
            });
        }
        if let Some(detected) = Format::recognized_from_path(path) {
            return Ok(detected);
        }
        let hint = match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => format!("extension .{ext}"),
            None => "no file extension".to_string(),
        };
        anyhow::bail!(
            "{}: not a prose format ({hint}); pass --format or use .org/.tex/.md/.rst/.typ/.adoc/.txt",
            path.display()
        );
    }

    Ok(project_config
        .default_format
        .as_deref()
        .and_then(Format::recognized_from_extension)
        .unwrap_or(Format::Plaintext))
}

fn should_skip_path(path: &Path, cli: &Cli, project_config: &ProjectConfig) -> bool {
    (cli.check || cli.in_place) && project_config.file_skipped(path)
}

/// Resolve max_width: CLI flag > project config > editorconfig > 0 (unlimited).
fn resolve_max_width(
    cli_width: usize,
    config_width: Option<usize>,
    file_path: Option<&Path>,
) -> usize {
    // CLI flag takes priority (if explicitly set to non-zero)
    if cli_width > 0 {
        return cli_width;
    }

    // Project config
    if let Some(w) = config_width {
        if w > 0 {
            return w;
        }
    }

    // .editorconfig
    if let Some(path) = file_path {
        if let Ok(props) = ec4rs::properties_of(path) {
            if let Ok(ec4rs::property::MaxLineLen::Value(n)) =
                props.get::<ec4rs::property::MaxLineLen>()
            {
                return n;
            }
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_width_wins() {
        assert_eq!(resolve_max_width(80, Some(120), None), 80);
    }

    #[test]
    fn config_width_when_cli_zero() {
        assert_eq!(resolve_max_width(0, Some(120), None), 120);
    }

    #[test]
    fn both_zero_returns_zero() {
        assert_eq!(resolve_max_width(0, None, None), 0);
        assert_eq!(resolve_max_width(0, Some(0), None), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_canonicalize_failure_is_not_classified_as_outside() {
        let root = std::env::current_dir().unwrap();
        let dir = tempfile::tempdir_in(&root).unwrap();
        let link = dir.path().join("missing.md");
        std::os::unix::fs::symlink("no-such-target.md", &link).unwrap();
        let err = snapper_fmt::changed::touch_for_path("HEAD", &link).unwrap_err();
        assert!(
            !outside_repo(&err),
            "a canonicalize failure must not be skipped as outside, got {err:#}"
        );
        let text = format!("{err:#}");
        assert!(
            !text.contains("outside the git repository"),
            "the message must not call a resolve failure outside, got {text}"
        );
        assert!(
            text.contains("failed to resolve"),
            "a dangling symlink must be a resolve error, got {text}"
        );
    }

    #[test]
    fn resolve_color_respects_never_and_no_color_flag() {
        // no_color forces off regardless of ColorWhen
        assert!(!resolve_color(ColorWhen::Always, true));
        assert!(!resolve_color(ColorWhen::Auto, true));
        assert!(!resolve_color(ColorWhen::Never, false));
        // Always without no_color is always on (independent of TTY)
        assert!(resolve_color(ColorWhen::Always, false));
    }
}
