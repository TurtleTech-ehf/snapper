use std::collections::HashMap;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;
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
                let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
                rt.block_on(snapper_fmt::lsp::run_lsp());
                return Ok(());
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
                let fmt = format.map(Format::from_arg);
                return snapper_fmt::watch::run_watch(patterns, fmt, cli.config.as_deref());
            }
        }
    }

    let project_config = ProjectConfig::resolve(cli.config.as_deref()).unwrap_or_default();

    if cli.files.is_empty() {
        // Read from stdin
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .context("failed to read stdin")?;

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

        if cli.diff {
            let color = resolve_color(cli.color, false);
            snapper_fmt::diff::print_diff("<stdin>", &input, &output, color);
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
        let use_parallel = cli.files.len() > 1;
        let mut splitter_cache: HashMap<SplitterKey, Arc<dyn SentenceSplitter>> = HashMap::new();

        let paths: Vec<&Path> = cli
            .files
            .iter()
            .map(Path::new)
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

        let results: Vec<(String, String, String)> = if use_parallel {
            paths
                .par_iter()
                .map(|path| process_file(path, &cli, &project_config, &cache))
                .collect::<Result<Vec<_>>>()?
        } else {
            paths
                .iter()
                .map(|path| process_file(path, &cli, &project_config, &cache))
                .collect::<Result<Vec<_>>>()?
        };

        let mut any_changed = false;
        let mut check_failed = false;
        let mut check_results: Vec<CheckResult> = Vec::new();

        let color = resolve_color(cli.color, false);
        for (path_str, input, output) in &results {
            if cli.diff {
                if output != input {
                    snapper_fmt::diff::print_diff(path_str, input, output, color);
                    any_changed = true;
                }
            } else if cli.check {
                let path = Path::new(path_str);
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
                let threshold =
                    resolve_long_threshold(config.max_width, project_config.long_threshold);
                let diagnostics = collect_diagnostics(input, format, splitter.as_ref(), threshold);
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
                    fs::write(path_str, output)
                        .with_context(|| format!("failed to write {path_str}"))?;
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
    }

    Ok(())
}

/// Key for reusing sentence splitters across files with the same split policy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SplitterKey {
    format: Format,
    use_neural: bool,
    neural_lang: String,
    neural_model: Option<std::path::PathBuf>,
    extras: Vec<String>,
}

impl SplitterKey {
    fn from_config(config: &FormatConfig) -> Self {
        Self {
            format: config.format,
            use_neural: config.use_neural,
            neural_lang: config.neural_lang.clone(),
            neural_model: config.neural_model_path.clone(),
            extras: config.extra_abbreviations.clone(),
        }
    }
}

/// Process a single file: read, format, return (path, input, output).
fn process_file(
    path: &Path,
    cli: &Cli,
    project_config: &ProjectConfig,
    splitter_cache: &HashMap<SplitterKey, Arc<dyn SentenceSplitter>>,
) -> Result<(String, String, String)> {
    let path_str = path.display().to_string();
    let input =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    let format = resolve_format(cli.format.map(Format::from_arg), Some(path), project_config)?;
    let config = build_format_config(cli, project_config, format, Some(path));
    let key = SplitterKey::from_config(&config);
    let splitter = splitter_cache
        .get(&key)
        .ok_or_else(|| anyhow::anyhow!("splitter cache miss for {path_str}"))?;

    let output = if let Some(ref range_str) = cli.range {
        let (start, end) =
            parse_range(range_str).context("invalid range format, expected START:END")?;
        // Range path still uses config; splitter reuse is best-effort via full-file path.
        let _ = splitter;
        format_range(&input, &config, start, end)?
    } else {
        format_text_with_splitter(&input, &config, splitter.as_ref())?
    };

    Ok((path_str, input, output))
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
        project_config.max_width_for_format(format_key),
        file_path,
    );
    let neural_lang = cli
        .lang
        .clone()
        .or_else(|| project_config.lang.clone())
        .unwrap_or_else(|| "en".to_string());

    // CLI --clause-breaks wins when set; otherwise project config (default false).
    let clause_breaks = cli.clause_breaks || project_config.clause_breaks.unwrap_or(false);

    FormatConfig {
        format,
        max_width,
        use_neural: cli.neural,
        neural_lang,
        neural_model_path: cli.model_path.clone(),
        extra_abbreviations: project_config.abbreviations_for_format(format_key),
        use_pandoc: cli.use_pandoc,
        #[cfg(feature = "pandoc")]
        pandoc_backend: cli
            .pandoc_backend
            .parse()
            .unwrap_or(snapper_fmt::parser::pandoc::PandocBackend::Cli),
        code: project_config.code.clone(),
        format_code: cli.format_code,
        clause_breaks,
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
        if let Some(detected) = Format::recognized_from_path(path) {
            return Ok(detected);
        }
        let hint = match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => format!("extension .{ext}"),
            None => "no file extension".to_string(),
        };
        anyhow::bail!(
            "{}: not a prose format ({hint}); pass --format or use .org/.tex/.md/.rst/.txt",
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
    (cli.check || cli.in_place) && project_config.is_ignored(path)
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
