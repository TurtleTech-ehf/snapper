use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process;

use anyhow::{Context, Result};
use clap::Parser;
use rayon::prelude::*;

use snapper_fmt::cli::{Cli, Commands, OutputFormat, parse_range};
use snapper_fmt::config::ProjectConfig;
use snapper_fmt::format::Format;
use snapper_fmt::output::{CheckResult, output_json, output_sarif};
use snapper_fmt::{FormatConfig, format_range, format_text};

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
                let result = snapper_fmt::sdiff::sentence_diff(old, new, fmt, !no_color)?;
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
                let has_diff = snapper_fmt::git_diff::run_git_diff(git_ref, files, fmt, !no_color)?;
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
            Commands::Watch { patterns, format } => {
                let fmt = format.map(Format::from_arg);
                return snapper_fmt::watch::run_watch(patterns, fmt);
            }
        }
    }

    let project_config = ProjectConfig::resolve(cli.config.as_ref()).unwrap_or_default();

    if cli.files.is_empty() {
        // Read from stdin
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .context("failed to read stdin")?;

        // Format detection: --format > --stdin-filepath > plaintext
        let format = cli
            .format
            .map(Format::from_arg)
            .or_else(|| cli.stdin_filepath.as_ref().map(|p| Format::from_path(p)))
            .unwrap_or(Format::Plaintext);

        let max_width = resolve_max_width(
            cli.max_width,
            project_config.max_width,
            cli.stdin_filepath.as_deref(),
        );

        let config = FormatConfig {
            format,
            max_width,
            use_neural: cli.neural,
            neural_lang: cli.lang.clone(),
            neural_model_path: cli.model_path.clone(),
            extra_abbreviations: project_config.extra_abbreviations.clone(),
        };

        let output = if let Some(ref range_str) = cli.range {
            let (start, end) =
                parse_range(range_str).context("invalid range format, expected START:END")?;
            format_range(&input, &config, start, end)?
        } else {
            format_text(&input, &config)?
        };

        if cli.diff {
            snapper_fmt::diff::print_diff("<stdin>", &input, &output);
        } else if let Some(ref path) = cli.output {
            fs::write(path, &output)
                .with_context(|| format!("failed to write {}", path.display()))?;
        } else {
            print!("{output}");
        }
    } else {
        // Process files (parallel when multiple files + in-place or check mode)
        let use_parallel = cli.files.len() > 1 && (cli.in_place || cli.check || cli.diff);

        let results: Vec<(String, String, String)> = if use_parallel {
            cli.files
                .par_iter()
                .map(|path| process_file(path, &cli, &project_config))
                .collect::<Result<Vec<_>>>()?
        } else {
            cli.files
                .iter()
                .map(|path| process_file(path, &cli, &project_config))
                .collect::<Result<Vec<_>>>()?
        };

        let mut any_changed = false;
        let mut check_results: Vec<CheckResult> = Vec::new();

        for (path_str, input, output) in &results {
            if cli.diff {
                if output != input {
                    snapper_fmt::diff::print_diff(path_str, input, output);
                    any_changed = true;
                }
            } else if cli.check {
                if output != input {
                    match cli.output_format {
                        OutputFormat::Text => eprintln!("would reformat: {path_str}"),
                        _ => check_results.push(CheckResult {
                            file: path_str.clone(),
                            original_lines: input.lines().count(),
                            formatted_lines: output.lines().count(),
                        }),
                    }
                    any_changed = true;
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

        // Structured output for check mode
        if cli.check && !check_results.is_empty() {
            match cli.output_format {
                OutputFormat::Json => output_json(&check_results),
                OutputFormat::Sarif => output_sarif(&check_results),
                OutputFormat::Text => {} // already printed above
            }
        }

        if (cli.check || cli.diff) && any_changed {
            process::exit(1);
        }
    }

    Ok(())
}

/// Process a single file: read, format, return (path, input, output).
fn process_file(
    path: &Path,
    cli: &Cli,
    project_config: &ProjectConfig,
) -> Result<(String, String, String)> {
    let path_str = path.display().to_string();
    let input =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    let format = cli
        .format
        .map(Format::from_arg)
        .unwrap_or_else(|| Format::from_path(path));

    let max_width = resolve_max_width(cli.max_width, project_config.max_width, Some(path));

    let config = FormatConfig {
        format,
        max_width,
        use_neural: cli.neural,
        neural_lang: cli.lang.clone(),
        neural_model_path: cli.model_path.clone(),
        extra_abbreviations: project_config.extra_abbreviations.clone(),
    };

    let output = if let Some(ref range_str) = cli.range {
        let (start, end) =
            parse_range(range_str).context("invalid range format, expected START:END")?;
        format_range(&input, &config, start, end)?
    } else {
        format_text(&input, &config)?
    };

    Ok((path_str, input, output))
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
