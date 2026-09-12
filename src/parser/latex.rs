use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Line, SpannedRegion, flush_prose_spanned, iter_lines, join_prose_gap,
};
use crate::sentence::unicode::latex_verb_span_end_with;

// Environments whose content is NOT prose (math, code, tables, pictures).
// Extra names are tree-sitter-latex `math_environment` plus latexindent
// `lookForAlignDelims` (amsmath / mathtools / tabularray / nicematrix /
// listabla / spreadtab), not a GPL copy.
// Overleaf leftover names (tokens.mjs) are `IEEEeqnarray` /
// `IEEEeqnarray*` / `subeqnarray` / `subeqnarray*` / `xltabular` /
// `math*`. `tikzcd` / pgfplots `axis` / `pgfpicture` are the same
// class (and starred variants). Not every pgfplots name.
//
// `figure` / `table` (and stars) are not here: Overleaf FigureEnvironment
// is Content<Text>, tree-sitter caption curly_group is text. Float chrome
// stays Structure; the caption long argument is Prose (snapper-t4lj / #95).
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
    "IEEEeqnarray",
    "IEEEeqnarray*",
    "subeqnarray",
    "subeqnarray*",
    "split",
    "split*",
    "displaymath",
    "displaymath*",
    "math",
    "math*",
    "tabular",
    "tabular*",
    "tabularx",
    "xltabular",
    "longtable",
    "tabu",
    "tblr",
    "longtblr",
    "talltblr",
    "listabla",
    "spreadtab",
    "NiceTabular",
    "NiceMatrix",
    "pNiceMatrix",
    "bNiceMatrix",
    "BNiceMatrix",
    "vNiceMatrix",
    "VNiceMatrix",
    "NiceArray",
    "pNiceArrayC",
    "bNiceArrayC",
    "BNiceArrayC",
    "vNiceArrayC",
    "VNiceArrayC",
    "NiceArrayCwithDelims",
    "pNiceArrayRC",
    "bNiceArrayRC",
    "BNiceArrayRC",
    "vNiceArrayRC",
    "VNiceArrayRC",
    "NiceArrayRCwithDelims",
    "lstlisting",
    "verbatim",
    "minted",
    "tikzpicture",
    "tikzcd",
    "tikzcd*",
    "pgfpicture",
    "pgfpicture*",
    "axis",
    "axis*",
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

/// Float environments: chrome is Structure; `\caption` long arg is Prose.
fn is_float_env(name: &str) -> bool {
    matches!(name, "figure" | "figure*" | "table" | "table*")
}

/// `\begin{minted}{LANG}` / `\begin{minted*}{LANG}` -- language is the brace
/// argument after the env. minted.sty `minted*` is the starred twin (same
/// FV@Scan / minted body; GitHub #273).
static MINTED_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\begin\{minted\*?\}\s*(?:\[[^\]]*\])?\s*\{([^}]+)\}").unwrap());

/// `\begin{lstlisting}[language=LANG, ...]` / `\begin{lstlisting*}[...]`.
/// listings.sty `\lstnewenvironment{lstlisting}` defines both names.
static LSTLISTING_LANG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\begin\{lstlisting\*?\}\s*\[[^\]]*language\s*=\s*([A-Za-z0-9_+.\-]+)").unwrap()
});

/// Built-in source-code environments whose body is `Region::Code`.
///
/// Beyond minted/lstlisting/verbatim: latexindent `fileContentsEnvironments`
/// (`filecontents`, `filecontents*`), filecontentsdef.sty `filecontentsdef`
/// (verbatim write of the env body into a macro; same raw grab as
/// `filecontents`; GitHub #294) plus leftover siblings `filecontentsgdef` /
/// `filecontentsdefmacro` / `filecontentsgdefmacro` / `filecontentshere` and
/// starred twins `filecontentsdef*` / `filecontentsgdef*` /
/// `filecontentshere*` (same raw grab; GitHub #299), scontents.sty
/// `scontents` (verbatim store into a sequence) / `verbatimsc` (package
/// verbatim display env; GitHub #304), and tree-sitter-latex
/// raw trivia envs
/// (`asy`, `asydef`, `pycode`, `luacode`, `luacode*`, `sagesilent`,
/// `sageblock`), luamplib.dtx `mplibcode` (same raw grab class as
/// `luacode`; GitHub #348), sagetex.sty `sageverbatim` / `sageexample` /
/// `sagecommandline` (same `verbatim@start` class as tree-sitter
/// `sagesilent` / `sageblock`; GitHub #298), pythontex.sty
/// `pyblock` / `pyverbatim` / `pyconsole`
/// / `pycode*` / `pyblock*` / `pyverbatim*` / `pyconsole*` / `pygments`
/// / `sympycode` / `sympyblock` / `sympyverbatim` / `sympyconsole` /
/// `pylabcode` / `pylabblock` / `pylabverbatim` / `pylabconsole` and
/// starred twins, plus leftover default-family `pyconcode` /
/// `pyconverbatim` / `pysub` / `pyconsub` / `sympyconcode` /
/// `sympyconverbatim` / `sympysub` / `sympyconsub` / `pylabconcode` /
/// `pylabconverbatim` / `pylabsub` / `pylabconsub` /
/// `pythontexcustomcode` (same `VerbatimEnvironment` class as `pycode`;
/// GitHub #249 / #274 / #276 / #277 / #278 / #333), plus option-family
/// `usefamily` / `\makepythontexfamily` leftovers (`rubycode`
/// representative; ruby/rb/julia/juliacon/jl/matlab/octave/bash/sage/
/// rust/rs/R/Rcon/perl/pl/perlsix/psix/javascript/js; GitHub #352),
/// pythonhighlight.sty `python` (`\lstnewenvironment{python}`; same
/// listings raw scan as `lstlisting`; GitHub #307),
/// pyluatex.sty `pythonq` / `pythonrepl` (verbatim python / REPL
/// bodies; GitHub #346),
/// showexpl.sty `LTXexample` (`\lstnewenvironment{LTXexample}`; same
/// listings raw scan as `lstlisting`; GitHub #345),
/// luamplib.dtx `mplibcode` (same raw grab class as `luacode`;
/// GitHub #348),
/// codehigh.sty `codehigh` / `demohigh` and starred twins
/// (`NewCodeHighEnv`; GitHub #350),
/// plus latex2e `verbatim*` / fancyvrb `Verbatim` /
/// `Verbatim*` / `BVerbatim` / `BVerbatim*` / `LVerbatim` /
/// `LVerbatim*` / `SaveVerbatim` / `VerbatimOut` / `VerbatimWrite` /
/// `VerbatimBuffer` / fvextra `VerbEnv` / verbments `pyglist` /
/// texments / pygmentex `pygmented`,
/// moreverb `boxedverbatim` / `verbatimtab` / `verbatimwrite` / `listing` /
/// `listingcont` / `listing*` / `listingcont*`, leftover sverb.sty
/// `verbwrite` / `ignore` / `demo` / `demo*` (`sv@readenv` raw grab;
/// GitHub #353), tcolorbox `tcblisting` /
/// `tcblisting*` / `codeexample` / `tcbverbatimwrite` / `tcbwritetemp` /
/// leftover `tcboutputlisting` / `tcbexternal` / `dispExample` /
/// `dispExample*` / `dispListing` / `dispListing*`,
/// standard `alltt` (alltt.sty: macros
/// still apply, line breaks stay raw; GitHub #230), spverbatim.sty `spverbatim`
/// (raw body; `\spverb` is the matching delimiter-body command,
/// GitHub #235), and the `comment` package env (tree-sitter
/// `comment_environment`: raw through matching `\end{comment}`).
/// Overleaf `verbatimEnvNames` is Verbatim, boxedverbatim, tcblisting,
/// codeexample. fancyvrb `BVerbatim` / `LVerbatim` are the same raw
/// class as `Verbatim` (GitHub #209). `SaveVerbatim` / `VerbatimOut`
/// are the same `FV@Scan` class (GitHub #213). fvextra `VerbatimWrite`
/// is the same `FV@Scan` class as `VerbatimOut` (GitHub #247). fvextra
/// `VerbatimBuffer` is the same raw grab as `VerbatimWrite`
/// (detokenize buffer; GitHub #292). fvextra `VerbEnv` is the
/// environment form of `Verb` (single-line raw body, closer on its
/// own line; GitHub #293). fancyvrb
/// `Verbatim*` / `BVerbatim*` / `LVerbatim*` are the starred twins
/// (same `\FV@Scan`; GitHub #244). listings.sty
/// `\lstnewenvironment{lstlisting}` also defines `lstlisting*` (same
/// raw body scan; GitHub #234). tcolorbox listings library
/// `tcblisting*` is the starred twin of `tcblisting` (same raw listing
/// body; GitHub #246). tcolorbox `tcbverbatimwrite` / `tcbwritetemp`
/// write the env body raw to a file (same verbatim grab as
/// `VerbatimOut`; GitHub #280). leftover tcolorbox write/listing
/// envs `tcboutputlisting` / `tcbexternal` / `dispExample` /
/// `dispExample*` / `dispListing` / `dispListing*` are the same raw
/// grab (tcblistingscore / tcbexternal / tcbdocumentation;
/// GitHub #334). moreverb `verbatimtab` is the same
/// tab-expanding raw class as `boxedverbatim` (GitHub #250). moreverb
/// `verbatimwrite` writes the env body raw via `verbatim@start` (same
/// class as `VerbatimOut` / `tcbverbatimwrite`; GitHub #306). leftover
/// sverb `verbwrite` / `ignore` / `demo` / `demo*` are the same
/// `sv@readenv` raw grab (write / discard / demo display; GitHub #353).
/// moreverb
/// `listing` / `listingcont` / `listing*` / `listingcont*` are
/// `verbatim@start` raw bodies (starred twins do not expand tabs;
/// GitHub #279). minted.sty
/// `minted*` is the starred twin of `minted` (same FV@Scan / minted
/// body; GitHub #273). sagetex.sty `sageverbatim` / `sageexample` /
/// `sagecommandline` use `verbatim@start` like tree-sitter
/// `sagesilent` / `sageblock` (GitHub #298). scontents.sty
/// `scontents` stores the env body verbatim into a sequence;
/// `verbatimsc` is the package verbatim display env (GitHub #304).
/// piton.sty `{Piton}` is a verbatim listing env (GitHub #305);
/// `\piton|...|` is the matching verb-like command.
/// pythonhighlight.sty `python` (`\lstnewenvironment{python}`) is the
/// same listings raw scan as `lstlisting` (GitHub #307).
/// pyluatex.sty `pythonq` / `pythonrepl` are verbatim python / REPL
/// bodies (GitHub #346). Landed `python` stays Code.
/// showexpl.sty `LTXexample` (`\lstnewenvironment{LTXexample}`) is the
/// same listings raw scan as `lstlisting` (GitHub #345).
/// luamplib.dtx `mplibcode` is the same raw grab class as `luacode`
/// (GitHub #348).
/// codehigh.sty `codehigh` / `demohigh` / `codehigh*` / `demohigh*`
/// (`NewCodeHighEnv`) are the same raw grab class (GitHub #350).
/// verbments.sty `pyglist` wraps fancyvrb `VerbatimOut` (raw listing
/// body; GitHub #308). texments.sty / pygmentex.sty `pygmented` is
/// `VerbatimEnvironment` plus `VerbatimOut` (raw listing body;
/// GitHub #342).
fn is_builtin_code_env(name: &str) -> bool {
    matches!(
        name,
        "minted"
            | "minted*"
            | "lstlisting"
            | "lstlisting*"
            | "python"
            | "pythonq"
            | "pythonrepl"
            | "LTXexample"
            | "codehigh"
            | "codehigh*"
            | "demohigh"
            | "demohigh*"
            | "verbatim"
            | "verbatim*"
            | "Verbatim"
            | "Verbatim*"
            | "BVerbatim"
            | "BVerbatim*"
            | "LVerbatim"
            | "LVerbatim*"
            | "SaveVerbatim"
            | "VerbatimOut"
            | "pyglist"
            | "pygmented"
            | "VerbatimWrite"
            | "VerbatimBuffer"
            | "VerbEnv"
            | "alltt"
            | "boxedverbatim"
            | "verbatimtab"
            | "verbatimwrite"
            | "verbwrite"
            | "ignore"
            | "demo"
            | "demo*"
            | "listing"
            | "listingcont"
            | "listing*"
            | "listingcont*"
            | "tcblisting"
            | "tcblisting*"
            | "codeexample"
            | "tcbverbatimwrite"
            | "tcbwritetemp"
            | "tcboutputlisting"
            | "tcbexternal"
            | "dispExample"
            | "dispExample*"
            | "dispListing"
            | "dispListing*"
            | "spverbatim"
            | "filecontents"
            | "filecontents*"
            | "filecontentsdef"
            | "filecontentsdef*"
            | "filecontentsgdef"
            | "filecontentsgdef*"
            | "filecontentsdefmacro"
            | "filecontentsgdefmacro"
            | "filecontentshere"
            | "filecontentshere*"
            | "scontents"
            | "verbatimsc"
            | "asy"
            | "asydef"
            | "pycode"
            | "pycode*"
            | "pyblock"
            | "pyblock*"
            | "pyverbatim"
            | "pyverbatim*"
            | "pyconsole"
            | "pyconsole*"
            | "pyconcode"
            | "pyconverbatim"
            | "pysub"
            | "pyconsub"
            | "pygments"
            | "pythontexcustomcode"
            | "sympycode"
            | "sympycode*"
            | "sympyblock"
            | "sympyblock*"
            | "sympyverbatim"
            | "sympyverbatim*"
            | "sympyconsole"
            | "sympyconsole*"
            | "sympyconcode"
            | "sympyconverbatim"
            | "sympysub"
            | "sympyconsub"
            | "pylabcode"
            | "pylabcode*"
            | "pylabblock"
            | "pylabblock*"
            | "pylabverbatim"
            | "pylabverbatim*"
            | "pylabconsole"
            | "pylabconsole*"
            | "pylabconcode"
            | "pylabconverbatim"
            | "pylabsub"
            | "pylabconsub"
            | "luacode"
            | "luacode*"
            | "mplibcode"
            | "sagesilent"
            | "sageblock"
            | "sageverbatim"
            | "sageexample"
            | "sagecommandline"
            | "comment"
            | "Piton"
    ) || is_pythontex_option_family_env(name)
}

