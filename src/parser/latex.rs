use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Line, SpannedRegion, flush_prose_spanned, iter_lines, join_prose_gap,
};
use crate::sentence::unicode::latex_verb_span_end_with;

// Environments whose content is NOT prose (math, code, figures, tables).
// Extra names are tree-sitter-latex `math_environment` plus latexindent
// `lookForAlignDelims` (amsmath / mathtools / tabularray), not a GPL copy.
static NON_PROSE_ENVS: &[&str] = &[
    "equation",
    "equation*",
    "align",
    "align*",
    "alignat",
    "alignat*",
    "aligned",
    "aligned*",
    "alignedat",
    "alignedat*",
    "flalign",
    "flalign*",
    "gather",
    "gather*",
    "gathered",
    "gathered*",
    "multline",
    "multline*",
    "eqnarray",
    "eqnarray*",
    "split",
    "split*",
    "displaymath",
    "displaymath*",
    "math",
    "figure",
    "figure*",
    "table",
    "table*",
    "tabular",
    "tabular*",
    "tabularx",
    "longtable",
    "tabu",
    "tblr",
    "longtblr",
    "talltblr",
    "lstlisting",
    "verbatim",
    "minted",
    "tikzpicture",
    "array",
    "array*",
    "matrix",
    "pmatrix",
    "bmatrix",
    "Bmatrix",
    "vmatrix",
    "Vmatrix",
    "cases",
    "cases*",
    "dcases",
    "dcases*",
    "rcases",
    "rcases*",
    "drcases",
    "drcases*",
];

/// `\begin{minted}{LANG}` -- the language is the brace argument after the env.
static MINTED_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\begin\{minted\}\s*(?:\[[^\]]*\])?\s*\{([^}]+)\}").unwrap());

/// `\begin{lstlisting}[language=LANG, ...]` -- language is an option key.
static LSTLISTING_LANG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\begin\{lstlisting\}\s*\[[^\]]*language\s*=\s*([A-Za-z0-9_+.\-]+)").unwrap()
});

/// Built-in source-code environments whose body is `Region::Code`.
///
/// Beyond minted/lstlisting/verbatim: latexindent `fileContentsEnvironments`
/// (`filecontents`, `filecontents*`) and tree-sitter-latex raw trivia envs
/// (`asy`, `asydef`, `pycode`, `luacode`, `luacode*`, `sagesilent`,
/// `sageblock`) plus fancyvrb `verbatim*` and the `comment` package env
/// (tree-sitter `comment_environment`: raw through matching `\end{comment}`).
fn is_builtin_code_env(name: &str) -> bool {
    matches!(
        name,
        "minted"
            | "lstlisting"
            | "verbatim"
            | "verbatim*"
            | "filecontents"
            | "filecontents*"
            | "asy"
            | "asydef"
            | "pycode"
            | "luacode"
            | "luacode*"
            | "sagesilent"
            | "sageblock"
            | "comment"
    )
}

static DISPLAY_MATH_CLOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\\]\s*$").unwrap());

/// tree-sitter `displayed_equation` (`$$` or `\[`) / latexindent `displayMath` + `displayMathTeX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayMathDelim {
    Bracket,
    Dollars,
}

fn display_math_open(s: &str) -> Option<DisplayMathDelim> {
    let t = s.trim_start();
    if t.starts_with("\\[") {
        Some(DisplayMathDelim::Bracket)
    } else if t.starts_with("$$") {
        Some(DisplayMathDelim::Dollars)
    } else {
        None
    }
}

fn display_math_closes(s: &str, delim: DisplayMathDelim) -> bool {
    match delim {
        DisplayMathDelim::Bracket => DISPLAY_MATH_CLOSE.is_match(s),
        DisplayMathDelim::Dollars => s.trim_end().ends_with("$$"),
    }
}

/// A line that is only `$$` is an opener, not a one-line `$$...$$` block.
fn display_math_is_single_line(s: &str, delim: DisplayMathDelim) -> bool {
    match delim {
        DisplayMathDelim::Bracket => DISPLAY_MATH_CLOSE.is_match(s),
        DisplayMathDelim::Dollars => {
            let t = s.trim();
            t != "$$" && t.ends_with("$$")
        }
    }
}

/// Sectioning commands whose brace argument is prose (titles can be long).
/// Captures: (1) command + opening brace prefix, (2) argument body, (3) closing brace + rest.
static SECTION_CMD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\s*\\(?:part|chapter|section|subsection|subsubsection|paragraph|subparagraph)\*?\{)([^}]*)(\}.*)$",
    )
    .unwrap()
});

#[derive(Debug, Default, Clone)]
pub struct LatexParser {
    extra_verbatim_envs: Vec<String>,
    extra_structure_envs: Vec<String>,
    extra_verbatim_commands: Vec<String>,
}

impl LatexParser {
    pub(crate) fn from_config(config: Option<&crate::FormatConfig>) -> Self {
        match config {
            Some(c) => Self {
                extra_verbatim_envs: c.latex_verbatim_envs.clone(),
                extra_structure_envs: c.latex_structure_envs.clone(),
                extra_verbatim_commands: c.latex_verbatim_commands.clone(),
            },
            None => Self::default(),
        }
    }

    fn is_comment(line: &str) -> bool {
        line.trim_start().starts_with('%')
    }

    /// Byte offset of the first `%` that is not escaped as `\%` and is not
    /// inside `\verb` / `\lstinline` / configured verbatim commands.
    fn unescaped_percent(&self, line: &str) -> Option<usize> {
        unescaped_percent_with(line, &self.extra_verbatim_commands)
    }

    fn is_code_env(&self, name: &str) -> bool {
        is_builtin_code_env(name) || self.extra_verbatim_envs.iter().any(|e| e == name)
    }

    fn is_non_prose_env(&self, name: &str) -> bool {
        NON_PROSE_ENVS.contains(&name)
            || self.extra_structure_envs.iter().any(|e| e == name)
            || self.is_code_env(name)
    }
}

fn unescaped_percent_with(line: &str, extra_cmds: &[String]) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(line, i, extra_cmds) {
                i = end;
                continue;
            }
            if i + 1 < bytes.len() {
                i += 2;
                continue;
            }
        }
        if bytes[i] == b'%' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `\begin{name}` or `\end{name}` on a line.
#[derive(Debug, Clone)]
struct EnvHit {
    start: usize,
    end: usize,
    is_begin: bool,
    name: String,
}

