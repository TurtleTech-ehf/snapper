use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

/// Per-format overrides in .snapperrc.toml.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FormatOverrides {
    pub extra_abbreviations: Vec<String>,
    pub max_width: Option<usize>,
}

/// Per-project configuration loaded from `.snapperrc.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// Additional abbreviations that should not trigger sentence breaks.
    pub extra_abbreviations: Vec<String>,
    /// File patterns to ignore (glob syntax).
    #[serde(alias = "ignore")]
    pub ignore_patterns: Vec<String>,
    /// Default format override.
    #[serde(alias = "format")]
    pub default_format: Option<String>,
    /// Default max width.
    pub max_width: Option<usize>,
    /// Default language for abbreviation sets.
    pub lang: Option<String>,

    /// Per-format overrides.
    pub org: Option<FormatOverrides>,
    pub latex: Option<FormatOverrides>,
    pub markdown: Option<FormatOverrides>,
    pub plaintext: Option<FormatOverrides>,
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
        let config: ProjectConfig = toml::from_str(toml_str)?;
        Ok(config)
    }

    /// Get the config file path if explicitly provided, otherwise search.
    pub fn resolve(explicit_path: Option<&Path>) -> Result<Self> {
        if let Some(path) = explicit_path {
            Self::load(path)
        } else {
            let cwd = std::env::current_dir()?;
            Self::find_and_load(&cwd)
        }
    }

    /// Get merged extra_abbreviations for a specific format, combining
    /// top-level abbreviations with per-format overrides.
    pub fn abbreviations_for_format(&self, format: &str) -> Vec<String> {
        let mut abbrevs = self.extra_abbreviations.clone();
        let overrides = match format {
            "org" => self.org.as_ref(),
            "latex" => self.latex.as_ref(),
            "markdown" => self.markdown.as_ref(),
            "plaintext" => self.plaintext.as_ref(),
            _ => None,
        };
        if let Some(ov) = overrides {
            abbrevs.extend(ov.extra_abbreviations.iter().cloned());
        }
        abbrevs
    }

    /// Get max_width for a specific format (per-format overrides top-level).
    pub fn max_width_for_format(&self, format: &str) -> Option<usize> {
        let overrides = match format {
            "org" => self.org.as_ref(),
            "latex" => self.latex.as_ref(),
            "markdown" => self.markdown.as_ref(),
            "plaintext" => self.plaintext.as_ref(),
            _ => None,
        };
        overrides.and_then(|ov| ov.max_width).or(self.max_width)
    }
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
lang = "de"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.extra_abbreviations, vec!["Dept", "Univ", "Corp"]);
        assert_eq!(config.ignore_patterns, vec!["*.bib", "*.cls"]);
        assert_eq!(config.default_format, Some("org".to_string()));
        assert_eq!(config.max_width, Some(80));
        assert_eq!(config.lang, Some("de".to_string()));
    }

    #[test]
    fn parse_comments_and_blanks() {
        let toml = "# comment\n\nextra_abbreviations = [\"Fig\"]\n";
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.extra_abbreviations, vec!["Fig"]);
    }

    #[test]
    fn parse_per_format_overrides() {
        let toml = r#"
extra_abbreviations = ["Global"]
max_width = 80

[org]
extra_abbreviations = ["PROPERTIES", "DEADLINE"]

[latex]
extra_abbreviations = ["Thm", "Lem"]
max_width = 100
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        let org_abbrevs = config.abbreviations_for_format("org");
        assert!(org_abbrevs.contains(&"Global".to_string()));
        assert!(org_abbrevs.contains(&"PROPERTIES".to_string()));
        assert_eq!(config.max_width_for_format("org"), Some(80));
        assert_eq!(config.max_width_for_format("latex"), Some(100));
        assert_eq!(config.max_width_for_format("plaintext"), Some(80));
    }
}
