use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{Result, bail};
#[cfg(any(feature = "cli", feature = "watch"))]
use glob::Pattern;
use serde::Deserialize;

/// Per-format overrides in .snapperrc.toml.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormatOverrides {
    pub extra_abbreviations: Vec<String>,
    pub max_width: Option<usize>,
    /// Extra LaTeX environments treated as code (no reflow).
    /// Meaningful under `[latex]` only. Missing or empty keeps the built-in
    /// minted/minted*/lstlisting/lstlisting*/verbatim/comment,
    /// filecontents/filecontents*/filecontentsdef plus leftover
    /// filecontentsgdef/filecontentsdefmacro/filecontentsgdefmacro/
    /// filecontentshere and starred twins filecontentsdef*/
    /// filecontentsgdef*/filecontentshere* plus aliases
    /// filecontentsdefstarred/filecontentsgdefstarred/filecontentsherestarred, scontents.sty
    /// scontents/verbatimsc, tree-sitter
    /// trivia, Overleaf Verbatim/boxedverbatim/tcblisting/tcblisting*/codeexample/
    /// tcbverbatimwrite/tcbwritetemp/tcboutputlisting/tcbexternal/
    /// dispExample/dispExample*/dispListing/dispListing*,
    /// fancyvrb BVerbatim/LVerbatim/SaveVerbatim/VerbatimOut/VerbatimWrite/
    /// VerbatimBuffer and Verbatim*/BVerbatim*/LVerbatim*, fvextra VerbEnv,
    /// verbments pyglist, texments/pygmentex pygmented,
    /// moreverb boxedverbatim/verbatimtab/verbatimwrite/listing/listingcont/listing*/listingcont*,
    /// leftover sverb verbwrite/verbwrite*/ignore/demo/demo*,
    /// leftover tcolorbox extcolorbox/extikzpicture,
    /// standard alltt, spverbatim, piton.sty Piton,
    /// tree-sitter sagesilent/sageblock, sagetex sageverbatim/sageexample/
    /// sagecommandline,
    /// pythontex pycode/pycode*/pyblock/pyblock*/pyverbatim/pyverbatim*/
    /// pyconsole/pyconsole*/pygments plus sympy and pylab families and
    /// starred twins, leftover default-family pyconcode/pyconverbatim/
    /// pysub/pyconsub/sympycon*/pylabcon*/pythontexcustomcode, option-family
    /// usefamily leftovers (rubycode representative; ruby/rb/julia/
    /// juliacon/jl/matlab/octave/bash/sage/rust/rs/R/Rcon/perl/pl/
    /// perlsix/psix/javascript/js), and
    /// pythonhighlight python, pyluatex pythonq/pythonrepl,
    /// showexpl LTXexample, luamplib mplibcode, luacode.sty leftover
    /// luaexec, and codehigh.sty codehigh/demohigh plus starred twins;
    /// entries are added to it.
    pub verbatim_envs: Vec<String>,
    /// Extra LaTeX environments treated as structure (no reflow).
    /// Meaningful under `[latex]` only. Missing or empty keeps
    /// `NON_PROSE_ENVS`; entries are added to that list.
    pub structure_envs: Vec<String>,
    /// Extra LaTeX command names tokenized like `\verb` before split.
    /// Meaningful under `[latex]` only. Missing or empty keeps
    /// verb/lstinline/lstinputlisting/spverb/mintinline/inputminted/mint/Verb/SaveVerb/VerbatimInput/piton/verbatiminput/PitonInputFile/PitonInputFileT/PitonInputFileF/PitonInputFileTF/tcbinputlisting/tcboxverb/inputpy/inputpycon/inputpylab/inputpylabcon/inputsympy/inputsympycon/py/pyc/pys/pyb/pyv/pycon and twins/sympy/pylab and twins/usefamily ruby/rb/julia/jl/matlab/octave/bash/sage/rust/rs/R/perl/pl/perlsix/psix/javascript/js and twins/pyq/pycq/pyfile/pyfileq/pyfilerepl/inputpygments/pygment/pythontexcustomc/inputpython/inputpythonfile/CatchFileBetweenTags/CatchFileBetweenDelims/ExecuteMetaData/CatchFileDef/CatchFileEdef/listinginput/verbatimtabinput/verbatimwrite/listingcont/sageinput/sageplot/sagestr/pyth/Scontents/typestored/getstored/mergesc/meaningsc/foreachsc/setupsc/countsc/cleanseqsc/UseVerb/UseVerbatim/LUseVerbatim/BUseVerbatim/DefineShortVerb/UndefineShortVerb/EscVerb/inputsc/VerbatimInsertBuffer/VerbatimClearBuffer/InsertBuffer/IterateBuffer/ClearBuffer/WriteBuffer/lstset/lstdefinestyle/lstMakeShortInline/lstDeleteShortInline/lstnewenvironment/verbinput/verbwrite.
    pub verbatim_commands: Vec<String>,
    /// Open a sentence at a lowercase word after a terminator.
    /// Overrides the top-level `lowercase_starts` for this format.
    pub lowercase_starts: Option<bool>,
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
#[serde(default, deny_unknown_fields)]
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