/// pythontex.sty `usefamily` / `\makepythontexfamily` option-only
/// families (GitHub #352). Same `VerbatimEnvironment` class as `pycode`.
/// Regular families mint `{name}code` / `{name}block` / `{name}verbatim`
/// / `{name}sub` (and starred twins of the first three). `juliacon` /
/// `Rcon` mint `{name}code` plus `{name}sole` (`juliaconsole` /
/// `Rconsole`).
fn is_pythontex_option_family_env(name: &str) -> bool {
    const FAMILIES: &[&str] = &[
        "ruby",
        "rb",
        "julia",
        "jl",
        "matlab",
        "octave",
        "bash",
        "sage",
        "rust",
        "rs",
        "R",
        "perl",
        "pl",
        "perlsix",
        "psix",
        "javascript",
        "js",
    ];
    if matches!(
        name,
        "juliaconcode" | "juliaconsole" | "juliaconsole*" | "Rconcode" | "Rconsole" | "Rconsole*"
    ) {
        return true;
    }
    FAMILIES.iter().any(|family| {
        name.strip_prefix(family).is_some_and(|rest| {
            matches!(
                rest,
                "code" | "code*" | "block" | "block*" | "verbatim" | "verbatim*" | "sub"
            )
        })
    })
}

/// tree-sitter `displayed_equation` (`$$` or `\[`) / latexindent `displayMath` + `displayMathTeX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayMathDelim {
    Bracket,
    Dollars,
}

/// latexindent `displayMath` begin/end `(?<!\\)\\\[` / `(?<!\\)\\\]`.
/// `\\[2ex]` is a linebreak skip: the `[` sits after `\`, so it is not `\[`.
fn find_unescaped_display_bracket(
    s: &str,
    from: usize,
    closer: u8,
    extra_cmds: &[String],
) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(s, i, extra_cmds) {
                i = end;
                continue;
            }
            if i + 1 < bytes.len() && bytes[i + 1] == closer && (i == 0 || bytes[i - 1] != b'\\') {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// `$$` still opens only at leftover start (guev). `\[` may sit mid-line.
fn display_math_open_at(s: &str, extra_cmds: &[String]) -> Option<(usize, DisplayMathDelim)> {
    let t = s.trim_start();
    let lead = s.len() - t.len();
    if t.starts_with("$$") {
        return Some((lead, DisplayMathDelim::Dollars));
    }
    find_unescaped_display_bracket(s, 0, b'[', extra_cmds)
        .map(|pos| (pos, DisplayMathDelim::Bracket))
}

fn display_math_closes(s: &str, delim: DisplayMathDelim, extra_cmds: &[String]) -> Option<usize> {
    match delim {
        DisplayMathDelim::Bracket => find_unescaped_display_bracket(s, 0, b']', extra_cmds),
        DisplayMathDelim::Dollars => {
            let t = s.trim_end();
            t.ends_with("$$").then_some(t.len() - 2)
        }
    }
}

/// A line that is only `$$` is an opener, not a one-line `$$...$$` block.
/// Bracket same-line `\[...\]` is closed in `consume_code_span`, not here.
fn display_math_is_single_line(s: &str, delim: DisplayMathDelim) -> bool {
    match delim {
        DisplayMathDelim::Bracket => false,
        DisplayMathDelim::Dollars => {
            let t = s.trim();
            t != "$$" && t.ends_with("$$")
        }
    }
}

/// Sectioning commands: the full physical line is Structure, including an
/// optional `[short title]` (tree-sitter `_section_part` brack_group then
/// curly_group). KOMA `\addsec`/`\addchap`/`\addpart` share that shape.
static SECTION_CMD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\s*\\(?:part|chapter|section|subsection|subsubsection|paragraph|subparagraph|addsec|addchap|addpart)\*?(?:\[[^\]]*\])?\{)([^}]*)(\}.*)$",
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
    /// inside `\verb` / `\lstinline` / `\spverb` / `\mintinline` / `\mint` /
    /// `\Verb` / `\SaveVerb` / `\piton` / `\lstinputlisting` /
    /// configured verbatim commands.
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

