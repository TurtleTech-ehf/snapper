use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Detect which prose formats exist in the current directory tree.
fn detect_formats(dir: &Path) -> Vec<&'static str> {
    let mut formats = Vec::new();
    let check = |ext: &str| -> bool {
        walkdir(dir)
            .into_iter()
            .any(|e| e.path().extension().and_then(|e| e.to_str()) == Some(ext))
    };
    // Simple recursive check using std::fs
    fn walkdir(dir: &Path) -> Vec<fs::DirEntry> {
        let mut entries = Vec::new();
        if let Ok(rd) = fs::read_dir(dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && !path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                {
                    entries.extend(walkdir(&path));
                } else {
                    entries.push(entry);
                }
            }
        }
        entries
    }

    if check("org") {
        formats.push("org");
    }
    if check("tex") || check("latex") {
        formats.push("latex");
    }
    if check("md") || check("markdown") {
        formats.push("markdown");
    }
    formats
}

/// Generate .snapperrc.toml content.
fn generate_config(formats: &[&str]) -> String {
    let default_format = formats.first().copied().unwrap_or("plaintext");
    format!(
        r#"# snapper project configuration
# https://snapper.turtletech.us/docs/reference/config/

# Extra abbreviations (merged with built-in list)
# extra_abbreviations = ["GROMACS", "LAMMPS", "DFT"]

# File patterns to ignore
# ignore = ["*.bib", "*.cls", "*.sty"]

# Default format (auto-detected from extension if omitted)
format = "{default_format}"

# Maximum line width (0 = unlimited)
max_width = 0
"#
    )
}

/// Generate .gitattributes entries.
fn generate_gitattributes(formats: &[&str]) -> String {
    let mut lines = String::from("# snapper semantic line break filter\n");
    for fmt in formats {
        let ext = match *fmt {
            "org" => "*.org",
            "latex" => "*.tex",
            "markdown" => "*.md",
            _ => continue,
        };
        lines.push_str(&format!("{ext} filter=snapper\n"));
    }
    lines
}

/// Generate pre-commit config snippet.
fn generate_precommit() -> &'static str {
    r#"# Add to .pre-commit-config.yaml:
- repo: https://github.com/TurtleTech-ehf/snapper
  rev: v0.1.0
  hooks:
    - id: snapper
"#
}

/// Generate Apheleia elisp snippet.
fn generate_apheleia(formats: &[&str]) -> String {
    let mut s = String::from(";; Add to your Emacs config:\n(with-eval-after-load 'apheleia\n");
    s.push_str("  (push '(snapper . (\"snapper\")) apheleia-formatters)\n");
    for fmt in formats {
        let mode = match *fmt {
            "org" => "org-mode",
            "latex" => "latex-mode",
            "markdown" => "markdown-mode",
            _ => continue,
        };
        s.push_str(&format!(
            "  (push '({mode} . snapper) apheleia-mode-alist)\n"
        ));
    }
    s.push_str(")\n");
    s
}

/// Run the init command.
pub fn run_init(dry_run: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let formats = detect_formats(&cwd);

    eprintln!(
        "Detected formats: {}",
        if formats.is_empty() {
            "none (will use plaintext defaults)".to_string()
        } else {
            formats.join(", ")
        }
    );

    // .snapperrc.toml
    let config_content = generate_config(&formats);
    let config_path = cwd.join(".snapperrc.toml");
    if config_path.exists() {
        eprintln!("  .snapperrc.toml already exists, skipping");
    } else if dry_run {
        eprintln!("\n--- .snapperrc.toml ---");
        eprint!("{config_content}");
    } else {
        fs::write(&config_path, &config_content).context("failed to write .snapperrc.toml")?;
        eprintln!("  Created .snapperrc.toml");
    }

    // .gitattributes
    if !formats.is_empty() {
        let ga_content = generate_gitattributes(&formats);
        let ga_path = cwd.join(".gitattributes");
        if dry_run {
            eprintln!("\n--- .gitattributes (append) ---");
            eprint!("{ga_content}");
        } else if ga_path.exists() {
            let existing = fs::read_to_string(&ga_path)?;
            if !existing.contains("filter=snapper") {
                fs::write(&ga_path, format!("{existing}\n{ga_content}"))
                    .context("failed to append .gitattributes")?;
                eprintln!("  Appended to .gitattributes");
            } else {
                eprintln!("  .gitattributes already has snapper filter, skipping");
            }
        } else {
            fs::write(&ga_path, &ga_content).context("failed to write .gitattributes")?;
            eprintln!("  Created .gitattributes");
        }
    }

    // Print pre-commit and Apheleia snippets
    eprintln!("\n{}", generate_precommit());
    eprintln!("{}", generate_apheleia(&formats));

    // Git filter setup reminder
    eprintln!("To enable the git smudge/clean filter, run:");
    eprintln!("  git config filter.snapper.clean \"snapper\"");
    eprintln!("  git config filter.snapper.smudge cat");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_config_with_org() {
        let config = generate_config(&["org"]);
        assert!(config.contains("format = \"org\""));
        assert!(config.contains("max_width = 0"));
    }

    #[test]
    fn generate_config_empty_defaults_to_plaintext() {
        let config = generate_config(&[]);
        assert!(config.contains("format = \"plaintext\""));
    }

    #[test]
    fn generate_gitattributes_multiple_formats() {
        let ga = generate_gitattributes(&["org", "latex", "markdown"]);
        assert!(ga.contains("*.org filter=snapper"));
        assert!(ga.contains("*.tex filter=snapper"));
        assert!(ga.contains("*.md filter=snapper"));
    }

    #[test]
    fn generate_gitattributes_empty() {
        let ga = generate_gitattributes(&[]);
        assert!(ga.contains("# snapper"));
        assert!(!ga.contains("filter=snapper"));
    }
}