fn is_env_name(name: &str) -> bool {
    let core = name.strip_suffix('*').unwrap_or(name);
    !core.is_empty() && core.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn rest_line(line: Line<'_>, rel: usize) -> Line<'_> {
    Line {
        start: line.start + rel,
        end: line.end,
        text: &line.text[rel..],
    }
}

/// Through EOL when `rel..` is only whitespace; otherwise just `rel`.
fn thru_eol_if_blank_rest(line: Line<'_>, rel: usize) -> usize {
    if line.text[rel..].trim().is_empty() {
        line.end
    } else {
        line.start + rel
    }
}

fn find_env_at(line: &str, from: usize, extra_cmds: &[String]) -> Option<EnvHit> {
    let bytes = line.as_bytes();
    let mut i = from;
    let stop = unescaped_percent_with(line, extra_cmds).unwrap_or(line.len());
    while i < stop {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(line, i, extra_cmds) {
                i = end;
                continue;
            }
            let rest = &line[i..];
            let (is_begin, prefix_len) = if rest.starts_with("\\begin{") {
                (true, "\\begin{".len())
            } else if rest.starts_with("\\end{") {
                (false, "\\end{".len())
            } else if i + 1 < bytes.len() {
                i += 2;
                continue;
            } else {
                break;
            };
            let name_start = i + prefix_len;
            if let Some(rel) = line[name_start..stop].find('}') {
                let name = &line[name_start..name_start + rel];
                if is_env_name(name) {
                    let mut end = name_start + rel + 1;
                    if is_begin {
                        let mut j = end;
                        while j < stop && matches!(line.as_bytes()[j], b' ' | b'\t') {
                            j += 1;
                        }
                        if let Some(br) = skip_optional_brackets(line, j, stop) {
                            end = br;
                        }
                    }
                    return Some(EnvHit {
                        start: i,
                        end,
                        is_begin,
                        name: name.to_string(),
                    });
                }
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    None
}

fn skip_optional_brackets(line: &str, open_at: usize, stop: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    if bytes.get(open_at) != Some(&b'[') {
        return None;
    }
    let mut depth = 0;
    let mut i = open_at;
    while i < stop {
        match bytes[i] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn find_matching_end(
    line: &str,
    from: usize,
    name: &str,
    mut depth: usize,
    extra_cmds: &[String],
) -> Option<usize> {
    let mut i = from;
    while let Some(hit) = find_env_at(line, i, extra_cmds) {
        if hit.name != name {
            i = hit.end;
            continue;
        }
        if hit.is_begin {
            depth += 1;
            i = hit.end;
        } else {
            depth -= 1;
            if depth == 0 {
                return Some(hit.end);
            }
            i = hit.end;
        }
    }
    None
}

/// `\begin{name}` / `\end{name}` as raw source (lstlisting/verbatim/minted).
/// `%` and `\verb` are content, not a TeX comment or a skipped span.
fn find_raw_env_at(line: &str, from: usize) -> Option<EnvHit> {
    let bytes = line.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let rest = &line[i..];
            let (is_begin, prefix_len) = if rest.starts_with("\\begin{") {
                (true, "\\begin{".len())
            } else if rest.starts_with("\\end{") {
                (false, "\\end{".len())
            } else {
                i += 1;
                continue;
            };
            let name_start = i + prefix_len;
            if let Some(rel) = line[name_start..].find('}') {
                let name = &line[name_start..name_start + rel];
                if is_env_name(name) {
                    return Some(EnvHit {
                        start: i,
                        end: name_start + rel + 1,
                        is_begin,
                        name: name.to_string(),
                    });
                }
            }
        }
        i += 1;
    }
    None
}

fn find_matching_raw_end(line: &str, from: usize, name: &str, mut depth: usize) -> Option<usize> {
    let mut i = from;
    while let Some(hit) = find_raw_env_at(line, i) {
        if hit.name != name {
            i = hit.end;
            continue;
        }
        if hit.is_begin {
            depth += 1;
            i = hit.end;
        } else {
            depth -= 1;
            if depth == 0 {
                return Some(hit.end);
            }
            i = hit.end;
        }
    }
    None
}

/// TeX control sequence at `i`: `\name` or a one-character control symbol.
fn tex_cs_at(line: &str, i: usize) -> Option<&str> {
    let bytes = line.as_bytes();
    if bytes.get(i) != Some(&b'\\') {
        return None;
    }
    let rest = line.get(i + 1..)?;
    let first = rest.chars().next()?;
    if first.is_ascii_alphabetic() {
        let n = rest.chars().take_while(|c| c.is_ascii_alphabetic()).count();
        Some(&line[i..i + 1 + n])
    } else {
        Some(&line[i..i + 1 + first.len_utf8()])
    }
}

/// Next `cs` (`\iffalse`, `\fi`, …) at or after `from`. Raw: `%` is content.
fn find_tex_cs(line: &str, from: usize, cs: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if let Some(name) = tex_cs_at(line, i) {
                if name == cs {
                    return Some(i);
                }
                i += name.len();
                continue;
            }
        }
        i += 1;
    }
    None
}

/// `\iffalse` in ordinary TeX, skipping `\verb` / `\lstinline` spans.
fn find_iffalse_at(line: &str, from: usize, extra_cmds: &[String]) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(line, i, extra_cmds) {
                i = end;
                continue;
            }
            if let Some(name) = tex_cs_at(line, i) {
                if name == "\\iffalse" {
                    return Some(i);
                }
                i += name.len();
                continue;
            }
        }
        i += 1;
    }
    None
}

struct ParseState<'a> {
    input: &'a str,
    parser: &'a LatexParser,
    regions: Vec<SpannedRegion>,
    current_prose: String,
    prose_span: Option<ByteSpan>,
    in_non_prose_env: Option<String>,
    non_prose_depth: usize,
    in_code_env: Option<String>,
    code_depth: usize,
    code_lang: Option<String>,
    code_header: ByteSpan,
    code_body_start: usize,
    in_display_math: Option<DisplayMathDelim>,
    in_iffalse: bool,
    nospace_join: bool,
}

impl<'a> ParseState<'a> {
    fn flush(&mut self) {
        flush_prose_spanned(
            &mut self.current_prose,
            &mut self.prose_span,
            &mut self.regions,
        );
    }

    fn push_structure(&mut self, span: ByteSpan) {
        self.flush();
        if span.is_empty() {
            return;
        }
        self.regions
            .push(SpannedRegion::structure(self.input, span));
    }

    fn extend_prose_to(&mut self, end: usize) {
        if let Some(s) = &mut self.prose_span {
            if end > s.end {
                s.end = end;
            }
        }
    }

    /// `\item` / `\item[label]` is Structure so reflow can hang the next
    /// sentence at marker width (same class as Markdown/Org/RST `1.`).
    fn append_item_or_prose(&mut self, abs_start: usize, piece: &str) {
        if let Some(marker_len) = latex_item_marker_len(piece) {
            self.push_structure(ByteSpan::new(abs_start, abs_start + marker_len));
            self.append_prose_slice(abs_start + marker_len, &piece[marker_len..]);
        } else {
            self.append_prose_slice(abs_start, piece);
        }
    }

    fn append_prose_slice(&mut self, abs_start: usize, piece: &str) {
        let trimmed = piece.trim();
        if trimmed.is_empty() {
            return;
        }
        let lead = piece.len() - piece.trim_start().len();
        let content_start = abs_start + lead;
        let content_end = content_start + trimmed.len();
        if !self.current_prose.is_empty() && !self.nospace_join {
            join_prose_gap(&mut self.current_prose);
        }
        self.current_prose.push_str(trimmed);
        match &mut self.prose_span {
            None => self.prose_span = Some(ByteSpan::new(content_start, content_end)),
            Some(s) => s.end = content_end,
        }
        self.nospace_join = false;
    }

    fn enter_code(&mut self, env_name: &str, line: Line<'_>, hit_start: usize) {
        self.flush();
        let header_src = &line.text[hit_start..];
        self.code_lang = if env_name == "minted" {
            MINTED_LANG_RE
                .captures(header_src)
                .map(|c| c.get(1).unwrap().as_str().to_string())
        } else if env_name == "lstlisting" {
            LSTLISTING_LANG_RE
                .captures(header_src)
                .map(|c| c.get(1).unwrap().as_str().to_string())
        } else {
            None
        };
        self.code_header = ByteSpan::new(line.start + hit_start, line.end);
        self.code_body_start = line.end;
        self.in_code_env = Some(env_name.to_string());
        self.code_depth = 1;
    }

