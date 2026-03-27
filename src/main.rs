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
            print_diff("<stdin>", &input, &output);
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
                    print_diff(&path.display().to_string(), &input, &output);
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

/// Print a unified diff between original and formatted text.
fn print_diff(filename: &str, original: &str, formatted: &str) {
    let orig_lines: Vec<&str> = original.lines().collect();
    let fmt_lines: Vec<&str> = formatted.lines().collect();

    println!("--- a/{filename}");
    println!("+++ b/{filename}");

    // Simple line-by-line diff with context
    let max_len = orig_lines.len().max(fmt_lines.len());
    let mut i = 0;
    while i < max_len {
        // Find next difference
        let orig = orig_lines.get(i).copied().unwrap_or("");
        let fmt = fmt_lines.get(i).copied().unwrap_or("");
        if orig == fmt {
            i += 1;
            continue;
        }

        // Find extent of change
        let start = i;
        let mut orig_end = i;
        let mut fmt_end = i;

        // Advance past differing lines
        while orig_end < orig_lines.len() || fmt_end < fmt_lines.len() {
            let o = orig_lines.get(orig_end).copied().unwrap_or("");
            let f = fmt_lines.get(fmt_end).copied().unwrap_or("");
            if o == f && orig_end > start {
                break;
            }
            if orig_end < orig_lines.len() {
                orig_end += 1;
            }
            if fmt_end < fmt_lines.len() {
                fmt_end += 1;
            }
        }

        // Print hunk header
        let ctx_start = start.saturating_sub(2);
        println!(
            "@@ -{},{} +{},{} @@",
            ctx_start + 1,
            orig_end - ctx_start + 1,
            ctx_start + 1,
            fmt_end - ctx_start + 1
        );

        // Context before
        for j in ctx_start..start {
            if let Some(line) = orig_lines.get(j) {
                println!(" {line}");
            }
        }

        // Removed lines
        for j in start..orig_end {
            if let Some(line) = orig_lines.get(j) {
                println!("-{line}");
            }
        }

        // Added lines
        for j in start..fmt_end {
            if let Some(line) = fmt_lines.get(j) {
                println!("+{line}");
            }
        }

        i = orig_end.max(fmt_end);
    }
}
