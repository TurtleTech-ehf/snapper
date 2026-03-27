use std::fs;
use std::io::{self, Read};
use std::process;

use anyhow::{Context, Result};
use clap::Parser;

use snapper_fmt::cli::Cli;
use snapper_fmt::config::ProjectConfig;
use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let project_config = ProjectConfig::resolve(cli.config.as_ref()).unwrap_or_default();

    if cli.files.is_empty() {
        // Read from stdin
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .context("failed to read stdin")?;

        let format = cli
            .format
            .map(Format::from_arg)
            .unwrap_or(Format::Plaintext);

        let config = FormatConfig {
            format,
            max_width: cli.max_width,
            use_neural: cli.neural,
            extra_abbreviations: project_config.extra_abbreviations.clone(),
        };

        let output = format_text(&input, &config)?;

        if cli.diff {
            snapper_fmt::diff::print_diff("<stdin>", &input, &output);
        } else if let Some(ref path) = cli.output {
            fs::write(path, &output)
                .with_context(|| format!("failed to write {}", path.display()))?;
        } else {
            print!("{output}");
        }
    } else {
        let mut any_changed = false;

        for path in &cli.files {
            let input = fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;

            let format = cli
                .format
                .map(Format::from_arg)
                .unwrap_or_else(|| Format::from_path(path));

            let config = FormatConfig {
                format,
                max_width: cli.max_width,
                use_neural: cli.neural,
                extra_abbreviations: project_config.extra_abbreviations.clone(),
            };

            let output = format_text(&input, &config)?;

            if cli.diff {
                if output != input {
                    snapper_fmt::diff::print_diff(&path.display().to_string(), &input, &output);
                    any_changed = true;
                }
            } else if cli.check {
                if output != input {
                    eprintln!("would reformat: {}", path.display());
                    any_changed = true;
                }
            } else if cli.in_place {
                if output != input {
                    fs::write(path, &output)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                }
            } else if let Some(ref out_path) = cli.output {
                fs::write(out_path, &output)
                    .with_context(|| format!("failed to write {}", out_path.display()))?;
            } else {
                print!("{output}");
            }
        }

        if (cli.check || cli.diff) && any_changed {
            process::exit(1);
        }
    }

    Ok(())
}

