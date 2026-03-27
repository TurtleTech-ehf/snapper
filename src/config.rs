use std::path::{Path, PathBuf};

use anyhow::Result;

/// Per-project configuration loaded from `.snapperrc.toml`.
#[derive(Debug, Default)]
pub struct ProjectConfig {
    /// Additional abbreviations that should not trigger sentence breaks.
    pub extra_abbreviations: Vec<String>,
    /// File patterns to ignore (glob syntax).
    pub ignore_patterns: Vec<String>,
    /// Default format override.
    pub default_format: Option<String>,
    /// Default max width.
    pub max_width: Option<usize>,
}

impl ProjectConfig {
    /// Search for `.snapperrc.toml` starting from `start_dir` and walking up
    /// to the filesystem root. Returns the default config if none found.
    pub fn find_and_load(start_dir: &Path) -> Result<Self> {
        let mut dir = start_dir.to_path_buf();
        loop {
            let candidate = dir.join(".snapperrc.toml");
            if candidate.is_file() {
                return Self::load(&candidate);
            }
            if !dir.pop() {
                break;
            }
        }
        Ok(Self::default())
    }

    /// Load config from a specific path.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Self::parse(&contents)
    }

    fn parse(toml_str: &str) -> Result<Self> {
        let mut config = Self::default();

        // Simple TOML parsing without pulling in a full TOML crate.
        // We only support flat key = value and key = ["array"] forms.
        for line in toml_str.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "extra_abbreviations" => {
                        config.extra_abbreviations = parse_string_array(value);
                    }
                    "ignore_patterns" | "ignore" => {
                        config.ignore_patterns = parse_string_array(value);
                    }
                    "default_format" | "format" => {
                        config.default_format =
                            Some(value.trim_matches('"').trim_matches('\'').to_string());
                    }
                    "max_width" => {
                        if let Ok(w) = value.parse::<usize>() {
                            config.max_width = Some(w);
                        }
                    }
                    _ => {} // Ignore unknown keys
                }
            }
        }

        Ok(config)
    }

    /// Get the config file path if explicitly provided, otherwise search.
    pub fn resolve(explicit_path: Option<&PathBuf>) -> Result<Self> {
        if let Some(path) = explicit_path {
            Self::load(path)
        } else {
            let cwd = std::env::current_dir()?;
            Self::find_and_load(&cwd)
        }
    }
}

/// Parse a TOML-style string array: ["foo", "bar", "baz"]
fn parse_string_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return vec![];
    }
    let inner = &s[1..s.len() - 1];
    inner
        .split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_config() {
        let config = ProjectConfig::parse("").unwrap();
        assert!(config.extra_abbreviations.is_empty());
        assert!(config.ignore_patterns.is_empty());
        assert!(config.default_format.is_none());
        assert!(config.max_width.is_none());
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
# Project-specific snapper config
extra_abbreviations = ["Dept", "Univ", "Corp"]
ignore = ["*.bib", "*.cls"]
format = "org"
max_width = 80
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.extra_abbreviations, vec!["Dept", "Univ", "Corp"]);
        assert_eq!(config.ignore_patterns, vec!["*.bib", "*.cls"]);
        assert_eq!(config.default_format, Some("org".to_string()));
        assert_eq!(config.max_width, Some(80));
    }

    #[test]
    fn parse_comments_and_blanks() {
        let toml = "# comment\n\nextra_abbreviations = [\"Fig\"]\n";
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.extra_abbreviations, vec!["Fig"]);
    }
}
