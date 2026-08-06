use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
#[cfg(any(feature = "cli", feature = "watch"))]
use glob::Pattern;
use serde::Deserialize;

/// Per-format overrides in .snapperrc.toml.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FormatOverrides {
    pub extra_abbreviations: Vec<String>,
    pub max_width: Option<usize>,
}

/// Per-language entry under the `[code]` table.
///
/// Each field is independent and optional:
/// - `line_comment`: marker that introduces a single-line comment (e.g. `//`).
/// - `block_comment`: opening and closing markers for a multi-line comment
///   (e.g. `["/*", "*/"]`). Stored as a fixed-arity pair.
/// - `formatter`: argv for an external formatter invoked via `--format-code`;
///   `formatter[0]` is the binary, the rest are its arguments. Stdin/stdout
///   carries block body.
/// - `string_delims`: quote characters that open and close a string, used to
///   tell a comment marker from the same characters inside a literal when no
///   grammar is available. Defaults to `"` and `'`.
/// - `escape`: character that escapes the next one inside a string. Defaults
///   to a backslash.
#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CodeLang {
    pub line_comment: Option<String>,
    pub block_comment: Option<[String; 2]>,
    pub formatter: Option<Vec<String>>,
    pub string_delims: Option<Vec<String>>,
    pub escape: Option<String>,
}

impl CodeLang {
    /// Quote characters for this language, falling back to the pair almost
    /// every language shares.
    pub(crate) fn quote_chars(&self) -> Vec<char> {
        match self.string_delims {
            Some(ref delims) => delims.iter().filter_map(|d| d.chars().next()).collect(),
            None => vec!['"', '\''],
        }
    }

    /// Escape character for this language.
    pub(crate) fn escape_char(&self) -> char {
        self.escape
            .as_ref()
            .and_then(|e| e.chars().next())
            .unwrap_or('\\')
    }
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
    /// Prefer soft breaks after independent-clause punctuation when wrapping.
    pub clause_breaks: Option<bool>,
    /// Default language for abbreviation sets.
    pub lang: Option<String>,

    /// Per-format overrides.
    pub org: Option<FormatOverrides>,
    pub latex: Option<FormatOverrides>,
    pub markdown: Option<FormatOverrides>,
    pub rst: Option<FormatOverrides>,
    pub plaintext: Option<FormatOverrides>,

    /// Per-language code-block reflow and formatter configuration.
    /// Key is the language identifier as it appears on the code fence
    /// (e.g. `rust`, `python`, `toml`). Missing languages mean the code
    /// block passes through unchanged.
    pub code: HashMap<String, CodeLang>,
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
            "rst" => self.rst.as_ref(),
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
            "rst" => self.rst.as_ref(),
            "plaintext" => self.plaintext.as_ref(),
            _ => None,
        };
        overrides.and_then(|ov| ov.max_width).or(self.max_width)
    }

    #[cfg(any(feature = "cli", feature = "watch"))]
    pub fn is_ignored(&self, path: &Path) -> bool {
        self.ignore_patterns.iter().any(|pattern| {
            Pattern::new(pattern).ok().is_some_and(|compiled| {
                compiled.matches_path(path)
                    || std::env::current_dir()
                        .ok()
                        .and_then(|cwd| path.strip_prefix(&cwd).ok())
                        .is_some_and(|relative| compiled.matches_path(relative))
                    || path
                        .file_name()
                        .is_some_and(|name| compiled.matches_path(Path::new(name)))
            })
        })
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
clause_breaks = true
lang = "de"
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.extra_abbreviations, vec!["Dept", "Univ", "Corp"]);
        assert_eq!(config.ignore_patterns, vec!["*.bib", "*.cls"]);
        assert_eq!(config.default_format, Some("org".to_string()));
        assert_eq!(config.max_width, Some(80));
        assert_eq!(config.clause_breaks, Some(true));
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

    #[test]
    fn parse_code_table_seven_seed_languages() {
        // The shape `snapper init` writes: seven languages with mixed
        // line_comment / block_comment / formatter fields. The python
        // triple-quoted markers need an extra `#` on the raw delimiter
        // so the inner escaped quotes lex.
        let toml = r##"
[code.rust]
line_comment = "//"
block_comment = ["/*", "*/"]
formatter = ["rustfmt", "--edition", "2024"]

[code.python]
line_comment = "#"
block_comment = ["\"\"\"", "\"\"\""]
formatter = ["ruff", "format", "-"]

[code.toml]
line_comment = "#"
formatter = ["taplo", "format", "-"]

[code.lua]
line_comment = "--"
block_comment = ["--[[", "]]"]

[code.lisp]
line_comment = ";"

[code.html]
block_comment = ["<!--", "-->"]

[code.javascript]
line_comment = "//"
block_comment = ["/*", "*/"]
formatter = ["prettier", "--stdin-filepath", "src.js"]
"##;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.code.len(), 7);
        let rust = config.code.get("rust").expect("rust entry present");
        assert_eq!(rust.line_comment.as_deref(), Some("//"));
        assert_eq!(
            rust.block_comment.as_ref(),
            Some(&["/*".to_string(), "*/".to_string()])
        );
        assert_eq!(
            rust.formatter.as_deref(),
            Some(
                &[
                    "rustfmt".to_string(),
                    "--edition".to_string(),
                    "2024".to_string()
                ][..]
            )
        );
        // lisp has only line_comment; missing fields stay None (no panic).
        let lisp = config.code.get("lisp").expect("lisp entry present");
        assert_eq!(lisp.line_comment.as_deref(), Some(";"));
        assert!(lisp.block_comment.is_none());
        assert!(lisp.formatter.is_none());
        // html has only block_comment.
        let html = config.code.get("html").expect("html entry present");
        assert!(html.line_comment.is_none());
        assert_eq!(
            html.block_comment.as_ref(),
            Some(&["<!--".to_string(), "-->".to_string()])
        );
        assert!(html.formatter.is_none());
    }

    #[test]
    fn parse_rst_overrides() {
        let toml = r#"
[rst]
extra_abbreviations = ["Fig"]
max_width = 72
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.max_width_for_format("rst"), Some(72));
        assert_eq!(config.abbreviations_for_format("rst"), vec!["Fig"]);
    }
}