/// Byte offset of `\caption` / `\caption*` at or after `from`.
/// `\captionof` / `\captionsetup` are different control words.
fn find_caption_at(line: &str, from: usize, extra_cmds: &[String]) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = from;
    let stop = unescaped_percent_with(line, extra_cmds).unwrap_or(line.len());
    while i < stop {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(line, i, extra_cmds) {
                i = end;
                continue;
            }
            if let Some(name) = tex_cs_at(line, i) {
                if name == "\\caption" {
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

/// End of `\caption` or `\caption*` starting at `start`.
fn caption_cmd_end(line: &str, start: usize) -> usize {
    let after = start + "\\caption".len();
    if line.get(after..).is_some_and(|r| r.starts_with('*')) {
        after + 1
    } else {
        after
    }
}

/// After `\caption`/`\caption*`: optional `[short title]`, then the `{` of
/// the long argument (tree-sitter caption curly_group). None if `{` is
/// not on this slice.
fn caption_open_brace(line: &str, cmd_end: usize, stop: usize) -> Option<usize> {
    let mut j = cmd_end;
    while j < stop && matches!(line.as_bytes()[j], b' ' | b'\t') {
        j += 1;
    }
    if let Some(br) = skip_optional_brackets(line, j, stop) {
        j = br;
        while j < stop && matches!(line.as_bytes()[j], b' ' | b'\t') {
            j += 1;
        }
    }
    (line.as_bytes().get(j) == Some(&b'{')).then_some(j)
}

/// Matching `}` for the `{` at `open_at`, skipping `\{` / `\}` and `\verb`.
fn find_matching_curly(s: &str, open_at: usize, extra_cmds: &[String]) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0;
    let mut i = open_at;
    let stop = unescaped_percent_with(s, extra_cmds).unwrap_or(s.len());
    while i < stop {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(s, i, extra_cmds) {
                i = end;
                continue;
            }
            if i + 1 < stop {
                i += 2;
                continue;
            }
        }
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Brace depth remaining at `stop` after walking from `{` at `open_at`.
fn curly_depth_at(s: &str, open_at: usize, stop: usize, extra_cmds: &[String]) -> usize {
    let bytes = s.as_bytes();
    let mut depth = 0;
    let mut i = open_at;
    let stop = stop.min(s.len());
    while i < stop {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(s, i, extra_cmds) {
                i = end.min(stop);
                continue;
            }
            if i + 1 < stop {
                i += 2;
                continue;
            }
        }
        match bytes[i] {
            b'{' => depth += 1,
            b'}' if depth > 0 => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    depth
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

/// `\iffalse` in ordinary TeX, skipping `\verb` / `\lstinline` /
/// `\spverb` / `\mintinline` / `\mint` / `\Verb` / `\SaveVerb` /
/// `\piton` / `\lstinputlisting` spans.
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

/// listings.sty `\lstinputlisting[...]{file}` span, skipping other verb
/// commands so `\verb|\lstinputlisting{x}|` is not stolen (GitHub #391).
fn find_lstinputlisting_span(
    text: &str,
    from: usize,
    extra_cmds: &[String],
) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(text, i, extra_cmds) {
                if text[i + 1..].starts_with("lstinputlisting") {
                    return Some((i, end));
                }
                i = end;
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
    in_float_env: Option<String>,
    float_depth: usize,
    in_caption: bool,
    caption_depth: usize,
    await_caption_brace: bool,
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
        self.code_lang = if env_name == "minted" || env_name == "minted*" {
            MINTED_LANG_RE
                .captures(header_src)
                .map(|c| c.get(1).unwrap().as_str().to_string())
        } else if env_name == "lstlisting" || env_name == "lstlisting*" {
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

        if self.in_caption || self.await_caption_brace {
            self.consume_caption_line(line);
            return;
        }

        if self.in_non_prose_env.is_some() {
            self.consume_non_prose_line(line);
            return;
        }

        if self.in_float_env.is_some() {
            self.consume_float_line(line);
            return;
        }

        if let Some(delim) = self.in_display_math {
            self.flush();
            if let Some(close_at) =
                display_math_closes(line.text, delim, &self.parser.extra_verbatim_commands)
            {
                let after = close_at + 2;
                let end = thru_eol_if_blank_rest(line, after);
                self.regions.push(SpannedRegion::structure(
                    self.input,
                    ByteSpan::new(line.start, end),
                ));
                self.in_display_math = None;
                if !line.text[after..].trim().is_empty() {
                    self.consume_body_line(rest_line(line, after));
                }
                return;
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
            if line_done
                || self.in_code_env.is_some()
                || self.in_non_prose_env.is_some()
                || self.in_float_env.is_some()
                || self.in_caption
                || self.await_caption_brace
            {
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
        } else if self.in_code_env.is_none()
            && self.in_non_prose_env.is_none()
            && self.in_float_env.is_none()
        {
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

    /// Float chrome is Structure. `\caption` long curly_group is Prose.
    fn consume_float_line(&mut self, line: Line<'_>) {
        if self.in_caption || self.await_caption_brace {
            self.consume_caption_line(line);
            return;
        }
        let name = self
            .in_float_env
            .as_deref()
            .expect("consume_float_line only when inside")
            .to_string();
        let extra = &self.parser.extra_verbatim_commands;

        if line.text.trim().is_empty() {
            self.flush();
            self.regions
                .push(SpannedRegion::blank(self.input, line.span()));
            return;
        }
        if LatexParser::is_comment(line.text) {
            self.flush();
            self.regions
                .push(SpannedRegion::structure(self.input, line.span()));
            return;
        }

        let mut i = 0;
        while i < line.text.len() {
            let env = find_env_at(line.text, i, extra);
            let cap = find_caption_at(line.text, i, extra);
            let env_first = match (env.as_ref(), cap) {
                (Some(hit), Some(c)) => hit.start <= c,
                (Some(_), None) => true,
                _ => false,
            };
            if env_first {
                let hit = env.expect("env_first implies find_env_at");
                if hit.start > i {
                    self.push_structure(ByteSpan::new(line.start + i, line.start + hit.start));
                }
                if hit.name == name {
                    if hit.is_begin {
                        self.float_depth += 1;
                        self.push_structure(ByteSpan::new(
                            line.start + hit.start,
                            line.start + hit.end,
                        ));
                        i = hit.end;
                        continue;
                    }
                    self.float_depth -= 1;
                    if self.float_depth == 0 {
                        let end = thru_eol_if_blank_rest(line, hit.end);
                        self.push_structure(ByteSpan::new(line.start + hit.start, end));
                        self.in_float_env = None;
                        if !line.text[hit.end..].trim().is_empty() {
                            self.consume_body_line(rest_line(line, hit.end));
                        }
                        return;
                    }
                    self.push_structure(ByteSpan::new(
                        line.start + hit.start,
                        line.start + hit.end,
                    ));
                    i = hit.end;
                    continue;
                }
                if hit.is_begin && self.parser.is_non_prose_env(&hit.name) {
                    if let Some(end_at) = find_matching_end(line.text, hit.end, &hit.name, 1, extra)
                    {
                        self.push_structure(ByteSpan::new(
                            line.start + hit.start,
                            line.start + end_at,
                        ));
                        i = end_at;
                        continue;
                    }
                    self.in_non_prose_env = Some(hit.name);
                    self.non_prose_depth = 1;
                    self.push_structure(ByteSpan::new(line.start + hit.start, line.end));
                    return;
                }
                if hit.is_begin && self.parser.is_code_env(&hit.name) {
                    if let Some(end_at) = find_matching_raw_end(line.text, hit.end, &hit.name, 1) {
                        let header = ByteSpan::new(line.start + hit.start, line.start + end_at);
                        let empty = ByteSpan::new(line.start + end_at, line.start + end_at);
                        self.flush();
                        self.regions
                            .push(SpannedRegion::code(self.input, None, header, empty, empty));
                        i = end_at;
                        continue;
                    }
                    self.enter_code(&hit.name, line, hit.start);
                    return;
                }
                self.push_structure(ByteSpan::new(line.start + hit.start, line.start + hit.end));
                i = hit.end;
                continue;
            }
            if let Some(c) = cap {
                if c > i {
                    self.push_structure(ByteSpan::new(line.start + i, line.start + c));
                }
                self.consume_caption_at(line, c);
                return;
            }
            self.push_structure(ByteSpan::new(line.start + i, line.end));
            return;
        }
    }

    fn consume_caption_at(&mut self, line: Line<'_>, cap_start: usize) {
        let extra = &self.parser.extra_verbatim_commands;
        let stop = unescaped_percent_with(line.text, extra).unwrap_or(line.text.len());
        let cmd_end = caption_cmd_end(line.text, cap_start);
        if let Some(open) = caption_open_brace(line.text, cmd_end, stop) {
            self.push_structure(ByteSpan::new(line.start + cap_start, line.start + open + 1));
            self.emit_caption_group(line, open);
            return;
        }
        self.push_structure(ByteSpan::new(line.start + cap_start, line.start + cmd_end));
        let mut j = cmd_end;
        while j < stop && matches!(line.text.as_bytes()[j], b' ' | b'\t') {
            j += 1;
        }
        if let Some(br) = skip_optional_brackets(line.text, j, stop) {
            self.push_structure(ByteSpan::new(line.start + cmd_end, line.start + br));
            j = br;
        }
        if j < stop && !line.text[j..stop].trim().is_empty() {
            self.push_structure(ByteSpan::new(line.start + j, line.start + stop));
        }
        if let Some(pct) = unescaped_percent_with(line.text, extra) {
            self.push_structure(ByteSpan::new(line.start + pct, line.end));
        } else if j < line.text.len() && line.text[j..].trim().is_empty() {
            self.push_structure(ByteSpan::new(line.start + j, line.end));
        }
        self.await_caption_brace = true;
    }

    fn emit_caption_group(&mut self, line: Line<'_>, open_at: usize) {
        let extra = &self.parser.extra_verbatim_commands;
        if let Some(close) = find_matching_curly(line.text, open_at, extra) {
            let inner_start = open_at + 1;
            if close > inner_start {
                self.append_prose_slice(line.start + inner_start, &line.text[inner_start..close]);
            }
            let after = close + 1;
            if line.text[after..].trim().is_empty() {
                self.push_structure(ByteSpan::new(line.start + close, line.end));
            } else {
                self.push_structure(ByteSpan::new(line.start + close, line.start + after));
                if self.in_float_env.is_some() {
                    self.consume_float_line(rest_line(line, after));
                } else {
                    self.consume_body_line(rest_line(line, after));
                }
            }
            return;
        }
        let stop = unescaped_percent_with(line.text, extra).unwrap_or(line.text.len());
        let inner_start = open_at + 1;
        if stop > inner_start {
            self.append_prose_slice(line.start + inner_start, &line.text[inner_start..stop]);
        }
        self.in_caption = true;
        self.caption_depth = curly_depth_at(line.text, open_at, stop, extra);
        if let Some(pct) = unescaped_percent_with(line.text, extra) {
            self.flush();
            self.regions.push(SpannedRegion::structure(
                self.input,
                ByteSpan::new(line.start + pct, line.end),
            ));
        } else {
            self.extend_prose_to(line.end);
        }
    }

    fn consume_caption_line(&mut self, line: Line<'_>) {
        let extra = &self.parser.extra_verbatim_commands;
        if self.await_caption_brace {
            if line.text.trim().is_empty() {
                self.flush();
                self.regions
                    .push(SpannedRegion::blank(self.input, line.span()));
                return;
            }
            if LatexParser::is_comment(line.text) {
                self.flush();
                self.regions
                    .push(SpannedRegion::structure(self.input, line.span()));
                return;
            }
            let stop = unescaped_percent_with(line.text, extra).unwrap_or(line.text.len());
            let mut j = 0;
            while j < stop && matches!(line.text.as_bytes()[j], b' ' | b'\t') {
                j += 1;
            }
            if line.text.as_bytes().get(j) == Some(&b'{') {
                self.await_caption_brace = false;
                if j > 0 {
                    self.push_structure(ByteSpan::new(line.start, line.start + j + 1));
                } else {
                    self.push_structure(ByteSpan::new(line.start, line.start + 1));
                }
                self.emit_caption_group(line, j);
                return;
            }
            self.push_structure(line.span());
            return;
        }

        if line.text.trim().is_empty() {
            if let Some(end) = self.prose_span.as_ref().map(|s| s.end) {
                self.extend_prose_to(end);
            }
            self.append_prose_slice(line.start, "");
            self.flush();
            self.regions
                .push(SpannedRegion::blank(self.input, line.span()));
            return;
        }

        let stop = unescaped_percent_with(line.text, extra).unwrap_or(line.text.len());
        let bytes = line.text.as_bytes();
        let mut i = 0;
        let mut depth = self.caption_depth;
        while i < stop {
            if bytes[i] == b'\\' {
                if let Some(end) = latex_verb_span_end_with(line.text, i, extra) {
                    i = end;
                    continue;
                }
                if i + 1 < stop {
                    i += 2;
                    continue;
                }
            }
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    if depth == 0 {
                        i += 1;
                        continue;
                    }
                    depth -= 1;
                    if depth == 0 {
                        if i > 0 {
                            self.append_prose_slice(line.start, &line.text[..i]);
                        }
                        self.in_caption = false;
                        self.caption_depth = 0;
                        let after = i + 1;
                        if line.text[after..].trim().is_empty() {
                            self.push_structure(ByteSpan::new(line.start + i, line.end));
                        } else {
                            self.push_structure(ByteSpan::new(line.start + i, line.start + after));
                            if self.in_float_env.is_some() {
                                self.consume_float_line(rest_line(line, after));
                            } else {
                                self.consume_body_line(rest_line(line, after));
                            }
                        }
                        return;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if stop > 0 {
            self.append_prose_slice(line.start, &line.text[..stop]);
        }
        self.caption_depth = depth;
        if let Some(pct) = unescaped_percent_with(line.text, extra) {
            self.flush();
            self.regions.push(SpannedRegion::structure(
                self.input,
                ByteSpan::new(line.start + pct, line.end),
            ));
        } else {
            self.extend_prose_to(line.end);
        }
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
                if hit.is_begin && is_float_env(&hit.name) {
                    self.flush();
                    let begin_end = thru_eol_if_blank_rest(line, hit.end);
                    self.push_structure(ByteSpan::new(line.start + hit.start, begin_end));
                    self.in_float_env = Some(hit.name);
                    self.float_depth = 1;
                    if !line.text[hit.end..].trim().is_empty() {
                        self.consume_float_line(rest_line(line, hit.end));
                    }
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
            if let Some((start, end)) =
                find_lstinputlisting_span(code, i, &self.parser.extra_verbatim_commands)
            {
                self.append_item_or_prose(line.start + i, &code[i..start]);
                let cmd_end = if code[end..].trim().is_empty() {
                    thru_eol_if_blank_rest(line, end)
                } else {
                    line.start + end
                };
                self.push_structure(ByteSpan::new(line.start + start, cmd_end));
                i = end;
                continue;
            }
            if SECTION_CMD_RE.is_match(rest) {
                self.push_structure(ByteSpan::new(line.start + i, line.end));
                return false;
            }
            if let Some((rel, delim)) =
                display_math_open_at(rest, &self.parser.extra_verbatim_commands)
            {
                self.append_item_or_prose(line.start + i, &rest[..rel]);
                self.flush();
                if delim == DisplayMathDelim::Dollars {
                    if !display_math_is_single_line(&rest[rel..], delim) {
                        self.in_display_math = Some(delim);
                    }
                    self.regions.push(SpannedRegion::structure(
                        self.input,
                        ByteSpan::new(line.start + i + rel, line.end),
                    ));
                    return false;
                }
                // Glue space stays on the `\[` island so reflow does not
                // break `inducing \[` / `See also \[ a = 1. \]`.
                // Pure indent before leftover-start `\[` stays on the island.
                let open_rel = if rest[..rel].trim().is_empty() {
                    0
                } else {
                    let prefix = &rest[..rel];
                    let glue = prefix.len() - prefix.trim_end_matches([' ', '\t']).len();
                    rel - glue
                };
                let after_open = rel + 2;
                if let Some(close_rel) = find_unescaped_display_bracket(
                    rest,
                    after_open,
                    b']',
                    &self.parser.extra_verbatim_commands,
                ) {
                    let after_close = close_rel + 2;
                    let end = if rest[after_close..].trim().is_empty() {
                        thru_eol_if_blank_rest(line, i + after_close)
                    } else {
                        line.start + i + after_close
                    };
                    self.regions.push(SpannedRegion::structure(
                        self.input,
                        ByteSpan::new(line.start + i + open_rel, end),
                    ));
                    i += after_close;
                    continue;
                }
                self.in_display_math = Some(delim);
                self.regions.push(SpannedRegion::structure(
                    self.input,
                    ByteSpan::new(line.start + i + open_rel, line.end),
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
            in_float_env: None,
            float_depth: 0,
            in_caption: false,
            caption_depth: 0,
            await_caption_brace: false,
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
    fn section_optional_short_title_is_structure_not_prose() {
        let input =
            "\\section[Short. Title.]{A long title. With two sentences.}\nBody. More body.\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains(r"\section[Short. Title.]{A long title. With two sentences.}")
            )),
            "full section line including optional short title must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("Short. Title")
                        || p.contains("A long title")
                        || p.contains("With two sentences")
            )),
            "optional short title and long title must not be Prose: {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(prose, ["Body. More body."]);
    }

    #[test]
    fn section_optional_short_title_stays_one_line() {
        use crate::format::Format;
        use crate::oracle;
        use crate::{FormatConfig, format_text};

        let input =
            "\\section[Short. Title.]{A long title. With two sentences.}\nBody. More body.\n";
        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("\\section[Short. Title.]{A long title. With two sentences.}"),
            "section line including optional short title must stay one line, got:\n{out}"
        );
        assert!(
            !out.contains("\\section[Short.\n") && !out.contains("Title.]{A long title.\n"),
            "must not split periods in the short title or long title:\n{out}"
        );
        assert!(
            out.contains("Body.\nMore body."),
            "body after the section line must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
        assert!(
            oracle::matches(Format::Latex, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }

    #[test]
    fn koma_addsec_addchap_addpart_optional_short_title_is_structure() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Latex,
            ..Default::default()
        }
        .without_safety_backstops();
        for cmd in [r"\addsec", r"\addchap", r"\addpart", r"\section*"] {
            let input = format!(
                "{cmd}[Short. Title.]{{A long title. With two sentences.}}\nBody. More body.\n"
            );
            let regions = LatexParser::default().parse(&input);
            let needle = format!("{cmd}[Short. Title.]{{A long title. With two sentences.}}");
            assert!(
                regions.iter().any(|r| matches!(
                    r,
                    Region::Structure(s) if s.contains(&needle)
                )),
                "{cmd} line including optional short title must be Structure, got: {regions:?}"
            );
            assert!(
                !regions.iter().any(|r| matches!(
                    r,
                    Region::Prose(p)
                        if p.contains("Short. Title") || p.contains("A long title")
                )),
                "{cmd} short/long title must not be Prose: {regions:?}"
            );
            let out = format_text(&input, &cfg).unwrap();
            assert!(
                out.contains(&needle),
                "{cmd} optional short title must stay one line, got:\n{out}"
            );
            assert!(
                out.contains("Body.\nMore body."),
                "{cmd} body after section must still reflow, got:\n{out}"
            );
            assert_eq!(format_text(&out, &cfg).unwrap(), out);
        }
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

    /// pythontex.sty option-family env names from `usefamily` /
    /// `\makepythontexfamily` (GitHub #352).
    fn pythontex_option_family_env_names() -> Vec<String> {
        const FAMILIES: &[&str] = &[
            "ruby",
            "rb",
            "julia",
            "jl",
            "matlab",
            "octave",
            "bash",
            "sage",
            "rust",
            "rs",
            "R",
            "perl",
            "pl",
            "perlsix",
            "psix",
            "javascript",
            "js",
        ];
        let mut names = Vec::new();
        for family in FAMILIES {
            for suffix in ["code", "block", "verbatim"] {
                names.push(format!("{family}{suffix}"));
                names.push(format!("{family}{suffix}*"));
            }
            names.push(format!("{family}sub"));
        }
        names.extend([
            "juliaconcode".into(),
            "juliaconsole".into(),
            "juliaconsole*".into(),
            "Rconcode".into(),
            "Rconsole".into(),
            "Rconsole*".into(),
        ]);
        names
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
    fn fancyvrb_verb_with_inner_punct_round_trips() {
        use crate::format_text;

        // GitHub #243 fixture: `\Verb|a.b! c|` is one token; Next sentence. splits.
        let input = "\\begin{document}\nUse \\Verb|a.b! c| here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\Verb|a.b! c|"),
            "Verb must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("\\Verb|a.\n") && !out.contains("\\Verb|a.b!\n"),
            "inner .!? must not split Verb, got:\n{out}"
        );
        assert!(
            out.contains("Use \\Verb|a.b! c| here.\nNext sentence."),
            "prose after Verb must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn fancyvrb_verb_star_with_inner_punct_round_trips() {
        use crate::format_text;

        let input =
            "\\begin{document}\nUse \\Verb*|a.b! c| here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\Verb*|a.b! c|"),
            "Verb* must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("\\Verb*|a.\n") && !out.contains("\\Verb*|a.b!\n"),
            "inner .!? must not split Verb*, got:\n{out}"
        );
        assert!(
            out.contains("Use \\Verb*|a.b! c| here.\nNext sentence."),
            "prose after Verb* must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn fancyvrb_verb_inner_percent_is_not_a_comment() {
        use crate::format_text;

        let input = "\\begin{document}\nCode \\Verb!%! here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\Verb!%!"),
            "Verb with inner % must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("here."),
            "text after Verb must not be commented out, got:\n{out}"
        );
        assert!(
            out.contains("Code \\Verb!%! here.\nNext sentence."),
            "prose after Verb must still split, got:\n{out}"
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            !regions.iter().any(
                |r| matches!(r, Region::Structure(s) if s.contains("%!") || s.trim() == "%!\n")
            ),
            "inner % of Verb must not be a comment, got: {regions:?}"
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

    /// Ticket fixture (GitHub #391): listings.sty `\lstinputlisting{file}`
    /// is one atomic command. Following flush prose does not join onto
    /// the command line. `After.` / `Next.` still split. `lstinline` /
    /// `lstlisting` unchanged.
    #[test]
    fn lstinputlisting_does_not_join_following_prose() {
        use crate::format_text;

        let input = concat!(
            "Before. Next.\n",
            "\\lstinputlisting{foo.py}\n",
            "After. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(r"\lstinputlisting{foo.py}")
            )),
            "lstinputlisting must stay one Structure command, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains(r"\lstinputlisting{foo.py}")
            )),
            "lstinputlisting must not leak into Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("After.") && p.contains("Next.")
            )),
            "After. / Next. must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\lstinputlisting{foo.py}\n"),
            "lstinputlisting must stay one atomic command, got:\n{out}"
        );
        assert!(
            !out.contains("\\lstinputlisting{foo.py} After."),
            "following flush prose must not join the command line, got:\n{out}"
        );
        assert!(
            out.contains("Before.\nNext."),
            "prose before lstinputlisting must still split, got:\n{out}"
        );
        assert!(
            out.contains("After.\nNext."),
            "prose after lstinputlisting must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let opts = concat!(
            "Before. Next.\n",
            "\\lstinputlisting[language=Python]{foo.py}\n",
            "After. Next.\n",
        );
        let opts_out = format_text(opts, &latex_cfg()).unwrap();
        assert!(
            opts_out.contains("\\lstinputlisting[language=Python]{foo.py}\n"),
            "lstinputlisting optional args must stay atomic, got:\n{opts_out}"
        );
        assert!(
            !opts_out.contains("\\lstinputlisting[language=Python]{foo.py} After."),
            "optional-arg lstinputlisting must not join following prose, got:\n{opts_out}"
        );
        assert!(
            opts_out.contains("After.\nNext."),
            "prose after optional-arg lstinputlisting must still split, got:\n{opts_out}"
        );

        let lstinline = "Use \\lstinline!a.b! here. Next sentence.\n";
        let lstinline_out = format_text(lstinline, &latex_cfg()).unwrap();
        assert!(
            lstinline_out.contains("Use \\lstinline!a.b! here.\nNext sentence."),
            "lstinline must stay intact and still split, got:\n{lstinline_out}"
        );

        let lstlisting = concat!(
            "\\begin{lstlisting}\n",
            "First line. Second line.\n",
            "\\end{lstlisting}\n",
            "After the block. Next.\n",
        );
        let lstlisting_out = format_text(lstlisting, &latex_cfg()).unwrap();
        assert!(
            lstlisting_out
                .contains("\\begin{lstlisting}\nFirst line. Second line.\n\\end{lstlisting}"),
            "lstlisting must stay a code env, got:\n{lstlisting_out}"
        );
        assert!(
            lstlisting_out.contains("After the block.\nNext."),
            "prose after lstlisting must still split, got:\n{lstlisting_out}"
        );
        assert_eq!(
            format_text(&lstlisting_out, &latex_cfg()).unwrap(),
            lstlisting_out
        );
    }

    /// Ticket fixture (GitHub #245): minted `\mintinline{lang}|body|` is
    /// one token; following `Next sentence.` still splits.
    #[test]
    fn mintinline_lang_delim_round_trips() {
        use crate::format_text;

        let input = "\\begin{document}\nUse \\mintinline{python}|a.b! c| here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\mintinline{python}|a.b! c|"),
            "mintinline lang+delim must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("\\mintinline{python}|a.\n")
                && !out.contains("\\mintinline{python}|a.b!\n"),
            "inner .!? must not split mintinline, got:\n{out}"
        );
        assert!(
            out.contains("Use \\mintinline{python}|a.b! c| here.\nNext sentence."),
            "prose after mintinline must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn mintinline_lang_brace_body_round_trips() {
        use crate::format_text;

        let input = "\\begin{document}\nUse \\mintinline{python}{a.b! c} here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\mintinline{python}{a.b! c}"),
            "mintinline lang+brace body must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("Use \\mintinline{python}{a.b! c} here.\nNext sentence."),
            "prose after mintinline brace body must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn mint_lang_delim_round_trips() {
        use crate::format_text;

        let input =
            "\\begin{document}\nUse \\mint{python}|a.b! c| here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\mint{python}|a.b! c|"),
            "mint lang+delim must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("Use \\mint{python}|a.b! c| here.\nNext sentence."),
            "prose after mint must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn mintinline_inner_percent_is_not_a_comment() {
        use crate::format_text;

        let input = "\\begin{document}\nCode \\mintinline{python}|a%b| here. Next sentence.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\mintinline{python}|a%b|"),
            "mintinline with inner % must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("here."),
            "text after mintinline must not be commented out, got:\n{out}"
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            !regions.iter().any(
                |r| matches!(r, Region::Structure(s) if s.contains("%b|") || s.trim() == "%b|\n")
            ),
            "inner % of mintinline must not be a comment, got: {regions:?}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #275): fancyvrb `\SaveVerb{name}|body|` is
    /// one token; following `After.` still splits.
    #[test]
    fn fancyvrb_saveverb_name_delim_round_trips() {
        use crate::format_text;

        let input =
            "\\begin{document}\nUse \\SaveVerb{foo}|done. Next| here. After.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\SaveVerb{foo}|done. Next|"),
            "SaveVerb name+delim must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("\\SaveVerb{foo}|done.\n")
                && !out.contains("\\SaveVerb{foo}|done. Next|\n"),
            "inner . must not split SaveVerb, got:\n{out}"
        );
        assert!(
            out.contains("Use \\SaveVerb{foo}|done. Next| here.\nAfter."),
            "prose after SaveVerb must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn fancyvrb_saveverb_inner_percent_is_not_a_comment() {
        use crate::format_text;

        let input = "\\begin{document}\nCode \\SaveVerb{foo}|a%b| here. After.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\SaveVerb{foo}|a%b|"),
            "SaveVerb with inner % must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("here."),
            "text after SaveVerb must not be commented out, got:\n{out}"
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            !regions.iter().any(
                |r| matches!(r, Region::Structure(s) if s.contains("%b|") || s.trim() == "%b|\n")
            ),
            "inner % of SaveVerb must not be a comment, got: {regions:?}"
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

    /// Ticket fixture (GitHub #98 / snapper-3tj3): fancyvrb Verbatim is Code.
    #[test]
    fn fancyvrb_verbatim_fixture_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{Verbatim}\n",
            "First line. Second line.\n",
            "\\end{Verbatim}\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "Verbatim body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "Verbatim body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("First line. Second line."),
            "Verbatim body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "Verbatim must stay verbatim, got:\n{out}"
        );
        assert_eq!(out, input, "Verbatim env must be identity, got:\n{out}");
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #209): fancyvrb BVerbatim and LVerbatim
    /// are the same raw class as Verbatim.
    #[test]
    fn fancyvrb_bverbatim_lverbatim_are_code_not_prose() {
        use crate::format_text;

        for name in ["BVerbatim", "LVerbatim"] {
            let input = format!(
                "\\begin{{document}}\nBefore the listing. More before.\n\\begin{{{name}}}\nThis is a long sentence that must not reflow as prose inside {name}.\n\\end{{{name}}}\nAfter the listing. Second sentence.\n\\end{{document}}\n"
            );
            let regions = LatexParser::default().parse(&input);
            assert!(
                regions.iter().any(|r| matches!(
                    r,
                    Region::Code { body, .. }
                        if body.contains("This is a long sentence that must not reflow as prose")
                )),
                "{name} body must be Code, got: {regions:?}"
            );
            assert!(
                !regions.iter().any(|r| matches!(
                    r,
                    Region::Prose(p)
                        if p.contains("This is a long sentence that must not reflow as prose")
                )),
                "{name} body must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains(&format!(
                    "This is a long sentence that must not reflow as prose inside {name}."
                )),
                "{name} body must not reflow, got:\n{out}"
            );
            assert!(
                !out.contains("This is a long sentence that must not reflow as prose inside\n"),
                "{name} must stay one source line, got:\n{out}"
            );
            assert!(
                out.contains("After the listing.\nSecond sentence."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

            // Two interior sentences must stay one source line (default
            // max_width=0 only splits Prose).
            let two = format!("\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\n");
            let two_out = format_text(&two, &latex_cfg()).unwrap();
            assert_eq!(two_out, two, "{name} env must be identity, got:\n{two_out}");
            assert!(
                !two_out.contains("First line.\nSecond line."),
                "{name} two-sentence body must not split, got:\n{two_out}"
            );
        }
    }

    /// Ticket fixture (GitHub #244): fancyvrb `Verbatim*` / `BVerbatim*`
    /// / `LVerbatim*` are the starred twins of the unstarred FV@Scan
    /// names. Body stays Code; following prose still splits.
    #[test]
    fn fancyvrb_verbatim_star_envs_are_code_not_prose() {
        use crate::format_text;

        for name in ["Verbatim*", "BVerbatim*", "LVerbatim*"] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (GitHub #213): fancyvrb SaveVerbatim and
    /// VerbatimOut are the same FV@Scan class as Verbatim. The required
    /// `{name}` / `{file}` argument stays on the begin header.
    #[test]
    fn fancyvrb_saveverbatim_verbatimout_are_code_not_prose() {
        use crate::format_text;

        for name in ["SaveVerbatim", "VerbatimOut"] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}{{foo}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
            );
            let regions = LatexParser::default().parse(&input);
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
                panic!("{name} must be Code, got: {regions:?}");
            };
            assert!(
                header.contains(&format!("\\begin{{{name}}}{{foo}}")),
                "{name} required arg must stay on the begin header, got header={header:?}"
            );
            assert!(
                body.contains("First line. Second line."),
                "{name} body must keep both sentences, got body={body:?}"
            );
            assert!(
                footer.contains(&format!("\\end{{{name}}}")),
                "{name} footer must be \\end{{{name}}}, got footer={footer:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
                "{name} body must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains(&format!("\\begin{{{name}}}{{foo}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (GitHub #308): verbments.sty `pyglist` wraps
    /// fancyvrb VerbatimOut; the body is a raw listing. Optional
    /// `[language=python]` stays on begin; body stays Code; following
    /// prose still splits. Landed VerbatimOut / minted stay Code.
    #[test]
    fn verbments_pyglist_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{pyglist}[language=python]\n",
            "First line. Second line.\n",
            "\\end{pyglist}\n",
            "After the block. Next.\n",
        );
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
            panic!("pyglist must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{pyglist}[language=python]"),
            "optional language arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "pyglist body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{pyglist}"),
            "pyglist footer must be \\end{{pyglist}}, got footer={footer:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "pyglist body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{pyglist}[language=python]") && out.contains(r"\end{pyglist}"),
            "pyglist begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "pyglist body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "pyglist must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after pyglist must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let verbatimout = concat!(
            "\\begin{VerbatimOut}{foo}\n",
            "First line. Second line.\n",
            "\\end{VerbatimOut}\n",
            "After the block. Next.\n",
        );
        let verbatimout_out = format_text(verbatimout, &latex_cfg()).unwrap();
        assert!(
            verbatimout_out.contains(
                "\\begin{VerbatimOut}{foo}\nFirst line. Second line.\n\\end{VerbatimOut}"
            ),
            "landed VerbatimOut must stay a code env, got:\n{verbatimout_out}"
        );
        assert!(
            verbatimout_out.contains("After the block.\nNext."),
            "prose after landed VerbatimOut must still split, got:\n{verbatimout_out}"
        );
        assert_eq!(
            format_text(&verbatimout_out, &latex_cfg()).unwrap(),
            verbatimout_out
        );

        let minted = concat!(
            "\\begin{minted}{python}\n",
            "First line. Second line.\n",
            "\\end{minted}\n",
            "After the block. Next.\n",
        );
        let minted_out = format_text(minted, &latex_cfg()).unwrap();
        assert!(
            minted_out.contains("\\begin{minted}{python}\nFirst line. Second line.\n\\end{minted}"),
            "landed minted must stay a code env, got:\n{minted_out}"
        );
        assert!(
            minted_out.contains("After the block.\nNext."),
            "prose after landed minted must still split, got:\n{minted_out}"
        );
        assert_eq!(format_text(&minted_out, &latex_cfg()).unwrap(), minted_out);
    }

    /// Ticket fixture (GitHub #342): texments.sty / pygmentex.sty
    /// `pygmented` is `VerbatimEnvironment` plus `VerbatimOut`. Required
    /// `{lang}` stays on the begin header; body stays Code; following
    /// prose still splits. Landed pygments / pyglist stay Code.
    #[test]
    fn texments_pygmented_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{pygmented}{python}\n",
            "First line. Second line.\n",
            "\\end{pygmented}\n",
            "After the block. Next.\n",
        );
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
            panic!("pygmented must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{pygmented}{python}"),
            "required lexer arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "pygmented body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{pygmented}"),
            "pygmented footer must stay, got footer={footer:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("First line") || p.contains("{python}")
            )),
            "pygmented lexer/body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{pygmented}{python}") && out.contains(r"\end{pygmented}"),
            "pygmented begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "pygmented body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "pygmented must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after pygmented must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let pygments = concat!(
            "\\begin{pygments}{python}\n",
            "First line. Second line.\n",
            "\\end{pygments}\n",
            "After the block. Next.\n",
        );
        let pygments_out = format_text(pygments, &latex_cfg()).unwrap();
        assert!(
            pygments_out
                .contains("\\begin{pygments}{python}\nFirst line. Second line.\n\\end{pygments}"),
            "landed pygments must stay a code env, got:\n{pygments_out}"
        );
        assert!(
            pygments_out.contains("After the block.\nNext."),
            "prose after landed pygments must still split, got:\n{pygments_out}"
        );
        assert_eq!(
            format_text(&pygments_out, &latex_cfg()).unwrap(),
            pygments_out
        );

        let pyglist = concat!(
            "\\begin{pyglist}[language=python]\n",
            "First line. Second line.\n",
            "\\end{pyglist}\n",
            "After the block. Next.\n",
        );
        let pyglist_out = format_text(pyglist, &latex_cfg()).unwrap();
        assert!(
            pyglist_out.contains(
                "\\begin{pyglist}[language=python]\nFirst line. Second line.\n\\end{pyglist}"
            ),
            "landed pyglist must stay a code env, got:\n{pyglist_out}"
        );
        assert!(
            pyglist_out.contains("After the block.\nNext."),
            "prose after landed pyglist must still split, got:\n{pyglist_out}"
        );
        assert_eq!(
            format_text(&pyglist_out, &latex_cfg()).unwrap(),
            pyglist_out
        );
    }

    /// Ticket fixture (GitHub #247): fvextra VerbatimWrite is the same
    /// FV@Scan class as fancyvrb VerbatimOut. The required `{file}`
    /// argument stays on the begin header.
    #[test]
    fn fancyvrb_verbatimwrite_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{VerbatimWrite}{out.tex}\n",
            "First line. Second line.\n",
            "\\end{VerbatimWrite}\n",
            "After the block. Next.\n",
        );
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
            panic!("VerbatimWrite must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{VerbatimWrite}{out.tex}"),
            "VerbatimWrite required arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "VerbatimWrite body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{VerbatimWrite}"),
            "VerbatimWrite footer must be \\end{{VerbatimWrite}}, got footer={footer:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "VerbatimWrite body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{VerbatimWrite}{out.tex}") && out.contains(r"\end{VerbatimWrite}"),
            "VerbatimWrite begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "VerbatimWrite body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "VerbatimWrite must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after VerbatimWrite must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #292): fvextra VerbatimBuffer is the same
    /// raw grab as VerbatimWrite (detokenize buffer). Body stays Code;
    /// following prose still splits.
    #[test]
    fn fvextra_verbatimbuffer_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{VerbatimBuffer}\n",
            "First line. Second line.\n",
            "\\end{VerbatimBuffer}\n",
            "After the block. Next.\n",
        );
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
            panic!("VerbatimBuffer must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{VerbatimBuffer}"),
            "VerbatimBuffer begin must stay on the header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "VerbatimBuffer body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{VerbatimBuffer}"),
            "VerbatimBuffer footer must be \\end{{VerbatimBuffer}}, got footer={footer:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "VerbatimBuffer body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{VerbatimBuffer}") && out.contains(r"\end{VerbatimBuffer}"),
            "VerbatimBuffer begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "VerbatimBuffer body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "VerbatimBuffer must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after VerbatimBuffer must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #293): fvextra VerbEnv is the environment
    /// form of Verb (single-line raw body, closer on its own line).
    /// Body stays Code; following prose still splits.
    #[test]
    fn fvextra_verbenv_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{VerbEnv}\n",
            "First line. Second line.\n",
            "\\end{VerbEnv}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "VerbEnv body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "VerbEnv body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{VerbEnv}") && out.contains(r"\end{VerbEnv}"),
            "VerbEnv begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "VerbEnv body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "VerbEnv must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after VerbEnv must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #249): pythontex.sty `pyblock` /
    /// `pyverbatim` / `pyconsole` are the same `VerbatimEnvironment`
    /// class as `pycode`. Body stays Code; following prose still splits.
    #[test]
    fn pythontex_pyblock_pyverbatim_pyconsole_are_code_not_prose() {
        use crate::format_text;

        for name in ["pyblock", "pyverbatim", "pyconsole"] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (GitHub #276): pythontex.sty `pycode*` /
    /// `pyblock*` / `pyverbatim*` / `pyconsole*` are the same
    /// `VerbatimEnvironment` class as the unstarred twins. Body stays
    /// Code; following prose still splits.
    #[test]
    fn pythontex_starred_py_envs_are_code_not_prose() {
        use crate::format_text;

        for name in ["pycode*", "pyblock*", "pyverbatim*", "pyconsole*"] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (GitHub #274): pythontex.sty `pygments` is the
    /// same `VerbatimEnvironment` class as `pycode`. Required `{lang}`
    /// stays on the begin header; body stays Code; following prose
    /// still splits.
    #[test]
    fn pythontex_pygments_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{pygments}{python}\n",
            "First line. Second line.\n",
            "\\end{pygments}\n",
            "After the block. Next.\n",
        );
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
            panic!("pygments must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{pygments}{python}"),
            "required lexer arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "pygments body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{pygments}"),
            "pygments footer must stay, got footer={footer:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("First line") || p.contains("{python}")
            )),
            "pygments lexer/body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{pygments}{python}") && out.contains(r"\end{pygments}"),
            "pygments begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "pygments body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "pygments must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after pygments must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #277): pythontex.sty `sympycode` /
    /// `sympyblock` / `sympyverbatim` / `sympyconsole` and starred
    /// twins are the same `VerbatimEnvironment` class as `pycode`.
    /// Body stays Code; following prose still splits.
    #[test]
    fn pythontex_sympy_family_is_code_not_prose() {
        use crate::format_text;

        for name in [
            "sympycode",
            "sympycode*",
            "sympyblock",
            "sympyblock*",
            "sympyverbatim",
            "sympyverbatim*",
            "sympyconsole",
            "sympyconsole*",
        ] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (GitHub #278): pythontex.sty `pylabcode` /
    /// `pylabblock` / `pylabverbatim` / `pylabconsole` and starred
    /// twins are the same `VerbatimEnvironment` class as `pycode`.
    /// Body stays Code; following prose still splits.
    #[test]
    fn pythontex_pylab_family_is_code_not_prose() {
        use crate::format_text;

        for name in [
            "pylabcode",
            "pylabcode*",
            "pylabblock",
            "pylabblock*",
            "pylabverbatim",
            "pylabverbatim*",
            "pylabconsole",
            "pylabconsole*",
        ] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        let pycode = concat!(
            "\\begin{pycode}\n",
            "First line. Second line.\n",
            "\\end{pycode}\n",
            "After the block. Next.\n",
        );
        let pycode_out = format_text(pycode, &latex_cfg()).unwrap();
        assert!(
            pycode_out.contains("\\begin{pycode}\nFirst line. Second line.\n\\end{pycode}"),
            "unstarred pycode family must stay a code env, got:\n{pycode_out}"
        );
        assert!(
            pycode_out.contains("After the block.\nNext."),
            "prose after unstarred pycode must still split, got:\n{pycode_out}"
        );
        assert_eq!(format_text(&pycode_out, &latex_cfg()).unwrap(), pycode_out);
    }

    /// Ticket fixture (GitHub #333): leftover pythontex.sty
    /// default-family envs (`pyconcode` / `pyconverbatim` / `pysub` /
    /// `pyconsub` / `sympycon*` / `pylabcon*` / `pythontexcustomcode`)
    /// are the same `VerbatimEnvironment` class as landed `pycode` /
    /// `pyconsole`. Body stays Code; following prose still splits.
    #[test]
    fn pythontex_leftover_default_family_envs_are_code_not_prose() {
        use crate::format_text;

        for name in [
            "pyconcode",
            "pyconverbatim",
            "pysub",
            "pyconsub",
            "sympyconcode",
            "sympyconverbatim",
            "sympysub",
            "sympyconsub",
            "pylabconcode",
            "pylabconverbatim",
            "pylabsub",
            "pylabconsub",
            "pythontexcustomcode",
        ] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        let custom = concat!(
            "\\begin{pythontexcustomcode}{py}\n",
            "First line. Second line.\n",
            "\\end{pythontexcustomcode}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(custom);
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
            panic!("pythontexcustomcode must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{pythontexcustomcode}{py}"),
            "required family arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "pythontexcustomcode body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{pythontexcustomcode}"),
            "pythontexcustomcode footer must stay, got footer={footer:?}"
        );
        let custom_out = format_text(custom, &latex_cfg()).unwrap();
        assert!(
            custom_out.contains("\\begin{pythontexcustomcode}{py}\nFirst line. Second line.\n\\end{pythontexcustomcode}"),
            "pythontexcustomcode required arg must stay on begin, got:\n{custom_out}"
        );
        assert!(
            custom_out.contains("After the block.\nNext."),
            "prose after pythontexcustomcode must still split, got:\n{custom_out}"
        );
        assert_eq!(format_text(&custom_out, &latex_cfg()).unwrap(), custom_out);

        let landed = concat!(
            "\\begin{pycode}\n",
            "First line. Second line.\n",
            "\\end{pycode}\n",
            "\\begin{pyconsole}\n",
            "First line. Second line.\n",
            "\\end{pyconsole}\n",
            "After the block. Next.\n",
        );
        let landed_out = format_text(landed, &latex_cfg()).unwrap();
        assert!(
            landed_out.contains("\\begin{pycode}\nFirst line. Second line.\n\\end{pycode}"),
            "landed pycode must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("\\begin{pyconsole}\nFirst line. Second line.\n\\end{pyconsole}"),
            "landed pyconsole must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("After the block.\nNext."),
            "prose after landed pycode/pyconsole must still split, got:\n{landed_out}"
        );
        assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
    }

    /// Ticket fixture (GitHub #352): pythontex.sty option-family
    /// `usefamily` / `\makepythontexfamily` envs (`rubycode`
    /// representative) are the same `VerbatimEnvironment` class as
    /// landed `pycode` / `pyconsole`. Body stays Code; following prose
    /// still splits. Default-family leftovers stay Code.
    #[test]
    fn pythontex_option_family_envs_are_code_not_prose() {
        use crate::format_text;

        for name in pythontex_option_family_env_names() {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        let landed = concat!(
            "\\begin{pycode}\n",
            "First line. Second line.\n",
            "\\end{pycode}\n",
            "\\begin{pyconsole}\n",
            "First line. Second line.\n",
            "\\end{pyconsole}\n",
            "\\begin{pyconcode}\n",
            "First line. Second line.\n",
            "\\end{pyconcode}\n",
            "After the block. Next.\n",
        );
        let landed_out = format_text(landed, &latex_cfg()).unwrap();
        assert!(
            landed_out.contains("\\begin{pycode}\nFirst line. Second line.\n\\end{pycode}"),
            "landed pycode must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("\\begin{pyconsole}\nFirst line. Second line.\n\\end{pyconsole}"),
            "landed pyconsole must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("\\begin{pyconcode}\nFirst line. Second line.\n\\end{pyconcode}"),
            "leftover default-family pyconcode must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("After the block.\nNext."),
            "prose after landed pycode/pyconsole must still split, got:\n{landed_out}"
        );
        assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
    }

    /// Ticket fixture (GitHub #230): alltt.sty is a standard
    /// verbatim-like env. Body stays Code; following prose still splits.
    #[test]
    fn alltt_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{document}\n",
            "Before. More.\n",
            "\\begin{alltt}\n",
            "First line. Second line.\n",
            "\\end{alltt}\n",
            "After the block. Next.\n",
            "\\end{document}\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "alltt body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "alltt body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{alltt}") && out.contains("\\end{alltt}"),
            "alltt begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "alltt body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "alltt must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after alltt must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let two = concat!(
            "\\begin{alltt}\n",
            "First line. Second line.\n",
            "\\end{alltt}\n",
            "After the block. Next.\n",
        );
        let two_out = format_text(two, &latex_cfg()).unwrap();
        assert!(
            two_out.contains("First line. Second line."),
            "alltt two-sentence body must stay one source line, got:\n{two_out}"
        );
        assert!(
            !two_out.contains("First line.\nSecond line."),
            "alltt two-sentence body must not split, got:\n{two_out}"
        );
        assert!(
            two_out.contains("After the block.\nNext."),
            "prose after alltt must still split, got:\n{two_out}"
        );
        assert_eq!(format_text(&two_out, &latex_cfg()).unwrap(), two_out);
    }

    /// Ticket fixture (GitHub #234): listings.sty `lstlisting*` is the
    /// starred twin of `lstlisting`. Body stays Code; following prose
    /// still splits.
    #[test]
    fn lstlisting_star_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{lstlisting*}\n",
            "First line. Second line.\n",
            "\\end{lstlisting*}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "lstlisting* body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "lstlisting* body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{lstlisting*}") && out.contains("\\end{lstlisting*}"),
            "lstlisting* begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "lstlisting* body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "lstlisting* must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after lstlisting* must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let lang = concat!(
            "\\begin{lstlisting*}[language=Python]\n",
            "print(1) # First. Second.\n",
            "\\end{lstlisting*}\n",
            "After the block. Next.\n",
        );
        let lang_regions = LatexParser::default().parse(lang);
        let lang_code = lang_regions.iter().find_map(|r| match r {
            Region::Code {
                lang, body, header, ..
            } => Some((lang.as_deref(), body.as_str(), header.as_str())),
            _ => None,
        });
        let Some((got_lang, body, header)) = lang_code else {
            panic!("lstlisting* with language= must be Code, got: {lang_regions:?}");
        };
        assert_eq!(
            got_lang,
            Some("Python"),
            "lstlisting* language= must parse like lstlisting, got lang={got_lang:?}"
        );
        assert!(
            header.contains(r"\begin{lstlisting*}[language=Python]"),
            "optional language= must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("print(1) # First. Second."),
            "lstlisting* language= body must stay Code, got body={body:?}"
        );
        let lang_out = format_text(lang, &latex_cfg()).unwrap();
        assert!(
            lang_out.contains("print(1) # First. Second."),
            "lstlisting* language= body must not reflow, got:\n{lang_out}"
        );
        assert!(
            lang_out.contains("After the block.\nNext."),
            "prose after lstlisting* language= must still split, got:\n{lang_out}"
        );
        assert_eq!(format_text(&lang_out, &latex_cfg()).unwrap(), lang_out);

        let raw = concat!(
            "\\begin{lstlisting*}\n",
            "print(1) % \\end{lstlisting*}\n",
            "After the block. Next.\n",
        );
        let raw_regions = LatexParser::default().parse(raw);
        let raw_code = raw_regions.iter().find_map(|r| match r {
            Region::Code { body, footer, .. } => Some((body.as_str(), footer.as_str())),
            _ => None,
        });
        let (raw_body, raw_footer) = raw_code.expect(&format!(
            "lstlisting* must stay Code on %, got: {raw_regions:?}"
        ));
        assert!(
            raw_body.contains("print(1)"),
            "lstlisting* raw scan must keep source before %, got body={raw_body:?}"
        );
        assert!(
            !raw_body.contains("After the block"),
            "% must not hide \\end{{lstlisting*}}; after-text is not listing body, got body={raw_body:?}"
        );
        assert!(
            raw_footer.contains(r"\end{lstlisting*}"),
            "lstlisting* footer must close on raw \\end, got footer={raw_footer:?}"
        );
        assert!(
            raw_regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("After the block"))),
            "prose after lstlisting* % closer must resume, got: {raw_regions:?}"
        );
    }

    /// Ticket fixture (GitHub #307): pythonhighlight.sty `python`
    /// (`lstnewenvironment{python}`) is the same listings raw scan as
    /// `lstlisting`. Body stays Code and one source line; following
    /// prose still splits. `lstlisting` / `pycode` unchanged.
    #[test]
    fn pythonhighlight_python_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{python}\n",
            "First line. Second line.\n",
            "\\end{python}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "python body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "python body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{python}") && out.contains("\\end{python}"),
            "python begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "python body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "python must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after python must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let lstlisting = concat!(
            "\\begin{lstlisting}\n",
            "First line. Second line.\n",
            "\\end{lstlisting}\n",
            "After the block. Next.\n",
        );
        let lstlisting_out = format_text(lstlisting, &latex_cfg()).unwrap();
        assert!(
            lstlisting_out
                .contains("\\begin{lstlisting}\nFirst line. Second line.\n\\end{lstlisting}"),
            "lstlisting must stay a code env, got:\n{lstlisting_out}"
        );
        assert!(
            lstlisting_out.contains("After the block.\nNext."),
            "prose after lstlisting must still split, got:\n{lstlisting_out}"
        );
        assert_eq!(
            format_text(&lstlisting_out, &latex_cfg()).unwrap(),
            lstlisting_out
        );

        let pycode = concat!(
            "\\begin{pycode}\n",
            "First line. Second line.\n",
            "\\end{pycode}\n",
            "After the block. Next.\n",
        );
        let pycode_out = format_text(pycode, &latex_cfg()).unwrap();
        assert!(
            pycode_out.contains("\\begin{pycode}\nFirst line. Second line.\n\\end{pycode}"),
            "pycode must stay a code env, got:\n{pycode_out}"
        );
        assert!(
            pycode_out.contains("After the block.\nNext."),
            "prose after pycode must still split, got:\n{pycode_out}"
        );
        assert_eq!(format_text(&pycode_out, &latex_cfg()).unwrap(), pycode_out);
    }

    /// Ticket fixture (GitHub #345): showexpl.sty `LTXexample`
    /// (`lstnewenvironment{LTXexample}`) is the same listings raw scan as
    /// `lstlisting`. Body stays Code and one source line; following
    /// prose still splits. `lstlisting` unchanged.
    #[test]
    fn showexpl_ltxexample_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{LTXexample}\n",
            "First line. Second line.\n",
            "\\end{LTXexample}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "LTXexample body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "LTXexample body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{LTXexample}") && out.contains("\\end{LTXexample}"),
            "LTXexample begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "LTXexample body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "LTXexample must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after LTXexample must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let lstlisting = concat!(
            "\\begin{lstlisting}\n",
            "First line. Second line.\n",
            "\\end{lstlisting}\n",
            "After the block. Next.\n",
        );
        let lstlisting_out = format_text(lstlisting, &latex_cfg()).unwrap();
        assert!(
            lstlisting_out
                .contains("\\begin{lstlisting}\nFirst line. Second line.\n\\end{lstlisting}"),
            "lstlisting must stay a code env, got:\n{lstlisting_out}"
        );
        assert!(
            lstlisting_out.contains("After the block.\nNext."),
            "prose after lstlisting must still split, got:\n{lstlisting_out}"
        );
        assert_eq!(
            format_text(&lstlisting_out, &latex_cfg()).unwrap(),
            lstlisting_out
        );
    }

    /// Ticket fixture (GitHub #346): pyluatex.sty `pythonq` / `pythonrepl`
    /// are verbatim python / REPL bodies. Body stays Code and one source
    /// line; following prose still splits. Landed `python` stays Code.
    #[test]
    fn pyluatex_pythonq_and_pythonrepl_are_code_not_prose() {
        use crate::format_text;

        for name in ["pythonq", "pythonrepl"] {
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        let python = concat!(
            "\\begin{python}\n",
            "First line. Second line.\n",
            "\\end{python}\n",
            "After the block. Next.\n",
        );
        let python_out = format_text(python, &latex_cfg()).unwrap();
        assert!(
            python_out.contains("\\begin{python}\nFirst line. Second line.\n\\end{python}"),
            "landed python must stay a code env, got:\n{python_out}"
        );
        assert!(
            python_out.contains("After the block.\nNext."),
            "prose after python must still split, got:\n{python_out}"
        );
        assert_eq!(format_text(&python_out, &latex_cfg()).unwrap(), python_out);
    }

    /// Ticket fixture (GitHub #348): luamplib.dtx `mplibcode` is the
    /// same raw grab class as `luacode`. Body stays Code and one source
    /// line; following prose still splits. `luacode` / `luacode*`
    /// unchanged.
    #[test]
    fn luamplib_mplibcode_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{mplibcode}\n",
            "First line. Second line.\n",
            "\\end{mplibcode}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "mplibcode body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "mplibcode body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{mplibcode}") && out.contains("\\end{mplibcode}"),
            "mplibcode begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "mplibcode body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "mplibcode must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after mplibcode must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        for name in ["luacode", "luacode*"] {
            let landed = format!(
                "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
            );
            let landed_out = format_text(&landed, &latex_cfg()).unwrap();
            assert!(
                landed_out.contains(&format!(
                    "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}"
                )),
                "{name} must stay a code env, got:\n{landed_out}"
            );
            assert!(
                landed_out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{landed_out}"
            );
            assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
        }
    }

    /// Ticket fixture (GitHub #350): codehigh.sty `codehigh` / `demohigh`
    /// and starred twins (`NewCodeHighEnv`) are leftover listing envs.
    /// Body stays Code and one source line; following prose still
    /// splits. `lstlisting` unchanged.
    #[test]
    fn codehigh_leftover_envs_are_code_not_prose() {
        use crate::format_text;

        for name in ["codehigh", "demohigh", "codehigh*", "demohigh*"] {
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
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        let lstlisting = concat!(
            "\\begin{lstlisting}\n",
            "First line. Second line.\n",
            "\\end{lstlisting}\n",
            "After the block. Next.\n",
        );
        let lstlisting_out = format_text(lstlisting, &latex_cfg()).unwrap();
        assert!(
            lstlisting_out
                .contains("\\begin{lstlisting}\nFirst line. Second line.\n\\end{lstlisting}"),
            "lstlisting must stay a code env, got:\n{lstlisting_out}"
        );
        assert!(
            lstlisting_out.contains("After the block.\nNext."),
            "prose after lstlisting must still split, got:\n{lstlisting_out}"
        );
        assert_eq!(
            format_text(&lstlisting_out, &latex_cfg()).unwrap(),
            lstlisting_out
        );
    }

    /// Ticket fixture (GitHub #273): minted.sty `minted*` is the starred
    /// twin of `minted`. Body stays Code; following prose still splits.
    /// Unstarred `minted` is unchanged.
    #[test]
    fn minted_star_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{minted*}\n",
            "First line. Second line.\n",
            "\\end{minted*}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "minted* body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "minted* body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{minted*}") && out.contains("\\end{minted*}"),
            "minted* begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "minted* body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "minted* must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after minted* must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let lang = concat!(
            "\\begin{minted*}{python}\n",
            "print(1) # First. Second.\n",
            "\\end{minted*}\n",
            "After the block. Next.\n",
        );
        let lang_regions = LatexParser::default().parse(lang);
        let lang_code = lang_regions.iter().find_map(|r| match r {
            Region::Code {
                lang, body, header, ..
            } => Some((lang.as_deref(), body.as_str(), header.as_str())),
            _ => None,
        });
        let Some((got_lang, body, header)) = lang_code else {
            panic!("minted* with {{lang}} must be Code, got: {lang_regions:?}");
        };
        assert_eq!(
            got_lang,
            Some("python"),
            "minted* language arg must parse like minted, got lang={got_lang:?}"
        );
        assert!(
            header.contains(r"\begin{minted*}{python}"),
            "required language arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("print(1) # First. Second."),
            "minted* language body must stay Code, got body={body:?}"
        );
        let lang_out = format_text(lang, &latex_cfg()).unwrap();
        assert!(
            lang_out.contains("print(1) # First. Second."),
            "minted* language body must not reflow, got:\n{lang_out}"
        );
        assert!(
            lang_out.contains("After the block.\nNext."),
            "prose after minted* language must still split, got:\n{lang_out}"
        );
        assert_eq!(format_text(&lang_out, &latex_cfg()).unwrap(), lang_out);

        let unstarred = concat!(
            "\\begin{minted}{python}\n",
            "print(1)\n",
            "print(2)\n",
            "\\end{minted}\n",
            "After the block. Next.\n",
        );
        let unstarred_regions = LatexParser::default().parse(unstarred);
        let unstarred_code = unstarred_regions.iter().find_map(|r| match r {
            Region::Code { lang, body, .. } => Some((lang.as_deref(), body.as_str())),
            _ => None,
        });
        let Some((got_lang, body)) = unstarred_code else {
            panic!("unstarred minted must stay Code, got: {unstarred_regions:?}");
        };
        assert_eq!(got_lang, Some("python"));
        assert!(
            body.contains("print(1)") && body.contains("print(2)"),
            "unstarred minted body must stay Code, got body={body:?}"
        );
        let unstarred_out = format_text(unstarred, &latex_cfg()).unwrap();
        assert!(
            unstarred_out.contains("\\begin{minted}{python}\nprint(1)\nprint(2)\n\\end{minted}"),
            "unstarred minted must stay a code env, got:\n{unstarred_out}"
        );
        assert!(
            unstarred_out.contains("After the block.\nNext."),
            "prose after unstarred minted must still split, got:\n{unstarred_out}"
        );
        assert_eq!(
            format_text(&unstarred_out, &latex_cfg()).unwrap(),
            unstarred_out
        );
    }

    /// Ticket fixture (GitHub #305): piton.sty `{Piton}` body stays
    /// Code; `\piton|done. Next|` is one token like `\verb`;
    /// `\piton{body}` stays one token via generic cmd-arg; following
    /// prose still splits. minted / lstlisting / `\verb` unchanged.
    #[test]
    fn piton_env_and_pipe_cmd_are_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{Piton}\n",
            "First line. Second line.\n",
            "\\end{Piton}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "Piton body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "Piton body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{Piton}") && out.contains("\\end{Piton}"),
            "Piton begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "Piton body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "Piton must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after Piton must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let cmd = "See \\piton|done. Next| here. After.\n";
        let cmd_out = format_text(cmd, &latex_cfg()).unwrap();
        assert!(
            cmd_out.contains(r"\piton|done. Next|"),
            "\\piton|done. Next| must stay one token, got:\n{cmd_out}"
        );
        assert!(
            !cmd_out.contains("\\piton|done.\n") && !cmd_out.contains("\\piton|done. Next|\n"),
            "inner .!? must not split \\piton|...|, got:\n{cmd_out}"
        );
        assert!(
            cmd_out.contains("See \\piton|done. Next| here.\nAfter."),
            "prose after \\piton|...| must still split, got:\n{cmd_out}"
        );
        assert_eq!(format_text(&cmd_out, &latex_cfg()).unwrap(), cmd_out);

        let brace = "See \\piton{done. Next} here. After.\n";
        let brace_out = format_text(brace, &latex_cfg()).unwrap();
        assert!(
            brace_out.contains(r"\piton{done. Next}"),
            "\\piton{{done. Next}} must stay one token via cmd-arg, got:\n{brace_out}"
        );
        assert!(
            !brace_out.contains("\\piton{done.\n"),
            "generic cmd-arg must keep \\piton{{...}} atomic, got:\n{brace_out}"
        );
        assert!(
            brace_out.contains("See \\piton{done. Next} here.\nAfter."),
            "prose after \\piton{{...}} must still split, got:\n{brace_out}"
        );
        assert_eq!(format_text(&brace_out, &latex_cfg()).unwrap(), brace_out);

        let minted = concat!(
            "\\begin{minted}{python}\n",
            "print(1)\n",
            "print(2)\n",
            "\\end{minted}\n",
            "After the block. Next.\n",
        );
        let minted_out = format_text(minted, &latex_cfg()).unwrap();
        assert!(
            minted_out.contains("\\begin{minted}{python}\nprint(1)\nprint(2)\n\\end{minted}"),
            "minted must stay a code env, got:\n{minted_out}"
        );
        assert!(
            minted_out.contains("After the block.\nNext."),
            "prose after minted must still split, got:\n{minted_out}"
        );

        let listing = concat!(
            "\\begin{lstlisting}\n",
            "First line. Second line.\n",
            "\\end{lstlisting}\n",
            "After the block. Next.\n",
        );
        let listing_out = format_text(listing, &latex_cfg()).unwrap();
        assert!(
            listing_out.contains("First line. Second line."),
            "lstlisting body must stay one source line, got:\n{listing_out}"
        );
        assert!(
            !listing_out.contains("First line.\nSecond line."),
            "lstlisting must not reflow as prose, got:\n{listing_out}"
        );
        assert!(
            listing_out.contains("After the block.\nNext."),
            "prose after lstlisting must still split, got:\n{listing_out}"
        );

        let verb = "Use \\verb|a.b! c| here. Next sentence.\n";
        let verb_out = format_text(verb, &latex_cfg()).unwrap();
        assert!(
            verb_out.contains(r"\verb|a.b! c|"),
            "\\verb must stay intact, got:\n{verb_out}"
        );
        assert!(
            verb_out.contains("Use \\verb|a.b! c| here.\nNext sentence."),
            "prose after \\verb must still split, got:\n{verb_out}"
        );
    }

    /// Ticket fixture (GitHub #235): spverbatim.sty env body stays Code;
    /// `\spverb|a.b%|` is one token; following `Next.` still splits.
    #[test]
    fn spverbatim_and_spverb_are_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{spverbatim}\n",
            "First line. Second line.\n",
            "\\end{spverbatim}\n",
            "See \\spverb|a.b%| please. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "spverbatim body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "spverbatim body must not leak into Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(r, Region::Structure(s) if s.contains("%|") || s.trim() == "%|\n")
            }),
            "inner % of \\spverb must not be a comment, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{spverbatim}") && out.contains("\\end{spverbatim}"),
            "spverbatim begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "spverbatim body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "spverbatim must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains(r"\spverb|a.b%|"),
            "\\spverb|a.b%| must stay one token, got:\n{out}"
        );
        assert!(
            !out.contains("\\spverb|a.\n") && !out.contains("\\spverb|a.b%\n"),
            "inner .!?% must not split \\spverb, got:\n{out}"
        );
        assert!(
            out.contains("See \\spverb|a.b%| please.\nNext."),
            "prose after \\spverb must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn boxedverbatim_tcblisting_codeexample_are_code_not_prose() {
        use crate::format_text;

        let names = ["boxedverbatim", "tcblisting", "codeexample"];
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

    /// Ticket fixture (GitHub #250): moreverb `verbatimtab` is the same
    /// raw class as `boxedverbatim`. Body stays Code; following prose
    /// still splits.
    #[test]
    fn moreverb_verbatimtab_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{verbatimtab}\n",
            "First line. Second line.\n",
            "\\end{verbatimtab}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "verbatimtab body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "verbatimtab body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\begin{verbatimtab}") && out.contains("\\end{verbatimtab}"),
            "verbatimtab begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "verbatimtab body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "verbatimtab must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after verbatimtab must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #279): moreverb `listing` is a
    /// `verbatim@start` raw body. Required `{1}` stays on the begin
    /// header. Body stays Code; following prose still splits.
    #[test]
    fn moreverb_listing_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{listing}{1}\n",
            "First line. Second line.\n",
            "\\end{listing}\n",
            "After the block. Next.\n",
        );
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
            panic!("listing must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{listing}{1}"),
            "required start-line arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "listing body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{listing}"),
            "listing footer must stay, got footer={footer:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("First line") || p.contains("{1}")
            )),
            "listing start-line/body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{listing}{1}") && out.contains(r"\end{listing}"),
            "listing begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "listing body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "listing must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after listing must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #306): moreverb `verbatimwrite` writes the
    /// env body raw via `verbatim@start` (same class as `VerbatimOut` /
    /// `tcbverbatimwrite`). Required `{out.tex}` stays on the begin
    /// header. Body stays Code; following prose still splits. Landed
    /// `VerbatimWrite` / `tcbverbatimwrite` / listing family stay Code.
    #[test]
    fn moreverb_verbatimwrite_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{verbatimwrite}{out.tex}\n",
            "First line. Second line.\n",
            "\\end{verbatimwrite}\n",
            "After the block. Next.\n",
        );
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
            panic!("verbatimwrite must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{verbatimwrite}{out.tex}"),
            "required file arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "verbatimwrite body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{verbatimwrite}"),
            "verbatimwrite footer must stay, got footer={footer:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "verbatimwrite body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{verbatimwrite}{out.tex}") && out.contains(r"\end{verbatimwrite}"),
            "verbatimwrite begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "verbatimwrite body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "verbatimwrite must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after verbatimwrite must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        // Landed neighbours stay Code (GitHub #247 / #280 / #279).
        let landed = [
            ("VerbatimWrite", "{out.tex}"),
            ("tcbverbatimwrite", "{out.tex}"),
            ("listing", "{1}"),
            ("listingcont", ""),
            ("listing*", "{1}"),
            ("listingcont*", ""),
        ];
        for (name, arg) in landed {
            let begin = format!("\\begin{{{name}}}{arg}");
            let input = format!(
                "{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
            );
            let regions = LatexParser::default().parse(&input);
            assert!(
                regions.iter().any(|r| matches!(
                    r,
                    Region::Code { body, .. } if body.contains("First line. Second line.")
                )),
                "landed {name} must stay Code, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains("First line. Second line.") && out.contains("After the block.\nNext."),
                "landed {name} must stay verbatim with following prose split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (GitHub #353): leftover sverb.sty `verbwrite` /
    /// `ignore` / `demo` / `demo*` (`sv@readenv` raw grab) stay Code.
    /// Required `{tmp.tex}` stays on the verbwrite begin header.
    /// Following prose still splits. Landed moreverb `verbatimwrite`
    /// stays Code.
    #[test]
    fn sverb_leftover_write_and_demo_envs_are_code_not_prose() {
        use crate::format_text;

        let cases = [
            ("verbwrite", "{tmp.tex}"),
            ("ignore", ""),
            ("demo", "{Title}"),
            ("demo*", "{Title}"),
        ];
        for (name, arg) in cases {
            let begin = format!("\\begin{{{name}}}{arg}");
            let input = format!(
                "{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
            );
            let regions = LatexParser::default().parse(&input);
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
                panic!("{name} must be Code, got: {regions:?}");
            };
            assert!(
                header.contains(&begin),
                "{name} begin must stay on the header, got header={header:?}"
            );
            assert!(
                body.contains("First line. Second line."),
                "{name} body must keep both sentences, got body={body:?}"
            );
            assert!(
                footer.contains(&format!("\\end{{{name}}}")),
                "{name} footer must stay, got footer={footer:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
                "{name} body must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains(&begin) && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        let landed = concat!(
            "\\begin{verbatimwrite}{out.tex}\n",
            "First line. Second line.\n",
            "\\end{verbatimwrite}\n",
            "After the block. Next.\n",
        );
        let landed_out = format_text(landed, &latex_cfg()).unwrap();
        assert!(
            landed_out.contains(
                "\\begin{verbatimwrite}{out.tex}\nFirst line. Second line.\n\\end{verbatimwrite}"
            ),
            "landed verbatimwrite must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("After the block.\nNext."),
            "prose after landed verbatimwrite must still split, got:\n{landed_out}"
        );
        assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
    }

    /// Ticket fixture (GitHub #279): moreverb `listingcont` / `listing*`
    /// / `listingcont*` are the same `verbatim@start` raw class as
    /// `listing`. `listing*` keeps the start-line arg on begin.
    #[test]
    fn moreverb_listing_twins_are_code_not_prose() {
        use crate::format_text;

        let cases = [
            ("listingcont", ""),
            ("listing*", "{1}"),
            ("listingcont*", ""),
        ];
        for (name, arg) in cases {
            let begin = format!("\\begin{{{name}}}{arg}");
            let input = format!(
                "{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
            );
            let regions = LatexParser::default().parse(&input);
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
                panic!("{name} must be Code, got: {regions:?}");
            };
            assert!(
                header.contains(&begin),
                "{name} begin must stay on the header, got header={header:?}"
            );
            assert!(
                body.contains("First line. Second line."),
                "{name} body must keep both sentences, got body={body:?}"
            );
            assert!(
                footer.contains(&format!("\\end{{{name}}}")),
                "{name} footer must stay, got footer={footer:?}"
            );
            assert!(
                !regions.iter().any(|r| matches!(
                    r,
                    Region::Prose(p) if p.contains("First line") || p.contains("{1}")
                )),
                "{name} body/arg must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains(&begin) && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (GitHub #246): tcolorbox listings `tcblisting*`
    /// is the starred twin of `tcblisting`. The required `{listing
    /// only}` arg stays on the begin header. Body stays Code;
    /// following prose still splits.
    #[test]
    fn tcolorbox_tcblisting_star_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{tcblisting*}{listing only}\n",
            "First line. Second line.\n",
            "\\end{tcblisting*}\n",
            "After the block. Next.\n",
        );
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
            panic!("tcblisting* must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{tcblisting*}{listing only}"),
            "required listing options must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "tcblisting* body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{tcblisting*}"),
            "tcblisting* footer must stay, got footer={footer:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "tcblisting* body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{tcblisting*}{listing only}")
                && out.contains(r"\end{tcblisting*}"),
            "tcblisting* begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "tcblisting* body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "tcblisting* must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after tcblisting* must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #280): tcolorbox `tcbverbatimwrite` writes
    /// the env body raw to a file (same verbatim grab as `VerbatimOut`).
    /// Required `{file}` stays on the begin header; body stays Code;
    /// following prose still splits.
    #[test]
    fn tcolorbox_tcbverbatimwrite_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{tcbverbatimwrite}{out.tex}\n",
            "First line. Second line.\n",
            "\\end{tcbverbatimwrite}\n",
            "After the block. Next.\n",
        );
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
            panic!("tcbverbatimwrite must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{tcbverbatimwrite}{out.tex}"),
            "required file arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "tcbverbatimwrite body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{tcbverbatimwrite}"),
            "tcbverbatimwrite footer must stay, got footer={footer:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "tcbverbatimwrite body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{tcbverbatimwrite}{out.tex}")
                && out.contains(r"\end{tcbverbatimwrite}"),
            "tcbverbatimwrite begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "tcbverbatimwrite body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "tcbverbatimwrite must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after tcbverbatimwrite must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #280): tcolorbox `tcbwritetemp` is the
    /// no-arg twin that writes to `\jobname.tcbtemp`. Body stays Code;
    /// following prose still splits.
    #[test]
    fn tcolorbox_tcbwritetemp_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{tcbwritetemp}\n",
            "First line. Second line.\n",
            "\\end{tcbwritetemp}\n",
            "After the block. Next.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "tcbwritetemp body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "tcbwritetemp body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{tcbwritetemp}") && out.contains(r"\end{tcbwritetemp}"),
            "tcbwritetemp begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "tcbwritetemp body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "tcbwritetemp must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after tcbwritetemp must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #334): leftover tcolorbox write/listing
    /// envs `tcboutputlisting` / `tcbexternal` / `dispExample` /
    /// `dispExample*` / `dispListing` / `dispListing*` stay Code.
    /// Following prose still splits. Landed `tcbverbatimwrite` /
    /// `tcbwritetemp` stay Code.
    #[test]
    fn tcolorbox_leftover_write_listing_envs_are_code_not_prose() {
        use crate::format_text;

        let cases = [
            ("tcboutputlisting", ""),
            ("tcbexternal", "{name}"),
            ("dispExample", ""),
            ("dispExample*", "{sbs}"),
            ("dispListing", ""),
            ("dispListing*", "{listing only}"),
        ];
        for (name, arg) in cases {
            let begin = format!("\\begin{{{name}}}{arg}");
            let input = format!(
                "{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
            );
            let regions = LatexParser::default().parse(&input);
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
                panic!("{name} must be Code, got: {regions:?}");
            };
            assert!(
                header.contains(&begin),
                "{name} begin must stay on the header, got header={header:?}"
            );
            assert!(
                body.contains("First line. Second line."),
                "{name} body must keep both sentences, got body={body:?}"
            );
            assert!(
                footer.contains(&format!("\\end{{{name}}}")),
                "{name} footer must stay, got footer={footer:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
                "{name} body must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains(&begin) && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        // Landed neighbours stay Code (GitHub #280).
        let landed = concat!(
            "\\begin{tcbverbatimwrite}{out.tex}\n",
            "First line. Second line.\n",
            "\\end{tcbverbatimwrite}\n",
            "\\begin{tcbwritetemp}\n",
            "First line. Second line.\n",
            "\\end{tcbwritetemp}\n",
            "After the block. Next.\n",
        );
        let landed_out = format_text(landed, &latex_cfg()).unwrap();
        assert!(
            landed_out.contains(
                "\\begin{tcbverbatimwrite}{out.tex}\nFirst line. Second line.\n\\end{tcbverbatimwrite}"
            ),
            "landed tcbverbatimwrite must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out
                .contains("\\begin{tcbwritetemp}\nFirst line. Second line.\n\\end{tcbwritetemp}"),
            "landed tcbwritetemp must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("After the block.\nNext."),
            "prose after landed tcolorbox write envs must still split, got:\n{landed_out}"
        );
        assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
    }

    #[test]
    fn configured_verbatim_envs_still_add_unlisted_names() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore.\n\\begin{MyListings}\nFirst line. Second line.\n\\end{MyListings}\nAfter the listing. Next.\n\\end{document}\n";
        let default_out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            default_out.contains("First line.\nSecond line."),
            "unlisted MyListings body is prose and reflows, got:\n{default_out}"
        );

        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_verbatim_envs: vec!["MyListings".into()],
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("First line. Second line."),
            "configured MyListings body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "MyListings must stay verbatim, got:\n{out}"
        );
        let regions = LatexParser::from_config(Some(&cfg)).parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "configured MyListings must be Code, got: {regions:?}"
        );
        assert!(
            out.contains("After the listing.\nNext."),
            "prose after MyListings must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn configured_verbatim_command_is_tokenized_like_verb() {
        use crate::format_text;

        let input =
            "\\begin{document}\nUse \\MyVerb|a.b! c| here. Next sentence.\n\\end{document}\n";
        let default_out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            !default_out.contains("Use \\MyVerb|a.b! c| here.\nNext sentence."),
            "unlisted MyVerb must not stay atomic like verb, got:\n{default_out}"
        );

        let cfg = crate::FormatConfig {
            format: crate::format::Format::Latex,
            latex_verbatim_commands: vec!["MyVerb".into()],
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains(r"\MyVerb|a.b! c|"),
            "configured MyVerb must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("\\MyVerb|a.\n") && !out.contains("\\MyVerb|a.b!\n"),
            "inner .!? must not split configured MyVerb, got:\n{out}"
        );
        assert!(
            out.contains("Use \\MyVerb|a.b! c| here.\nNext sentence."),
            "configured MyVerb must tokenize like verb before split, got:\n{out}"
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
    fn verbatim_fixture_file_is_code_not_prose() {
        let input = include_str!("../../tests/fixtures/verbatim.tex");
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "tests/fixtures/verbatim.tex body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "verbatim fixture must not be Prose, got: {regions:?}"
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
            "mplibcode",
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

    /// Ticket fixture (GitHub #298): sagetex.sty `sageverbatim` /
    /// `sageexample` / `sagecommandline` are the same `verbatim@start`
    /// class as tree-sitter `sagesilent` / `sageblock`. Body stays
    /// Code; following prose still splits. Landed trivia sage envs
    /// stay Code.
    #[test]
    fn sagetex_sageverbatim_family_is_code_not_prose() {
        use crate::format_text;

        for name in ["sageverbatim", "sageexample", "sagecommandline"] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
            );
            let regions = LatexParser::default().parse(&input);
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
                panic!("{name} must be Code, got: {regions:?}");
            };
            assert!(
                header.contains(&format!("\\begin{{{name}}}")),
                "{name} begin must stay on the header, got header={header:?}"
            );
            assert!(
                body.contains("First line. Second line."),
                "{name} body must keep both sentences, got body={body:?}"
            );
            assert!(
                footer.contains(&format!("\\end{{{name}}}")),
                "{name} footer must stay, got footer={footer:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
                "{name} body must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        for name in ["sagesilent", "sageblock"] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
            );
            let regions = LatexParser::default().parse(&input);
            assert!(
                regions.iter().any(|r| matches!(
                    r,
                    Region::Code { body, .. } if body.contains("First line. Second line.")
                )),
                "landed {name} must stay Code, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains("First line. Second line.") && out.contains("After the block.\nNext."),
                "landed {name} must stay verbatim with following prose split, got:\n{out}"
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

    /// Ticket fixture (GitHub #294): filecontentsdef.sty `filecontentsdef`
    /// writes the env body verbatim into a macro (same raw grab as
    /// `filecontents`). Required `{\body}` stays on the begin header;
    /// body stays Code; following prose still splits.
    #[test]
    fn filecontentsdef_fixture_is_code_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{filecontentsdef}{\\body}\n",
            "First line. Second line.\n",
            "\\end{filecontentsdef}\n",
            "After the block. Next.\n",
        );
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
            panic!("filecontentsdef must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(r"\begin{filecontentsdef}{\body}"),
            "required macro arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "filecontentsdef body must keep both sentences, got body={body:?}"
        );
        assert!(
            footer.contains(r"\end{filecontentsdef}"),
            "filecontentsdef footer must stay, got footer={footer:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "filecontentsdef body must not leak into Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(r"\begin{filecontentsdef}{\body}")
                && out.contains(r"\end{filecontentsdef}"),
            "filecontentsdef begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "filecontentsdef body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "filecontentsdef must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after filecontentsdef must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #299): leftover filecontentsdef.dtx
    /// siblings stay Code; required `{\body}` stays on begin; following
    /// prose still splits. Landed filecontents / filecontents* /
    /// filecontentsdef stay Code.
    #[test]
    fn filecontentsdef_sibling_envs_are_code_not_prose() {
        use crate::format_text;

        let names = [
            "filecontentsgdef",
            "filecontentsdefmacro",
            "filecontentsgdefmacro",
            "filecontentshere",
            "filecontentsdef*",
            "filecontentsgdef*",
            "filecontentshere*",
        ];
        for name in names {
            let begin = format!(r"\begin{{{name}}}{{\body}}");
            let input = format!(
                "{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
            );
            let regions = LatexParser::default().parse(&input);
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
                panic!("{name} must be Code, got: {regions:?}");
            };
            assert!(
                header.contains(&begin),
                "{name} required arg must stay on the begin header, got header={header:?}"
            );
            assert!(
                body.contains("First line. Second line."),
                "{name} body must keep both sentences, got body={body:?}"
            );
            assert!(
                footer.contains(&format!("\\end{{{name}}}")),
                "{name} footer must stay, got footer={footer:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
                "{name} body must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains(&begin) && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        let landed = concat!(
            "\\begin{filecontents}{x.tex}\n",
            "First line. Second line.\n",
            "\\end{filecontents}\n",
            "\\begin{filecontents*}\n",
            "First line. Second line.\n",
            "\\end{filecontents*}\n",
            "\\begin{filecontentsdef}{\\body}\n",
            "First line. Second line.\n",
            "\\end{filecontentsdef}\n",
            "After the block. Next.\n",
        );
        let landed_out = format_text(landed, &latex_cfg()).unwrap();
        assert!(
            landed_out.contains(
                "\\begin{filecontents}{x.tex}\nFirst line. Second line.\n\\end{filecontents}"
            ),
            "landed filecontents must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out
                .contains("\\begin{filecontents*}\nFirst line. Second line.\n\\end{filecontents*}"),
            "landed filecontents* must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains(
                "\\begin{filecontentsdef}{\\body}\nFirst line. Second line.\n\\end{filecontentsdef}"
            ),
            "landed filecontentsdef must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("After the block.\nNext."),
            "prose after landed filecontentsdef must still split, got:\n{landed_out}"
        );
        assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
    }

    /// Ticket fixture (GitHub #304): scontents.sty `scontents` stores
    /// the env body verbatim into a sequence; `verbatimsc` is the
    /// package verbatim display env. Body stays Code; following prose
    /// still splits. Landed filecontentsdef family stays Code.
    #[test]
    fn scontents_verbatimsc_are_code_not_prose() {
        use crate::format_text;

        for name in ["scontents", "verbatimsc"] {
            let input = format!(
                concat!(
                    "\\begin{{{name}}}\n",
                    "First line. Second line.\n",
                    "\\end{{{name}}}\n",
                    "After the block. Next.\n",
                ),
                name = name
            );
            let regions = LatexParser::default().parse(&input);
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
                panic!("{name} must be Code, got: {regions:?}");
            };
            assert!(
                header.contains(&format!("\\begin{{{name}}}")),
                "{name} begin must stay on the header, got header={header:?}"
            );
            assert!(
                body.contains("First line. Second line."),
                "{name} body must keep both sentences, got body={body:?}"
            );
            assert!(
                footer.contains(&format!("\\end{{{name}}}")),
                "{name} footer must stay, got footer={footer:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
                "{name} body must not leak into Prose, got: {regions:?}"
            );
            let out = format_text(&input, &latex_cfg()).unwrap();
            assert!(
                out.contains(&format!("\\begin{{{name}}}"))
                    && out.contains(&format!("\\end{{{name}}}")),
                "{name} begin/end must stay, got:\n{out}"
            );
            assert!(
                out.contains("First line. Second line."),
                "{name} body must stay one source line, got:\n{out}"
            );
            assert!(
                !out.contains("First line.\nSecond line."),
                "{name} must not reflow as prose, got:\n{out}"
            );
            assert!(
                out.contains("After the block.\nNext."),
                "prose after {name} must still split, got:\n{out}"
            );
            assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
        }

        let landed = concat!(
            "\\begin{filecontents}{x.tex}\n",
            "First line. Second line.\n",
            "\\end{filecontents}\n",
            "\\begin{filecontents*}\n",
            "First line. Second line.\n",
            "\\end{filecontents*}\n",
            "\\begin{filecontentsdef}{\\body}\n",
            "First line. Second line.\n",
            "\\end{filecontentsdef}\n",
            "After the block. Next.\n",
        );
        let landed_out = format_text(landed, &latex_cfg()).unwrap();
        assert!(
            landed_out.contains(
                "\\begin{filecontents}{x.tex}\nFirst line. Second line.\n\\end{filecontents}"
            ),
            "landed filecontents must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out
                .contains("\\begin{filecontents*}\nFirst line. Second line.\n\\end{filecontents*}"),
            "landed filecontents* must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains(
                "\\begin{filecontentsdef}{\\body}\nFirst line. Second line.\n\\end{filecontentsdef}"
            ),
            "landed filecontentsdef must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("After the block.\nNext."),
            "prose after landed filecontentsdef must still split, got:\n{landed_out}"
        );
        assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
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

    /// Ticket fixture (GitHub #97 / snapper-e916): tikzcd is Structure.
    #[test]
    fn tikzcd_fixture_is_structure_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{tikzcd}\n",
            "This is a long sentence that must not reflow as prose inside tikzcd.\n",
            "\\end{tikzcd}\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("must not reflow as prose inside tikzcd")
            )),
            "tikzcd body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("must not reflow as prose inside tikzcd")
            )),
            "tikzcd body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("This is a long sentence that must not reflow as prose inside tikzcd."),
            "tikzcd body must stay one source line, got:\n{out}"
        );
        assert_eq!(out, input, "tikzcd env must be identity, got:\n{out}");
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn tikzcd_axis_pgfpicture_and_stars_are_structure() {
        let names = [
            "tikzcd",
            "tikzcd*",
            "axis",
            "axis*",
            "pgfpicture",
            "pgfpicture*",
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
    fn tikzcd_axis_pgfpicture_two_sentence_bodies_do_not_reflow() {
        use crate::format_text;

        let input = "\\begin{document}\nBefore the figures. More before.\n\\begin{tikzcd}\nFirst sentence inside tikzcd. Second sentence stays put.\n\\end{tikzcd}\n\\begin{axis}\nFirst sentence inside axis. Second sentence stays put.\n\\end{axis}\n\\begin{pgfpicture}\nFirst sentence inside pgfpicture. Second sentence stays put.\n\\end{pgfpicture}\nAfter the figures. Next.\n\\end{document}\n";
        let out = format_text(input, &latex_cfg()).unwrap();
        for fused in [
            "First sentence inside tikzcd. Second sentence stays put.",
            "First sentence inside axis. Second sentence stays put.",
            "First sentence inside pgfpicture. Second sentence stays put.",
        ] {
            assert!(
                out.contains(fused),
                "tikz/pgf body must not reflow, missing {fused:?}, got:\n{out}"
            );
        }
        assert!(
            !out.contains("inside tikzcd.\nSecond")
                && !out.contains("inside axis.\nSecond")
                && !out.contains("inside pgfpicture.\nSecond"),
            "tikz/pgf bodies must stay one source line, got:\n{out}"
        );
        assert!(
            out.contains("Before the figures.\nMore before.")
                && out.contains("After the figures.\nNext."),
            "prose around tikz/pgf envs must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    /// Ticket fixture (GitHub #182 / snapper-x4di): NiceTabular is Structure.
    #[test]
    fn nicetabular_fixture_is_structure_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{NiceTabular}{cc}\n",
            "This is a long sentence that must stay inside NiceTabular and must not reflow as prose.\n",
            "\\end{NiceTabular}\n",
            "After the table. Second sentence.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("must stay inside NiceTabular and must not reflow as prose")
            )),
            "NiceTabular body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("must stay inside NiceTabular and must not reflow as prose")
            )),
            "NiceTabular body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(
                "This is a long sentence that must stay inside NiceTabular and must not reflow as prose."
            ),
            "NiceTabular body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("must stay inside NiceTabular.\n"),
            "must not split the NiceTabular body into prose sentences, got:\n{out}"
        );
        assert!(
            out.contains("After the table.\nSecond sentence."),
            "prose after NiceTabular must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn ieeeeqnarray_fixture_is_structure_not_prose() {
        use crate::format_text;

        let input = concat!(
            "\\begin{IEEEeqnarray}{rCl}\n",
            "This is a long sentence that must stay inside IEEEeqnarray and must not reflow as prose.\n",
            "\\end{IEEEeqnarray}\n",
            "After the array. Second sentence.\n",
        );
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("must stay inside IEEEeqnarray and must not reflow as prose")
            )),
            "IEEEeqnarray body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("must stay inside IEEEeqnarray and must not reflow as prose")
            )),
            "IEEEeqnarray body must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(
                "This is a long sentence that must stay inside IEEEeqnarray and must not reflow as prose."
            ),
            "IEEEeqnarray body must stay one source line, got:\n{out}"
        );
        assert!(
            out.contains("After the array.\nSecond sentence."),
            "prose after IEEEeqnarray must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
    #[test]
    fn leftover_overleaf_ieee_xltabular_mathstar_are_structure() {
        let names = [
            "IEEEeqnarray",
            "IEEEeqnarray*",
            "subeqnarray",
            "subeqnarray*",
            "xltabular",
            "math*",
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
    fn leftover_latexindent_nicematrix_listabla_spreadtab_are_structure() {
        let names = [
            "NiceTabular",
            "NiceMatrix",
            "pNiceMatrix",
            "bNiceMatrix",
            "BNiceMatrix",
            "vNiceMatrix",
            "VNiceMatrix",
            "NiceArray",
            "pNiceArrayC",
            "bNiceArrayC",
            "BNiceArrayC",
            "vNiceArrayC",
            "VNiceArrayC",
            "NiceArrayCwithDelims",
            "pNiceArrayRC",
            "bNiceArrayRC",
            "BNiceArrayRC",
            "vNiceArrayRC",
            "VNiceArrayRC",
            "NiceArrayRCwithDelims",
            "listabla",
            "spreadtab",
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
    fn leftover_latexindent_nicematrix_two_sentence_bodies_do_not_reflow() {
        use crate::format_text;

        let input = concat!(
            "\\begin{document}\n",
            "\\begin{NiceTabular}{cc}\n",
            "First sentence inside NiceTabular. Second sentence stays put.\n",
            "\\end{NiceTabular}\n",
            "\\begin{NiceMatrix}\n",
            "First sentence inside NiceMatrix. Second sentence stays put.\n",
            "\\end{NiceMatrix}\n",
            "\\begin{NiceArray}\n",
            "First sentence inside NiceArray. Second sentence stays put.\n",
            "\\end{NiceArray}\n",
            "\\begin{listabla}\n",
            "First sentence inside listabla. Second sentence stays put.\n",
            "\\end{listabla}\n",
            "\\begin{spreadtab}\n",
            "First sentence inside spreadtab. Second sentence stays put.\n",
            "\\end{spreadtab}\n",
            "After the table. Next.\n",
            "\\end{document}\n",
        );
        let out = format_text(input, &latex_cfg()).unwrap();
        for fused in [
            "First sentence inside NiceTabular. Second sentence stays put.",
            "First sentence inside NiceMatrix. Second sentence stays put.",
            "First sentence inside NiceArray. Second sentence stays put.",
            "First sentence inside listabla. Second sentence stays put.",
            "First sentence inside spreadtab. Second sentence stays put.",
        ] {
            assert!(
                out.contains(fused),
                "nicematrix/listabla/spreadtab body must not reflow, missing {fused:?}, got:\n{out}"
            );
        }
        assert!(
            !out.contains("inside NiceTabular.\nSecond")
                && !out.contains("inside NiceMatrix.\nSecond")
                && !out.contains("inside NiceArray.\nSecond")
                && !out.contains("inside listabla.\nSecond")
                && !out.contains("inside spreadtab.\nSecond"),
            "nicematrix/listabla/spreadtab bodies must stay one source line, got:\n{out}"
        );
        assert!(
            out.contains("After the table.\nNext."),
            "prose after the envs must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }

    #[test]
    fn tikzcd_fixture_file_is_structure_not_prose() {
        let input = include_str!("../../tests/fixtures/tikzcd.tex");
        let regions = LatexParser::default().parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("must not reflow as prose inside tikzcd")
            )),
            "tests/fixtures/tikzcd.tex body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("inside tikzcd")
            )),
            "tikzcd fixture must not be Prose, got: {regions:?}"
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
    #[test]
    fn mid_line_bracket_display_math_is_structure_not_prose() {
        use crate::format_text;

        // snapper-ep2t / GitHub #85: latexindent `(?<!\\)\\\[` — mid-line `\[`
        // opens display math; `\\[2ex]` is a linebreak skip.
        let input = "inducing \\[\nE = m c^2.\n\\] more words. Next.\nSee also \\[ a = 1. \\] done. Next.\nfoo \\\\[2ex]\nstill prose. Next.\n";
        let regions = LatexParser::default().parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("inducing"))),
            "leading words before mid-line \\[ must be Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("E = m c^2.")
            )),
            "inducing \\[ body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("E = m c^2."))),
            "inducing \\[ body must not be Prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("more words"))),
            "words after \\] must resume Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("a = 1.")
            )),
            "same-line \\[ a = 1. \\] must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("a = 1."))),
            "same-line \\[ body must not be Prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("done"))),
            "words after same-line \\] must be Prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("\\\\[2ex]"))),
            "\\\\[2ex] linebreak skip must stay Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| {
                matches!(r, Region::Structure(s) if s.contains("\\\\[2ex]") || s.contains("[2ex]"))
            }),
            "\\\\[2ex] must not open display math, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("still prose"))),
            "line after \\\\[2ex] must stay Prose, got: {regions:?}"
        );

        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains("inducing \\[\nE = m c^2.\n\\]"),
            "mid-line \\[ must not join into the math body, got:\n{out}"
        );
        assert!(
            !out.contains("inducing \\[ E = m c^2."),
            "must not join inducing \\[ onto the next line, got:\n{out}"
        );
        assert!(
            out.contains("\\] more words.\nNext."),
            "prose after \\] must still reflow, got:\n{out}"
        );
        assert!(
            out.contains("See also \\[ a = 1. \\] done.\nNext."),
            "same-line \\[ a = 1. \\] must stay intact and trailing prose reflow, got:\n{out}"
        );
        assert!(
            !out.contains("\\[ a = 1.\n"),
            "must not split inside same-line display math, got:\n{out}"
        );
        assert!(
            out.contains("\\\\[2ex]"),
            "\\\\[2ex] must remain in the output, got:\n{out}"
        );
        assert!(
            out.contains("still prose.\nNext."),
            "\\\\[2ex] must not swallow following prose as math, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}