    fn consume_body_line(&mut self, line: Line<'_>) {
        if self.in_iffalse {
            self.consume_iffalse_line(line);
            return;
        }

        if self.in_non_prose_env.is_some() {
            self.consume_non_prose_line(line);
            return;
        }

        if let Some(delim) = self.in_display_math {
            self.flush();
            if display_math_closes(line.text, delim) {
                self.in_display_math = None;
            }
            self.regions
                .push(SpannedRegion::structure(self.input, line.span()));
            return;
        }

        if line.text.trim().is_empty() {
            self.flush();
            self.nospace_join = false;
            self.regions
                .push(SpannedRegion::blank(self.input, line.span()));
            return;
        }

        if LatexParser::is_comment(line.text) {
            self.flush();
            self.nospace_join = false;
            self.regions
                .push(SpannedRegion::structure(self.input, line.span()));
            return;
        }

        let pct = self.parser.unescaped_percent(line.text);
        let code = match pct {
            Some(idx) => &line.text[..idx],
            None => line.text,
        };

        if !code.trim().is_empty() {
            let line_done = self.consume_code_span(code, line);
            if line_done || self.in_code_env.is_some() || self.in_non_prose_env.is_some() {
                return;
            }
        }

        if let Some(idx) = pct {
            self.nospace_join = true;
            let comment = &line.text[idx..];
            if comment.trim() != "%" {
                self.flush();
                self.regions.push(SpannedRegion::structure(
                    self.input,
                    ByteSpan::new(line.start + idx, line.end),
                ));
            } else {
                self.extend_prose_to(line.end);
            }
        } else if self.in_code_env.is_none() && self.in_non_prose_env.is_none() {
            self.extend_prose_to(line.end);
            self.nospace_join = false;
        }
    }

    fn consume_iffalse_line(&mut self, line: Line<'_>) {
        // tree-sitter `_trivia_raw_fi`: first `\fi` command ends the skip.
        if let Some(close) = find_tex_cs(line.text, 0, "\\fi") {
            let after = close + "\\fi".len();
            let end = thru_eol_if_blank_rest(line, after);
            self.regions.push(SpannedRegion::structure(
                self.input,
                ByteSpan::new(line.start, end),
            ));
            self.in_iffalse = false;
            if !line.text[after..].trim().is_empty() {
                self.consume_body_line(rest_line(line, after));
            }
            return;
        }
        self.regions
            .push(SpannedRegion::structure(self.input, line.span()));
    }

    fn consume_non_prose_line(&mut self, line: Line<'_>) {
        let name = self
            .in_non_prose_env
            .as_deref()
            .expect("consume_non_prose_line only when inside")
            .to_string();
        let mut i = 0;
        while let Some(hit) = find_env_at(line.text, i, &self.parser.extra_verbatim_commands) {
            if hit.name != name {
                i = hit.end;
                continue;
            }
            if hit.is_begin {
                self.non_prose_depth += 1;
                i = hit.end;
            } else {
                self.non_prose_depth -= 1;
                if self.non_prose_depth == 0 {
                    let end = thru_eol_if_blank_rest(line, hit.end);
                    self.regions.push(SpannedRegion::structure(
                        self.input,
                        ByteSpan::new(line.start, end),
                    ));
                    self.in_non_prose_env = None;
                    if !line.text[hit.end..].trim().is_empty() {
                        self.consume_body_line(rest_line(line, hit.end));
                    }
                    return;
                }
                i = hit.end;
            }
        }
        self.regions
            .push(SpannedRegion::structure(self.input, line.span()));
    }

    fn consume_code_env_line(&mut self, line: Line<'_>) {
        let name = self
            .in_code_env
            .as_deref()
            .expect("consume_code_env_line only when inside")
            .to_string();
        let mut i = 0;
        while let Some(hit) = find_raw_env_at(line.text, i) {
            if hit.name != name {
                i = hit.end;
                continue;
            }
            if hit.is_begin {
                self.code_depth += 1;
                i = hit.end;
            } else {
                self.code_depth -= 1;
                if self.code_depth == 0 {
                    let footer_end = thru_eol_if_blank_rest(line, hit.end);
                    let footer = ByteSpan::new(line.start + hit.start, footer_end);
                    self.in_code_env = None;
                    self.regions.push(SpannedRegion::code(
                        self.input,
                        self.code_lang.take(),
                        self.code_header,
                        ByteSpan::new(self.code_body_start, line.start + hit.start),
                        footer,
                    ));
                    if !line.text[hit.end..].trim().is_empty() {
                        self.consume_body_line(rest_line(line, hit.end));
                    }
                    return;
                }
                i = hit.end;
            }
        }
    }

    /// Returns true when the physical `line` is fully consumed.
    fn consume_code_span(&mut self, code: &str, line: Line<'_>) -> bool {
        let mut i = 0;
        while i < code.len() {
            let env = find_env_at(code, i, &self.parser.extra_verbatim_commands);
            let iffalse = find_iffalse_at(code, i, &self.parser.extra_verbatim_commands);
            // tree-sitter `block_comment`: take the earlier of `\iffalse` and
            // `\begin`/`\end`. Env-first leaked `\iffalse ... \fi \end{document}`
            // (and `\iffalse ... \fi \begin{equation}`) as Prose.
            let env_first = match (env.as_ref(), iffalse) {
                (Some(hit), Some(open)) => hit.start <= open,
                (Some(_), None) => true,
                _ => false,
            };
            if env_first {
                let hit = env.expect("env_first implies find_env_at");
                self.append_item_or_prose(line.start + i, &code[i..hit.start]);
                if hit.is_begin && self.parser.is_code_env(&hit.name) {
                    self.flush();
                    if let Some(end_at) = find_matching_raw_end(line.text, hit.end, &hit.name, 1) {
                        let header = ByteSpan::new(line.start + hit.start, line.start + end_at);
                        let empty = ByteSpan::new(line.start + end_at, line.start + end_at);
                        self.regions
                            .push(SpannedRegion::code(self.input, None, header, empty, empty));
                        if !line.text[end_at..].trim().is_empty() {
                            self.consume_body_line(rest_line(line, end_at));
                        }
                        return true;
                    }
                    self.enter_code(&hit.name, line, hit.start);
                    return true;
                }
                if hit.is_begin && self.parser.is_non_prose_env(&hit.name) {
                    self.flush();
                    if let Some(end_at) = find_matching_end(
                        code,
                        hit.end,
                        &hit.name,
                        1,
                        &self.parser.extra_verbatim_commands,
                    ) {
                        let end = if code[end_at..].trim().is_empty() {
                            thru_eol_if_blank_rest(line, end_at)
                        } else {
                            line.start + end_at
                        };
                        self.regions.push(SpannedRegion::structure(
                            self.input,
                            ByteSpan::new(line.start + hit.start, end),
                        ));
                        i = end_at;
                        continue;
                    }
                    self.in_non_prose_env = Some(hit.name);
                    self.non_prose_depth = 1;
                    self.regions.push(SpannedRegion::structure(
                        self.input,
                        ByteSpan::new(line.start + hit.start, line.end),
                    ));
                    return true;
                }
                let cmd_end = thru_eol_if_blank_rest(line, hit.end);
                self.push_structure(ByteSpan::new(line.start + hit.start, cmd_end));
                i = hit.end;
                continue;
            }

            let rest = &code[i..];
            if let Some(open) = iffalse {
                self.append_item_or_prose(line.start + i, &code[i..open]);
                self.flush();
                let after_open = open + "\\iffalse".len();
                if let Some(close) = find_tex_cs(line.text, after_open, "\\fi") {
                    let after = close + "\\fi".len();
                    let end = if line.text[after..].trim().is_empty() {
                        thru_eol_if_blank_rest(line, after)
                    } else {
                        line.start + after
                    };
                    self.regions.push(SpannedRegion::structure(
                        self.input,
                        ByteSpan::new(line.start + open, end),
                    ));
                    if !line.text[after..].trim().is_empty() {
                        self.consume_body_line(rest_line(line, after));
                    }
                    return true;
                }
                self.in_iffalse = true;
                self.regions.push(SpannedRegion::structure(
                    self.input,
                    ByteSpan::new(line.start + open, line.end),
                ));
                return true;
            }
            if SECTION_CMD_RE.is_match(rest) {
                self.push_structure(ByteSpan::new(line.start + i, line.end));
                return false;
            }
            if let Some(delim) = display_math_open(rest) {
                self.flush();
                if !display_math_is_single_line(rest, delim) {
                    self.in_display_math = Some(delim);
                }
                self.regions.push(SpannedRegion::structure(
                    self.input,
                    ByteSpan::new(line.start + i, line.end),
                ));
                return false;
            }
            self.append_item_or_prose(line.start + i, rest);
            return false;
        }
        false
    }
}