/// One `[[path]]` rule in `.snapperrc.toml`.
///
/// Rules are ordered. The first rule whose glob matches the file supplies
/// every key it sets. Later rules are not consulted. A pattern with no `/`
/// also matches the file name, so `*.md` covers `docs/a.md`.
#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PathRule {
    /// One glob, relative to the directory that holds the config file.
    pub glob: Option<String>,
    /// More globs for the same overrides.
    pub globs: Vec<String>,
    pub max_width: Option<usize>,
    pub clause_breaks: Option<bool>,
    pub lowercase_starts: Option<bool>,
    pub extra_abbreviations: Vec<String>,
    /// Format name (`org`, `latex`, `markdown`, `rst`, `typst`, `asciidoc`, `plaintext`).
    /// Used when the CLI did not pass `--format`.
    pub format: Option<String>,
    /// Skip the file during `--check` and `--in-place`, same as `ignore`.
    pub ignore: bool,
    pub long_threshold: Option<usize>,
}

impl PathRule {
    fn patterns(&self) -> impl Iterator<Item = &str> {
        self.glob
            .as_deref()
            .into_iter()
            .chain(self.globs.iter().map(String::as_str))
    }
}

/// Per-project configuration loaded from `.snapperrc.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
    /// Character threshold for advisory `long` diagnostics when `max_width` is
    /// unset. Defaults to 120 when omitted.
    pub long_threshold: Option<usize>,
    /// Prefer soft breaks after independent-clause punctuation. With
    /// `max_width = 0` this still inserts a newline after each mark that
    /// is already followed by whitespace.
    pub clause_breaks: Option<bool>,
    /// Open a sentence at a lowercase word after a terminator and a space.
    /// Per-format sections override this. Default false.
    pub lowercase_starts: Option<bool>,
    /// When true, the CLI takes the pandoc path without `--use-pandoc`.
    /// `--native` still forces the native parsers. Default false.
    pub use_pandoc: Option<bool>,
    /// Default language for abbreviation sets.
    pub lang: Option<String>,

    /// Per-format overrides.
    pub org: Option<FormatOverrides>,
    pub latex: Option<FormatOverrides>,
    pub markdown: Option<FormatOverrides>,
    pub rst: Option<FormatOverrides>,
    pub plaintext: Option<FormatOverrides>,
    pub typst: Option<FormatOverrides>,
    pub asciidoc: Option<FormatOverrides>,

    /// Ordered path rules. The first match wins. See [`PathRule`].
    #[serde(default, rename = "path")]
    pub paths: Vec<PathRule>,

    /// Directory that held the config file. Set by [`ProjectConfig::load`].
    /// Glob matching is relative to this directory. Not read from TOML.
    #[serde(skip)]
    pub root: Option<PathBuf>,

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
    ///
    /// `root` becomes the config file's directory so `[[path]]` globs resolve
    /// against that tree rather than whatever the process later uses as cwd.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let mut config = Self::parse(&contents)?;
        config.root = path.parent().map(|parent| {
            if parent.as_os_str().is_empty() {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            } else if parent.is_absolute() {
                parent.to_path_buf()
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(parent)
            }
        });
        warn_unsupported_path_globs(&config);
        Ok(config)
    }

    fn parse(toml_str: &str) -> Result<Self> {
        let config: ProjectConfig = toml::from_str(toml_str)?;
        for (index, rule) in config.paths.iter().enumerate() {
            let has_glob = rule.patterns().any(|pattern| !pattern.trim().is_empty());
            if !has_glob {
                bail!(
                    "[[path]] rule {} has no glob (set `glob` or `globs` to a non-empty pattern)",
                    index + 1
                );
            }
        }
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
            "typst" => self.typst.as_ref(),
            "asciidoc" => self.asciidoc.as_ref(),
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
            "typst" => self.typst.as_ref(),
            "asciidoc" => self.asciidoc.as_ref(),
            _ => None,
        };
        overrides.and_then(|ov| ov.max_width).or(self.max_width)
    }

    /// Whether a lowercase word after a terminator opens a sentence.
    /// Per-format `lowercase_starts` overrides the top-level key.
    /// Missing keys stay false.
    pub fn lowercase_starts_for_format(&self, format: &str) -> bool {
        let overrides = match format {
            "org" => self.org.as_ref(),
            "latex" => self.latex.as_ref(),
            "markdown" => self.markdown.as_ref(),
            "rst" => self.rst.as_ref(),
            "plaintext" => self.plaintext.as_ref(),
            "typst" => self.typst.as_ref(),
            "asciidoc" => self.asciidoc.as_ref(),
            _ => None,
        };
        overrides
            .and_then(|ov| ov.lowercase_starts)
            .or(self.lowercase_starts)
            .unwrap_or(false)
    }

    /// First `[[path]]` rule that matches `path`, if any.
    pub fn matching_rule(&self, path: &Path) -> Option<&PathRule> {
        let candidates = self.relative_candidates(path);
        self.paths.iter().find(|rule| {
            rule.patterns()
                .any(|pattern| candidates.iter().any(|cand| path_glob_match(pattern, cand)))
        })
    }

    /// True when a matching path rule sets `ignore`.
    pub fn path_ignored(&self, path: &Path) -> bool {
        self.matching_rule(path).is_some_and(|rule| rule.ignore)
    }

    /// True when a path rule or the top-level `ignore` list skips `path`.
    pub fn file_skipped(&self, path: &Path) -> bool {
        if self.path_ignored(path) {
            return true;
        }
        #[cfg(any(feature = "cli", feature = "watch"))]
        {
            self.is_ignored(path)
        }
        #[cfg(not(any(feature = "cli", feature = "watch")))]
        {
            let _ = path;
            false
        }
    }

    /// `max_width` from the first matching path rule, else the format value.
    pub fn max_width_for_path(&self, format: &str, path: Option<&Path>) -> Option<usize> {
        if let Some(path) = path {
            if let Some(width) = self.matching_rule(path).and_then(|rule| rule.max_width) {
                return Some(width);
            }
        }
        self.max_width_for_format(format)
    }

    /// `clause_breaks` from the first matching path rule, else the top-level key.
    pub fn clause_breaks_for_path(&self, path: Option<&Path>) -> bool {
        path.and_then(|candidate| self.matching_rule(candidate))
            .and_then(|rule| rule.clause_breaks)
            .or(self.clause_breaks)
            .unwrap_or(false)
    }

    /// Path rule, then per-format, then the top-level key. Missing stays false.
    pub fn lowercase_starts_for_path(&self, format: &str, path: Option<&Path>) -> bool {
        if let Some(path) = path {
            if let Some(value) = self
                .matching_rule(path)
                .and_then(|rule| rule.lowercase_starts)
            {
                return value;
            }
        }
        self.lowercase_starts_for_format(format)
    }

    /// Top-level and per-format abbreviations, plus extras from a matching rule.
    pub fn abbreviations_for_path(&self, format: &str, path: Option<&Path>) -> Vec<String> {
        let mut abbrevs = self.abbreviations_for_format(format);
        if let Some(path) = path {
            if let Some(rule) = self.matching_rule(path) {
                abbrevs.extend(rule.extra_abbreviations.iter().cloned());
            }
        }
        abbrevs
    }

    /// Path rule threshold, else the top-level `long_threshold`.
    pub fn long_threshold_for_path(&self, path: Option<&Path>) -> Option<usize> {
        path.and_then(|candidate| self.matching_rule(candidate))
            .and_then(|rule| rule.long_threshold)
            .or(self.long_threshold)
    }

    fn relative_candidates(&self, path: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut push = |rel: &Path| {
            let text = rel.to_string_lossy().replace('\\', "/");
            let text = text.trim_start_matches("./").to_string();
            if !text.is_empty() && !out.iter().any(|existing| existing == &text) {
                out.push(text);
            }
        };
        if let Some(root) = &self.root {
            // The config directory is the only base. A cwd-relative guess
            // matches the wrong file, and a path outside the tree matches none.
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            if let Some(rel) = path_relative_to_config(root, &cwd, path) {
                push(Path::new(&rel));
            }
            return out;
        }
        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(rel) = path.strip_prefix(&cwd) {
                push(rel);
            }
        }
        push(path);
        out
    }

    /// Extra `[latex].verbatim_envs` names. Empty when the key is missing.
    pub fn latex_verbatim_envs(&self) -> Vec<String> {
        self.latex
            .as_ref()
            .map(|ov| ov.verbatim_envs.clone())
            .unwrap_or_default()
    }

    /// Extra `[latex].structure_envs` names. Empty when the key is missing.
    pub fn latex_structure_envs(&self) -> Vec<String> {
        self.latex
            .as_ref()
            .map(|ov| ov.structure_envs.clone())
            .unwrap_or_default()
    }

    /// Extra `[latex].verbatim_commands` names. Empty when the key is missing.
    pub fn latex_verbatim_commands(&self) -> Vec<String> {
        self.latex
            .as_ref()
            .map(|ov| ov.verbatim_commands.clone())
            .unwrap_or_default()
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

/// Path of `path` relative to `root`, after joining a relative `path` onto `cwd`.
///
/// `.` and `..` are collapsed without reading the filesystem. `None` means
/// `path` is `root` itself or lives outside it.
pub(crate) fn path_relative_to_config(root: &Path, cwd: &Path, path: &Path) -> Option<String> {
    let absolute = if path.is_absolute() {
        normalize_lex(path)
    } else {
        normalize_lex(&cwd.join(path))
    };
    let root = if root.is_absolute() {
        normalize_lex(root)
    } else {
        normalize_lex(&cwd.join(root))
    };
    // A symlink and its target are the same file. Compare the canonical
    // paths when they exist so an absolute path through a link still matches.
    let absolute = canonicalize_existing(&absolute);
    let root = canonicalize_existing(&root);
    let rel = absolute.strip_prefix(&root).ok()?;
    if rel.as_os_str().is_empty() {
        return None;
    }
    let text = rel.to_string_lossy().replace('\\', "/");
    let text = text.trim_start_matches("./").to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn canonicalize_existing(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    let mut cur = path.to_path_buf();
    let mut tail = Vec::new();
    while !cur.as_os_str().is_empty() && !cur.exists() {
        let Some(name) = cur.file_name().map(|name| name.to_os_string()) else {
            break;
        };
        tail.push(name);
        if !cur.pop() {
            break;
        }
    }
    if let Ok(canonical) = cur.canonicalize() {
        let mut out = canonical;
        for name in tail.into_iter().rev() {
            out.push(name);
        }
        return out;
    }
    path.to_path_buf()
}

fn normalize_lex(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() && !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn warn_unsupported_path_globs(config: &ProjectConfig) {
    for rule in &config.paths {
        for pattern in rule.patterns() {
            if let Some(reason) = unsupported_glob_reason(pattern) {
                eprintln!(
                    "snapper: [[path]] glob {pattern:?} is not fully supported ({reason}); the rule may not match the files you expect"
                );
            }
        }
    }
}

/// `[[path]]` globs understand `*`, `?`, and `**` as its own segment.
/// Anything else is reported once, from [`ProjectConfig::load`].
pub(crate) fn unsupported_glob_reason(pattern: &str) -> Option<&'static str> {
    if pattern
        .chars()
        .any(|c| matches!(c, '[' | ']' | '{' | '}' | '\\'))
    {
        return Some("character classes, braces, and escapes are not matched");
    }
    for segment in pattern.split('/') {
        if segment.contains("**") && segment != "**" {
            return Some("`**` must be its own path segment");
        }
    }
    None
}

/// Drop a repeated `./` prefix. A trailing `/` means the directory and
/// everything under it (`docs/` matches `docs/a.md` and `docs/sub/a.md`).
fn normalize_glob(pattern: &str) -> String {
    let mut pattern = pattern.trim();
    while let Some(rest) = pattern.strip_prefix("./") {
        pattern = rest;
    }
    let pattern = pattern.trim_start_matches('/');
    if pattern.len() > 1 && pattern.ends_with('/') {
        format!("{}/**", pattern.trim_end_matches('/'))
    } else {
        pattern.to_string()
    }
}

/// `pattern` against `path`, both using `/`.
///
/// `**` matches across directories. `*` and `?` stay inside one directory.
/// A pattern with no `/` also matches the final path segment.
/// True when `pattern` matches `path` the way a `[[path]]` glob does.
///
/// MCP and WASM use this so an ignore rule is the same match the CLI uses.
pub fn path_rule_matches(pattern: &str, path: &str) -> bool {
    path_glob_match(pattern, path)
}

fn path_glob_match(pattern: &str, path: &str) -> bool {
    let pattern = normalize_glob(pattern);
    let path = path.trim_start_matches("./");
    if glob_parts(&pattern, path) {
        return true;
    }
    if !pattern.contains('/') {
        if let Some(name) = path.rsplit('/').next() {
            return glob_parts(&pattern, name);
        }
    }
    false
}

fn glob_parts(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match_parts(&pat, &segs)
}

fn match_parts(pat: &[&str], segs: &[&str]) -> bool {
    if pat.is_empty() {
        return segs.is_empty();
    }
    if pat[0] == "**" {
        if match_parts(&pat[1..], segs) {
            return true;
        }
        return !segs.is_empty() && match_parts(pat, &segs[1..]);
    }
    !segs.is_empty() && seg_match(pat[0], segs[0]) && match_parts(&pat[1..], &segs[1..])
}

fn seg_match(pattern: &str, segment: &str) -> bool {
    fn rec(pat: &[char], text: &[char]) -> bool {
        if pat.is_empty() {
            return text.is_empty();
        }
        if pat[0] == '*' {
            if rec(&pat[1..], text) {
                return true;
            }
            return !text.is_empty() && rec(pat, &text[1..]);
        }
        if text.is_empty() {
            return false;
        }
        (pat[0] == '?' || pat[0] == text[0]) && rec(&pat[1..], &text[1..])
    }
    let pat: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = segment.chars().collect();
    rec(&pat, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typo_names_the_unknown_field() {
        let err = ProjectConfig::parse("max_wdith = 10\n").unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("max_wdith"),
            "the error must name the typo, got {text}"
        );
        let err = ProjectConfig::parse("[[path]]\nmax_wdith = 10\n").unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("max_wdith"),
            "a path-rule typo must be named, got {text}"
        );
    }

    #[test]
    fn a_path_rule_needs_a_glob() {
        for toml in ["[[path]]\nignore = true\n", "[[path]]\nglob = \"\"\n"] {
            let err = ProjectConfig::parse(toml).unwrap_err();
            let text = err.to_string();
            assert!(
                text.contains("glob"),
                "missing or empty glob must be an error, got {text} for {toml:?}"
            );
        }
    }

    #[test]
    fn globs_drop_a_leading_dot_slash_and_a_trailing_slash() {
        assert!(path_glob_match("./docs/*.md", "docs/a.md"));
        assert!(path_glob_match("docs/", "docs/a.md"));
        assert!(path_glob_match("docs/", "docs/sub/a.md"));
        assert!(path_glob_match("docs/**/*.md", "docs/sub/a.md"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_resolves_to_the_config_root() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("docs")).unwrap();
        std::fs::write(real.join("docs/note.md"), "x").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let via_link = path_relative_to_config(&real, dir.path(), &link.join("docs/note.md"));
        assert_eq!(via_link.as_deref(), Some("docs/note.md"));
        let via_real = path_relative_to_config(&link, dir.path(), &real.join("docs/note.md"));
        assert_eq!(via_real.as_deref(), Some("docs/note.md"));
    }

    #[test]
    fn symlink_config_test_is_unix_only() {
        let src = include_str!("config.rs");
        let marker = "fn a_symlink_resolves_to_the_config_root";
        let at = src.find(marker).expect("symlink test");
        let before = &src[..at];
        let attr = before.rfind("#[cfg(unix)]").unwrap_or(0);
        let last_fn = before.rfind("\n    fn ").unwrap_or(0);
        assert!(
            attr > last_fn,
            "a_symlink_resolves_to_the_config_root calls std::os::unix::fs::symlink and must be #[cfg(unix)] so Windows can compile the tests"
        );
    }

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
        assert_eq!(config.long_threshold, None);
        assert_eq!(config.clause_breaks, Some(true));
        assert_eq!(config.lang, Some("de".to_string()));
    }

    #[test]
    fn parse_long_threshold() {
        let config = ProjectConfig::parse("long_threshold = 100\n").unwrap();
        assert_eq!(config.long_threshold, Some(100));
    }

    #[test]
    fn parse_lowercase_starts() {
        let top = ProjectConfig::parse("lowercase_starts = true\n").unwrap();
        assert!(top.lowercase_starts_for_format("markdown"));
        assert!(top.lowercase_starts_for_format("org"));
        let over =
            ProjectConfig::parse("lowercase_starts = true\n[markdown]\nlowercase_starts = false\n")
                .unwrap();
        assert!(!over.lowercase_starts_for_format("markdown"));
        assert!(over.lowercase_starts_for_format("org"));
        let missing = ProjectConfig::parse("").unwrap();
        assert!(!missing.lowercase_starts_for_format("plaintext"));
    }

    #[test]
    fn parse_use_pandoc() {
        let on = ProjectConfig::parse("use_pandoc = true\n").unwrap();
        assert_eq!(on.use_pandoc, Some(true));
        let off = ProjectConfig::parse("use_pandoc = false\n").unwrap();
        assert_eq!(off.use_pandoc, Some(false));
        let missing = ProjectConfig::parse("").unwrap();
        assert_eq!(missing.use_pandoc, None);
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

    #[test]
    fn parse_latex_env_and_command_lists() {
        let toml = r#"
[latex]
extra_abbreviations = ["Thm"]
verbatim_envs = ["Verbatim"]
structure_envs = ["algorithm", "comment"]
verbatim_commands = ["Verb"]
"#;
        let config = ProjectConfig::parse(toml).unwrap();
        assert_eq!(config.latex_verbatim_envs(), vec!["Verbatim"]);
        assert_eq!(config.latex_structure_envs(), vec!["algorithm", "comment"]);
        assert_eq!(config.latex_verbatim_commands(), vec!["Verb"]);
        assert_eq!(config.abbreviations_for_format("latex"), vec!["Thm"]);
    }

    #[test]
    fn missing_latex_env_keys_are_empty_extras() {
        let config = ProjectConfig::parse("[latex]\nextra_abbreviations = [\"Thm\"]\n").unwrap();
        assert!(config.latex_verbatim_envs().is_empty());
        assert!(config.latex_structure_envs().is_empty());
        assert!(config.latex_verbatim_commands().is_empty());
    }

    #[test]
    fn latex_other_regex_key_is_rejected() {
        // latexindent's `other:` regex matching is not a snapper key.
        let err = ProjectConfig::parse("[latex]\nother = \".*code\"\n").unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("other"),
            "an unknown latex key must be named, got {text}"
        );
    }

    #[test]
    fn path_globs_match_directories_and_bare_names() {
        assert!(path_glob_match("docs/**/*.md", "docs/a/b.md"));
        assert!(path_glob_match("docs/**/*.md", "docs/b.md"));
        assert!(!path_glob_match("docs/**/*.md", "docs/a/b.org"));
        assert!(path_glob_match("*.md", "docs/a.md"));
        assert!(path_glob_match("notes/?.txt", "notes/a.txt"));
        assert!(!path_glob_match("notes/?.txt", "notes/ab.txt"));
    }

    #[test]
    fn the_first_matching_path_rule_wins() {
        let config = ProjectConfig::parse(
            r#"
max_width = 80
extra_abbreviations = ["Global"]
clause_breaks = false

[[path]]
glob = "docs/**"
max_width = 40
clause_breaks = true
extra_abbreviations = ["RFC"]

[[path]]
globs = ["docs/special.md"]
max_width = 0
ignore = true
"#,
        )
        .unwrap();
        let special = Path::new("docs/special.md");
        assert_eq!(
            config.max_width_for_path("markdown", Some(special)),
            Some(40)
        );
        assert!(config.clause_breaks_for_path(Some(special)));
        assert!(!config.path_ignored(special));
        let abbrevs = config.abbreviations_for_path("markdown", Some(special));
        assert!(abbrevs.contains(&"Global".to_string()));
        assert!(abbrevs.contains(&"RFC".to_string()));
        let other = Path::new("notes/a.md");
        assert_eq!(config.max_width_for_path("markdown", Some(other)), Some(80));
        assert!(!config.clause_breaks_for_path(Some(other)));
    }

    #[test]
    fn a_rooted_rule_ignores_a_path_outside_the_config_tree() {
        let mut config = ProjectConfig::parse(
            r#"
[[path]]
glob = "*.md"
ignore = true
"#,
        )
        .unwrap();
        config.root = Some(PathBuf::from("/work/repo"));
        assert!(
            !config.path_ignored(Path::new("/other/docs/note.md")),
            "a file outside the config root must not match"
        );
        assert!(
            !config.path_ignored(Path::new("docs/note.md")),
            "a cwd-relative guess must not match a rooted rule"
        );
    }

    #[test]
    fn path_relative_to_config_joins_the_working_directory() {
        let rel = path_relative_to_config(
            Path::new("/work/repo"),
            Path::new("/work/repo/docs"),
            Path::new("note.md"),
        );
        assert_eq!(rel.as_deref(), Some("docs/note.md"));
        let parent = path_relative_to_config(
            Path::new("/work/repo"),
            Path::new("/work/repo/docs"),
            Path::new("../notes/a.md"),
        );
        assert_eq!(parent.as_deref(), Some("notes/a.md"));
        assert_eq!(
            path_relative_to_config(
                Path::new("/work/repo"),
                Path::new("/work/repo"),
                Path::new("/other/note.md"),
            ),
            None
        );
    }

    #[test]
    fn unsupported_globs_are_named_once() {
        assert!(unsupported_glob_reason("docs/**/*.md").is_none());
        assert!(unsupported_glob_reason("*.md").is_none());
        assert!(unsupported_glob_reason("docs/{a,b}/*.md").is_some());
        assert!(unsupported_glob_reason("docs/[abc]/*.md").is_some());
        assert!(unsupported_glob_reason("**.md").is_some());
        assert!(unsupported_glob_reason(r"docs/\*.md").is_some());
    }
}