/// Byte length of a compact LaTeX `\item` opener: leading indent, `\item`,
/// optional `[label]`, and one trailing space when present.
/// `\itemize` / `\itemsep` are different control words.
fn latex_item_marker_len(s: &str) -> Option<usize> {
    let indent = s.len() - s.trim_start_matches([' ', '\t']).len();
    let t = &s[indent..];
    let rest = t.strip_prefix("\\item")?;
    if rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut len = indent + "\\item".len();
    if rest.starts_with('[') {
        let close = rest.find(']')?;
        if rest[1..close].contains('[') {
            return None;
        }
        len += close + 1;
        if rest[close + 1..].starts_with(' ') {
            len += 1;
        }
        return Some(len);
    }
    if rest.starts_with(' ') {
        Some(len + 1)
    } else {
        Some(len)
    }
}

impl FormatParser for LatexParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let mut state = ParseState {
            input,
            parser: self,
            regions: Vec::new(),
            current_prose: String::new(),
            prose_span: None,
            in_non_prose_env: None,
            non_prose_depth: 0,
            in_code_env: None,
            code_depth: 0,
            code_lang: None,
            code_header: ByteSpan::default(),
            code_body_start: 0,
            in_display_math: None,
            in_iffalse: false,
            nospace_join: false,
        };
        let mut in_preamble = super::latex_starts_in_preamble(input);
        let mut pragma_off = false;

        for line in iter_lines(input) {
            // Check for snapper:off/on pragmas; inside a code environment
            // the per-language reflow path handles pragmas instead.
            if state.in_code_env.is_none() && !state.in_iffalse {
                if super::is_no_preamble_pragma(line.text) {
                    state.flush();
                    state
                        .regions
                        .push(SpannedRegion::structure(input, line.span()));
                    continue;
                }
                if let Some(on) = super::check_pragma(line.text) {
                    state.flush();
                    pragma_off = !on;
                    state
                        .regions
                        .push(SpannedRegion::structure(input, line.span()));
                    continue;
                }

                if pragma_off {
                    state.flush();
                    state
                        .regions
                        .push(SpannedRegion::structure(input, line.span()));
                    continue;
                }
            }

            // Preamble: everything before \begin{document} is structure
            if in_preamble {
                if line.text.contains(r"\begin{document}") {
                    in_preamble = false;
                }
                state.flush();
                state
                    .regions
                    .push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            if state.in_code_env.is_some() {
                state.consume_code_env_line(line);
                continue;
            }

            state.consume_body_line(line);
        }

        state.flush();
        if state.in_code_env.is_some() {
            let eof = ByteSpan::new(input.len(), input.len());
            state.regions.push(SpannedRegion::code(
                input,
                state.code_lang.take(),
                state.code_header,
                ByteSpan::new(state.code_body_start, input.len()),
                eof,
            ));
        }
        state.regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Region;

    #[test]
    fn section_command_title_is_structure_not_prose() {
        let input = "\\begin{document}\n\\section{A long title. With two sentences.}\nBody.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(r"\section{A long title. With two sentences.}")
            )),
            "full section line must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("A long title"))),
            "section title must not be Prose: {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(prose.contains(&"Body."));
    }

    #[test]
    fn multi_sentence_section_title_stays_one_line() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "\\begin{document}\n\\section{A long title. With two sentences.}\nBody text here. More body.\n\\end{document}\n";
        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("\\section{A long title. With two sentences.}"),
            "section title must stay one line, got:\n{out}"
        );
        assert!(
            !out.contains("\\section{A long title.\n"),
            "must not reflow mid-title inside braces:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn fragment_without_begin_document_is_prose() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "This sentence is a test. This sentence is also a test.\n";
        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, "This sentence is a test.\nThis sentence is also a test.\n",
            "input chapter without \\begin{{document}} must reflow, got:\n{out}"
        );
    }

    #[test]
    fn no_preamble_pragma_formats_body() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input =
            "% snapper:no-preamble\nThis sentence is a test. This sentence is also a test.\n";
        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.starts_with("% snapper:no-preamble\n"),
            "pragma line must stay structure, got:\n{out}"
        );
        assert!(
            out.contains("This sentence is a test.\nThis sentence is also a test."),
            "pragma must treat the file as body, got:\n{out}"
        );
    }

    #[test]
    fn documentclass_without_begin_stays_preamble() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input =
            "\\documentclass{article}\nThis sentence is a test. This sentence is also a test.\n";
        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, input, "class-only file must stay preamble");
    }

    #[test]
    fn preamble_is_structure() {
        let input = r"\documentclass{article}
\usepackage{amsmath}
\begin{document}
Hello world.
\end{document}";
        let regions = LatexParser::default().parse(input);
        // First 3 lines are preamble structure (including \begin{document})
        assert!(matches!(&regions[0], Region::Structure(_)));
        assert!(matches!(&regions[1], Region::Structure(_)));
        assert!(matches!(&regions[2], Region::Structure(_)));
        // "Hello world." is prose
        let has_prose = regions.iter().any(|r| matches!(r, Region::Prose(_)));
        assert!(has_prose);
    }

    #[test]
    fn equation_preserved() {
        let input = r"\begin{document}
Some text here.
\begin{equation}
E = mc^2
\end{equation}
More text.
\end{document}";
        let regions = LatexParser::default().parse(input);
        let structure_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Structure(_)))
            .count();
        // Preamble line + begin{equation} + E=mc^2 + end{equation} + end{document}
        assert!(structure_count >= 4);
    }

    #[test]
    fn comments_preserved() {
        let input = r"\begin{document}
% This is a comment
Some text.
\end{document}";
        let regions = LatexParser::default().parse(input);
        let comment_region = regions.iter().find(|r| {
            if let Region::Structure(s) = r {
                s.contains("% This is a comment")
            } else {
                false
            }
        });
        assert!(comment_region.is_some());
    }

    #[test]
    fn trailing_percent_is_nospace_join() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "\\begin{document}\nfoo%\nbar. Next sentence.\n\\end{document}\n";
        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("foo% bar"),
            "trailing % must not become a space comment: {out}"
        );
        assert!(
            out.contains("foobar.") || out.contains("foo%\nbar."),
            "foo%\\nbar must stay one TeX word, got:\n{out}"
        );
        assert!(out.contains("Next sentence."));
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn escaped_percent_is_not_a_comment() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "\\begin{document}\n50\\% of cases. More text.\n\\end{document}\n";
        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("50\\% of cases."),
            "escaped percent must stay in prose: {out}"
        );
        assert!(out.contains("More text."));
    }

    #[test]
    fn mid_line_percent_comment_is_structure() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "\\begin{document}\nSee Fig. 1. % TODO cite\nNext sentence.\n\\end{document}\n";
        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("% TODO cite"),
            "trailing comment must be kept: {out}"
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("% TODO cite"))),
            "mid-line % comment must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("TODO"))),
            "comment text must not stay in prose: {regions:?}"
        );
    }

    fn latex_cfg() -> crate::FormatConfig {
        crate::FormatConfig {
            format: crate::format::Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops()
    }

    #[test]
    fn verb_with_inner_punct_round_trips() {
        use crate::format_text;

        let input = "\\begin{document}\nUse \\verb|a.b! c| here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\verb|a.b! c|"),
            "verb must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("\\verb|a.\n") && !out.contains("\\verb|a.b!\n"),
            "inner .!? must not split the verb, got:\n{out}"
        );
        assert!(
            out.contains("Next sentence."),
            "following sentence must remain, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn lstinline_inner_percent_is_not_a_comment() {
        use crate::format_text;

        let input =
            "\\begin{document}\nCode \\lstinline!%! here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\lstinline!%!"),
            "lstinline with inner % must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("here."),
            "text after lstinline must not be commented out, got:\n{out}"
        );
        assert!(
            out.contains("Next sentence."),
            "following sentence must remain, got:\n{out}"
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            !regions.iter().any(
                |r| matches!(r, Region::Structure(s) if s.contains("%!") || s.trim() == "%!\n")
            ),
            "inner % of lstinline must not be a comment, got: {regions:?}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn lstinline_optional_args_round_trip() {
        use crate::format_text;

        let input = "\\begin{document}\nSee \\lstinline[language=TeX]!a.b%! please. Next.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\lstinline[language=TeX]!a.b%!"),
            "lstinline optional args and inner % must stay, got:\n{out}"
        );
        assert!(
            out.contains("please."),
            "prose after lstinline must remain, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn unknown_theorem_env_is_region_boundary() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore the claim. More before.\n\\begin{theorem}\nA statement. Another claim.\n\\end{theorem}\nAfter the claim. More after.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(r"\begin{theorem}"))),
            "\\begin{{theorem}} must be Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(r"\end{theorem}"))),
            "\\end{{theorem}} must be Structure, got: {regions:?}"
        );
        let before_mixed = regions.iter().any(|r| {
            matches!(
                r,
                Region::Prose(p) if p.contains("More before") && p.contains("A statement")
            )
        });
        assert!(
            !before_mixed,
            "theorem begin must bound regions, not concatenate neighboring prose: {regions:?}"
        );
        let after_mixed = regions.iter().any(|r| {
            matches!(
                r,
                Region::Prose(p) if p.contains("Another claim") && p.contains("After the claim")
            )
        });
        assert!(
            !after_mixed,
            "theorem end must bound regions, not concatenate neighboring prose: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("A statement"))),
            "theorem body must stay prose, got: {regions:?}"
        );

        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{theorem}\n"),
            "begin theorem must stay a boundary, got:\n{out}"
        );
        assert!(
            out.contains("A statement.\nAnother claim."),
            "theorem body must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn mid_line_begin_equation_leaves_leading_words_as_prose() {
        let input = "\\begin{document}\ninducing \\begin{equation}\nE = mc^2\n\\end{equation}\nAfter.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("inducing"))),
            "leading words before mid-line begin must be Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(
                    r,
                    Region::Structure(s) if s.contains("inducing") && s.contains(r"\begin{equation}")
                )
            }),
            "leading words must not be marked Structure with the env, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(r"\begin{equation}"))),
            "equation begin must still be Structure, got: {regions:?}"
        );
    }

    #[test]
    fn nested_same_name_envs_close_on_matching_depth() {
        let input = "\\begin{document}\n\\begin{equation}\n\\begin{equation}\nx = 1\n\\end{equation}\ny = 2\n\\end{equation}\nAfter the nest. Next.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("y = 2"))),
            "inner \\end must not close the outer equation; y = 2 stays Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("After the nest"))),
            "prose after the outer end must resume, got: {regions:?}"
        );
        let y_is_structure = regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("y = 2")));
        assert!(
            y_is_structure,
            "y = 2 must remain inside the outer equation Structure, got: {regions:?}"
        );
    }

    #[test]
    fn unmatched_verb_inner_percent_is_not_a_comment() {
        use crate::format_text;

        let input = "\\begin{document}\nSee \\verb|a%b. Next sentence.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            !regions.iter().any(|r| {
                matches!(r, Region::Structure(s) if s.contains("%b") || s.contains("%b."))
            }),
            "unmatched \\verb|a%b must not treat % as a comment, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains(r"\verb|a%b"))),
            "unmatched verb must stay in prose through EOL, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\verb|a%b"),
            "unmatched verb must keep inner %, got:\n{out}"
        );
        assert!(
            out.contains("Next sentence.") || out.contains(r"\verb|a%b. Next sentence."),
            "text after % must not be commented out, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn lstlisting_end_python_does_not_steal_the_close() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore.\n\\begin{lstlisting}\nprint(1)\n\\end{python} \\end{lstlisting}\nAfter the listing. Next.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        let code = regions.iter().find_map(|r| match r {
            Region::Code { body, footer, .. } => Some((body.as_str(), footer.as_str())),
            _ => None,
        });
        let (body, footer) = code.expect(&format!("lstlisting must be Code, got: {regions:?}"));
        assert!(
            body.contains("print(1)"),
            "listing body must keep source, got body={body:?} regions={regions:?}"
        );
        assert!(
            body.contains(r"\end{python}"),
            "\\end{{python}} is listing content, got body={body:?}"
        );
        assert!(
            !body.contains(r"\end{lstlisting}"),
            "real closer must not stay in the body, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{lstlisting}"),
            "footer must be \\end{{lstlisting}}, got footer={footer:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("After the listing"))),
            "prose after the listing must resume, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("After the listing."),
            "text after lstlisting must remain, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn nested_same_name_verbatim_closes_on_matching_depth() {
        let input = "\\begin{document}\n\\begin{verbatim}\n\\begin{verbatim}\ninner\n\\end{verbatim}\nstill body\n\\end{verbatim}\nAfter the nest.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        let code = regions.iter().find_map(|r| match r {
            Region::Code { body, footer, .. } => Some((body.as_str(), footer.as_str())),
            _ => None,
        });
        let (body, footer) = code.expect(&format!("verbatim must be Code, got: {regions:?}"));
        assert!(
            body.contains("still body"),
            "inner \\end must not close the outer verbatim; still body stays in the listing, got body={body:?} regions={regions:?}"
        );
        assert!(
            body.contains("inner"),
            "inner content must stay in the listing, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{verbatim}"),
            "outer closer is the footer, got footer={footer:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("After the nest"))),
            "prose after the outer end must resume, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("still body"))),
            "still body must not leak into prose, got: {regions:?}"
        );
    }

    #[test]
    fn lstlisting_percent_does_not_hide_end() {
        use crate::format_text;

        let input = "\\begin{document}\n\\begin{lstlisting}\nprint(1) % \\end{lstlisting}\nAfter the listing. Next.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        let code = regions.iter().find_map(|r| match r {
            Region::Code { body, footer, .. } => Some((body.as_str(), footer.as_str())),
            _ => None,
        });
        let (body, footer) = code.expect(&format!("lstlisting must be Code, got: {regions:?}"));
        assert!(
            body.contains("print(1)"),
            "listing body must keep source before %, got body={body:?} regions={regions:?}"
        );
        assert!(
            !body.contains("After the listing"),
            "% must not hide \\end{{lstlisting}}; after-text is not listing body, got body={body:?} regions={regions:?}"
        );
        assert!(
            footer.contains(r"\end{lstlisting}"),
            "footer must be \\end{{lstlisting}} even after %, got footer={footer:?} regions={regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("After the listing"))),
            "prose after the listing must resume, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("After the listing."),
            "text after lstlisting must remain, got:\n{out}"
        );
        assert!(
            out.contains("Next."),
            "following sentence must remain, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn lstlisting_same_line_percent_in_string_does_not_hide_end() {
        use crate::format_text;

        let input = "\\begin{document}\n\\begin{lstlisting} print(\"%\") \\end{lstlisting}\nAfter.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(r, Region::Code { .. })),
            "same-line lstlisting must be Code, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("After"))),
            "prose after same-line listing must resume, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains(r"\end{lstlisting}"))),
            "\\end{{lstlisting}} after % in a string must still close, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("After."),
            "text after same-line lstlisting must remain, got:\n{out}"
        );
        assert!(
            out.contains(r"\end{lstlisting}"),
            "closer must survive, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn theorem_optional_args_stay_on_the_begin_token() {
        let input = "\\begin{document}\nBefore.\n\\begin{theorem}[A. B. C.]\nA statement. Another.\n\\end{theorem}\nAfter.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| {
                matches!(r, Region::Structure(s) if s.contains(r"\begin{theorem}[A. B. C.]"))
            }),
            "optional [A. B. C.] must stay on the begin token, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("A. B. C."))),
            "theorem optional title must not become prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("A statement"))),
            "theorem body must stay prose, got: {regions:?}"
        );
    }

    #[test]
    fn missing_env_keys_keep_builtin_algorithm_as_prose() {
        use crate::format_text;

        // algorithm is not in NON_PROSE_ENVS; missing config keeps that list.
        let input = "\\begin{document}\nBefore the algo. More before.\n\\begin{algorithm}\nFirst step. Second step.\n\\end{algorithm}\nAfter the algo. More after.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("First step.\nSecond step."),
            "unlisted algorithm body must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn configured_structure_envs_stop_algorithm_and_comment_reflow() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore.\n\\begin{algorithm}\nFirst step. Second step.\n\\end{algorithm}\n\\begin{comment}\nHidden one. Hidden two.\n\\end{comment}\nAfter the block. Next.\n\\end{document}\n";
        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_structure_envs: vec!["algorithm".into(), "comment".into()],
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("First step. Second step."),
            "algorithm body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("First step.\nSecond step."),
            "algorithm must stay one source line, got:\n{out}"
        );
        assert!(
            out.contains("Hidden one. Hidden two."),
            "comment body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("Hidden one.\nHidden two."),
            "comment env must stay one source line, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after configured envs must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn configured_verbatim_env_stops_fancyvrb_reflow() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore.\n\\begin{Verbatim}\nFirst line. Second line.\n\\end{Verbatim}\nAfter the listing. Next.\n\\end{document}\n";
        let default_out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            default_out.contains("First line.\nSecond line."),
            "unlisted Verbatim body is prose and reflows, got:\n{default_out}"
        );

        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_verbatim_envs: vec!["Verbatim".into()],
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("First line. Second line."),
            "configured Verbatim body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "Verbatim must stay verbatim, got:\n{out}"
        );
        let regions = LatexParser::from_config(Some(&cfg)).parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "configured Verbatim must be Code, got: {regions:?}"
        );
        assert!(
            out.contains("After the listing.\nNext."),
            "prose after Verbatim must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn configured_verbatim_command_is_tokenized_like_verb() {
        use crate::format_text;

        let input = "\\begin{document}\nUse \\Verb|a.b! c| here. Next sentence.\n\\end{document}\n";
        let default_out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            !default_out.contains("Use \\Verb|a.b! c| here.\nNext sentence."),
            "unlisted Verb must not stay atomic like verb, got:\n{default_out}"
        );

        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_verbatim_commands: vec!["Verb".into()],
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains(r"\Verb|a.b! c|"),
            "configured Verb must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("\\Verb|a.\n") && !out.contains("\\Verb|a.b!\n"),
            "inner .!? must not split configured Verb, got:\n{out}"
        );
        assert!(
            out.contains("Use \\Verb|a.b! c| here.\nNext sentence."),
            "configured Verb must tokenize like verb before split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn configured_lists_keep_builtin_minted_and_equation() {
        use crate::format_text;

        let input = "\\begin{document}\nIntro. More intro.\n\\begin{equation}\nE = mc^2\n\\end{equation}\n\\begin{minted}{python}\nprint(1)\nprint(2)\n\\end{minted}\nAfter. Next.\n\\end{document}\n";
        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_verbatim_envs: vec!["Verbatim".into()],
            latex_structure_envs: vec!["algorithm".into()],
            latex_verbatim_commands: vec!["Verb".into()],
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("\\begin{equation}\nE = mc^2\n\\end{equation}"),
            "built-in equation must stay structure, got:\n{out}"
        );
        assert!(
            out.contains("\\begin{minted}{python}\nprint(1)\nprint(2)\n\\end{minted}"),
            "built-in minted must stay a code env, got:\n{out}"
        );
        assert!(
            out.contains("Intro.\nMore intro."),
            "surrounding prose must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn configured_lists_keep_eq_ref_nbsp() {
        use crate::format_text;

        let input = "\\begin{document}\nSee Eq.~\\ref{eq:diff}. Next.\n\\end{document}\n";
        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_verbatim_envs: vec!["Verbatim".into()],
            latex_structure_envs: vec!["algorithm".into()],
            latex_verbatim_commands: vec!["Verb".into()],
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Eq.~\\ref{eq:diff}."),
            "must not invent a space before ~, got:\n{out}"
        );
        assert!(
            !out.contains("Eq. ~"),
            "abbreviation merge must not insert a space before ~, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn configured_verb_inner_percent_is_not_a_comment() {
        use crate::format_text;

        let input = "\\begin{document}\nCode \\Verb!%! here. Next sentence.\n\\end{document}\n";
        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_verbatim_commands: vec!["Verb".into()],
            ..Default::default()
        };
        assert!(
            cfg.render_backstop && cfg.fixpoint_backstop,
            "this case is the production backstop path"
        );
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains(r"\Verb!%!"),
            "configured Verb with inner % must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("Code \\Verb!%! here.\nNext sentence."),
            "production backstops must not revert the whole file, got:\n{out}"
        );
        assert!(
            !out.contains("Code \\Verb!%! here. Next sentence."),
            "inner % is not a comment; the fused line must split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn configured_verb_does_not_steal_verbatim() {
        use crate::format_text;

        let input =
            "\\begin{document}\nUse \\Verbatim|x.y| here. Next sentence.\n\\end{document}\n";
        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_verbatim_commands: vec!["Verb".into()],
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Use \\Verbatim|x.y| here.\nNext sentence."),
            "\\Verb must not consume \\Verbatim, so the next sentence must split, got:\n{out}"
        );
        assert!(
            out.contains(r"\Verbatim|x.y|"),
            "\\Verbatim must remain in the source, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn latex_item_marker_len_skips_itemize() {
        assert_eq!(latex_item_marker_len("\\item "), Some(6));
        assert_eq!(latex_item_marker_len("  \\item "), Some(8));
        assert_eq!(latex_item_marker_len("\\item[Term] "), Some(12));
        assert_eq!(latex_item_marker_len("\\itemize"), None);
        assert_eq!(latex_item_marker_len("\\itemsep"), None);
        assert_eq!(latex_item_marker_len("not an item"), None);
    }

    #[test]
    fn enumerate_item_marker_is_structure() {
        let input =
            "\\begin{enumerate}\n\\item First sentence. Second sentence.\n\\end{enumerate}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "\\item ")),
            "\\item  must be Structure, got {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prose,
            ["First sentence. Second sentence."],
            "item body must be one Prose region, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("\\begin{enumerate}")
            )),
            "enumerate opener must stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("\\end{enumerate}")
            )),
            "enumerate closer must stay Structure, got {regions:?}"
        );
    }

    #[test]
    fn enumerate_item_hangs_next_sentence() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Latex,
            max_width: 0,
            ..Default::default()
        };
        let input =
            "\\begin{enumerate}\n\\item First sentence. Second sentence.\n\\end{enumerate}\n";
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            "\\begin{enumerate}\n\\item First sentence.\n      Second sentence.\n\\end{enumerate}\n",
            "enumerate item must hang at \\\\item  width, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(out, twice, "hung enumerate must be identity, got:\n{twice}");
        assert!(
            oracle::matches(Format::Latex, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn filecontents_fixture_file_is_code_not_prose() {
        let input = include_str!("../../tests/fixtures/filecontents.tex");
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. }
                    if body.contains("must not reflow as prose inside filecontents")
            )),
            "tests/fixtures/filecontents.tex body must be Code, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("inside filecontents")
            )),
            "filecontents fixture must not be Prose, got: {regions:?}"
        );
    }

    #[test]
    fn pycode_fixture_file_is_code_not_prose() {
        let input = include_str!("../../tests/fixtures/pycode.tex");
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. }
                    if body.contains("must not reflow as prose inside pycode")
            )),
            "tests/fixtures/pycode.tex body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("inside pycode"))),
            "pycode fixture must not be Prose, got: {regions:?}"
        );
    }

    /// Ticket fixture: filecontents / pycode must be Code, not reflowed prose.
    #[test]
    fn filecontents_and_pycode_fixtures_are_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{filecontents}{x.tex}\n",
            "This is a long sentence that must not reflow as prose inside filecontents.\n",
            "\\end{filecontents}\n",
            "\n",
            "\\begin{pycode}\n",
            "This is a long sentence that must not reflow as prose inside pycode.\n",
            "\\end{pycode}\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. }
                    if body.contains("must not reflow as prose inside filecontents")
            )),
            "filecontents body must be Code, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. }
                    if body.contains("must not reflow as prose inside pycode")
            )),
            "pycode body must be Code, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("inside filecontents") || p.contains("inside pycode")
            )),
            "filecontents/pycode bodies must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(
                "This is a long sentence that must not reflow as prose inside filecontents."
            ),
            "filecontents body must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("This is a long sentence that must not reflow as prose inside pycode."),
            "pycode body must stay intact, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn filecontents_and_trivia_env_two_sentences_do_not_reflow() {
        use crate::format_text;

        let names = [
            "filecontents*",
            "verbatim*",
            "asy",
            "asydef",
            "luacode",
            "luacode*",
            "sagesilent",
            "sageblock",
        ];
        for name in names {
            let input = format!(
                "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
            );
            let regions = LatexParser::default().parse(&input);
            assert!(
                regions.iter().any(|r| matches!(
                    r,
                    Region::Code { body, .. } if body.contains("First line. Second line.")
                )),
                "{name} body must be Code, got: {regions:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
                "{name} body must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains("First line. Second line."),
                "{name} body must not reflow, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must stay verbatim, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still reflow, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }
    }

    #[test]
    fn filecontents_required_filename_stays_on_begin() {
        let input = "\\begin{filecontents}{x.tex}\nFirst line. Second line.\n\\end{filecontents}\n";
        let regions = LatexParser::default().parse(input);
        let code = regions.iter().find_map(|r| match r {
            Region::Code {
                header,
                body,
                footer,
                ..
            } => Some((header.as_str(), body.as_str(), footer.as_str())),
            _ => None,
        });
        let Some((header, body, footer)) = code else {
            panic!("filecontents must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{filecontents}{x.tex}"),
            "filename arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "filecontents body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{filecontents}"),
            "footer must be \\end{{filecontents}}, got footer={footer:?}"
        );
    }

    #[test]
    fn dollar_dollar_display_math_is_structure_not_prose() {
        let input = "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("This is a long sentence that must stay inside display math")
            )),
            "$$ body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This is a long sentence that must stay inside display math")
            )),
            "$$ body must not be Prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "$$")),
            "$$ delimiters must be Structure, got: {regions:?}"
        );
    }

    #[test]
    fn dollar_dollar_display_math_does_not_reflow_as_prose() {
        use crate::format_text;

        let input = "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(
                "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$"
            ),
            "$$ display math must stay a structure block, got:\n{out}"
        );
        assert!(
            !out.contains("$$ This is a long sentence"),
            "must not join $$ into surrounding prose, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn dollar_dollar_two_sentences_do_not_split() {
        use crate::format_text;

        let input =
            "$$\nFirst sentence. Second sentence that would split if this were prose.\n$$\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(
                "$$\nFirst sentence. Second sentence that would split if this were prose.\n$$"
            ),
            "$$ display math must stay a structure block, got:\n{out}"
        );
        assert!(
            !out.contains("$$ First sentence"),
            "must not join $$ into surrounding prose, got:\n{out}"
        );
        assert!(
            !out.contains("First sentence.\nSecond sentence"),
            "$$ body must not split at sentence end, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn dollar_dollar_single_line_display_is_structure() {
        use crate::format_text;

        let input = "Before the math. More before.\n$$E = mc^2$$\nAfter the math. More after.\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("$$E = mc^2$$"))),
            "single-line $$...$$ must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("E = mc^2"))),
            "single-line $$ body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("$$E = mc^2$$"),
            "single-line $$ must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("Before the math.\nMore before."),
            "surrounding prose must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn bracket_display_math_still_structure() {
        use crate::format_text;

        let input = "\\[\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n\\]\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long sentence that must stay inside display math")
            )),
            "\\[ body must stay Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("This is a long sentence that must stay inside display math")
            )),
            "\\[ body must not become Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(
                "\\[\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n\\]"
            ),
            "\\[ display math must stay a structure block, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn inline_single_dollar_math_is_still_prose() {
        use crate::format_text;

        let input = "See $x = 1$ here. Next sentence.\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("$x = 1$"))),
            "inline $...$ must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("See $x = 1$ here.\nNext sentence."),
            "inline $...$ must not open display math, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn tree_sitter_and_latexindent_math_table_envs_are_structure() {
        // Names missing on origin/main; each body is Structure, not Prose.
        let names = [
            "displaymath",
            "displaymath*",
            "math",
            "aligned",
            "aligned*",
            "alignat",
            "alignat*",
            "alignedat",
            "alignedat*",
            "flalign",
            "flalign*",
            "gathered",
            "gathered*",
            "split",
            "split*",
            "tabularx",
            "longtable",
            "tabu",
            "cases",
            "cases*",
            "dcases",
            "dcases*",
            "rcases",
            "rcases*",
            "drcases",
            "drcases*",
            "tblr",
            "longtblr",
            "talltblr",
            "Bmatrix",
            "vmatrix",
            "Vmatrix",
            "array*",
        ];
        for name in names {
            let input = format!(
                "\\begin{{{name}}}\nThis is a long sentence that must not reflow as prose inside {name}.\n\\end{{{name}}}\n"
            );
            let needle = format!("must not reflow as prose inside {name}");
            let regions = LatexParser::default().parse(&input);
            assert!(
                regions
                    .iter()
                    .any(|r| matches!(r, Region::Structure(s) if s.contains(&needle))),
                "{name} body must be Structure, got: {regions:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains(&needle))),
                "{name} body must not be Prose, got: {regions:?}"
            );
        }
    }

    #[test]
    fn alignat_and_tabularx_bodies_are_structure_not_prose() {
        let input = "\\begin{document}\n\\begin{alignat}{2}\nThis is a long sentence that must not reflow as prose inside alignat.\n\\end{alignat}\n\\begin{tabularx}{\\textwidth}{l}\nThis is a long sentence that must not reflow as prose inside tabularx.\n\\end{tabularx}\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("must not reflow as prose inside alignat")
            )),
            "alignat body must be Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("must not reflow as prose inside tabularx")
            )),
            "tabularx body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("inside alignat") || p.contains("inside tabularx")
            )),
            "alignat/tabularx bodies must not be Prose, got: {regions:?}"
        );
    }

    #[test]
    fn alignat_and_tabularx_two_sentence_bodies_do_not_reflow() {
        use crate::format_text;

        let input = "\\begin{document}\n\\begin{alignat}{2}\nFirst sentence inside alignat. Second sentence stays put.\n\\end{alignat}\n\\begin{tabularx}{\\textwidth}{l}\nFirst sentence inside tabularx. Second sentence stays put.\n\\end{tabularx}\nAfter the tables. Next.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("First sentence inside alignat. Second sentence stays put."),
            "alignat body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("First sentence inside alignat.\nSecond sentence stays put."),
            "alignat body must stay one source line, got:\n{out}"
        );
        assert!(
            out.contains("First sentence inside tabularx. Second sentence stays put."),
            "tabularx body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("First sentence inside tabularx.\nSecond sentence stays put."),
            "tabularx body must stay one source line, got:\n{out}"
        );
        assert!(
            out.contains("After the tables.\nNext."),
            "prose after the envs must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn comment_fixture_file_is_code_or_structure_not_prose() {
        let input = include_str!("../../tests/fixtures/comment.tex");
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| match r {
                Region::Code { body, .. } => {
                    body.contains("must not reflow as prose inside comment")
                }
                Region::Structure(s) => s.contains("must not reflow as prose inside comment"),
                _ => false,
            }),
            "tests/fixtures/comment.tex body must be Code or Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("inside comment")
            )),
            "comment fixture must not be Prose, got: {regions:?}"
        );
    }

    #[test]
    fn iffalse_fixture_file_is_structure_not_prose() {
        let input = include_str!("../../tests/fixtures/iffalse.tex");
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("must not reflow as prose inside iffalse")
            )),
            "tests/fixtures/iffalse.tex body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("inside iffalse")
            )),
            "iffalse fixture must not be Prose, got: {regions:?}"
        );
    }

    #[test]
    fn comment_environment_body_is_not_prose() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore.\n\\begin{comment}\nThis is a long sentence that must not reflow as prose inside comment.\n\\end{comment}\nAfter the comment. Next.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| match r {
                Region::Code { body, .. } => {
                    body.contains("must not reflow as prose inside comment")
                }
                Region::Structure(s) => s.contains("must not reflow as prose inside comment"),
                _ => false,
            }),
            "comment env body must be Code or Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("must not reflow as prose inside comment")
            )),
            "comment env body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("This is a long sentence that must not reflow as prose inside comment."),
            "comment body must stay one line, got:\n{out}"
        );
        assert!(
            out.contains("After the comment.\nNext."),
            "prose after comment env must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn comment_environment_two_sentences_do_not_reflow() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore.\n\\begin{comment}\nHidden one. Hidden two.\n\\end{comment}\nAfter the comment. Next.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("Hidden one. Hidden two."),
            "comment env two sentences must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("Hidden one.\nHidden two."),
            "comment env must stay one source line, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn iffalse_block_is_structure_not_prose() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore.\n\\iffalse\nThis is a long sentence that must not reflow as prose inside iffalse.\n\\fi\nAfter the skip. Next.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("must not reflow as prose inside iffalse")
            )),
            "iffalse body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("must not reflow as prose inside iffalse")
            )),
            "iffalse body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("This is a long sentence that must not reflow as prose inside iffalse."),
            "iffalse body must stay one line, got:\n{out}"
        );
        assert!(
            out.contains("After the skip.\nNext."),
            "prose after \\fi must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn iffalse_two_sentences_do_not_reflow() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore.\n\\iffalse\nHidden one. Hidden two.\n\\fi\nAfter the skip. Next.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("Hidden one. Hidden two."),
            "iffalse two sentences must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("Hidden one.\nHidden two."),
            "iffalse must stay one source line, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn same_line_iffalse_is_structure() {
        let input = "\\begin{document}\nKeep this. \\iffalse Hidden one. Hidden two. \\fi After. Next.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(r"\iffalse") && s.contains("Hidden one")
            )),
            "same-line iffalse must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Hidden one"))),
            "iffalse payload must not be Prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Keep this"))),
            "text before iffalse stays prose, got: {regions:?}"
        );
    }

    #[test]
    fn same_line_iffalse_before_end_env_is_structure() {
        use crate::format_text;

        // snapper-d93x: env-first scan leaked the payload as Prose because
        // `\end{document}` sits later on the same physical line.
        let input = "\\begin{document}\nKeep this. \\iffalse Hidden one. Hidden two. \\fi After. \\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(r"\iffalse") && s.contains("Hidden one")
            )),
            "same-line iffalse before \\end must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Hidden one"))),
            "iffalse payload before \\end must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("Hidden one. Hidden two."),
            "iffalse two sentences must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("Hidden one.\nHidden two."),
            "iffalse payload must stay one source line, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn same_line_iffalse_before_begin_env_is_structure() {
        use crate::format_text;

        let input = "\\begin{document}\nKeep this. \\iffalse Hidden one. Hidden two. \\fi \\begin{equation}x=1.\\end{equation}\nAfter the skip. Next.\n\\end{document}\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(r"\iffalse") && s.contains("Hidden one")
            )),
            "same-line iffalse before \\begin must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Hidden one"))),
            "iffalse payload before \\begin must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("Hidden one. Hidden two."),
            "iffalse two sentences must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("Hidden one.\nHidden two."),
            "iffalse payload must stay one source line, got:\n{out}"
        );
        assert!(
            out.contains("After the skip.\nNext."),
            "prose after the envs must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}
