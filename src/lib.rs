//! # snapper
//!
//! Semantic line break formatter for prose documents. Reformats text so each
//! sentence occupies its own line, producing minimal git diffs when
//! collaborating on papers and documentation.
//!
//! The crate is published as `snapper-fmt` on crates.io. Installers ship two
//! CLI names for the same program: `snapper` and `snapper-fmt` (the latter
//! avoids colliding with openSUSE's Btrfs snapshot tool of the same name).
//!
//! ## Supported formats
//!
//! - **Org-mode**: drawers, tables, keywords preserved; `#+BEGIN_SRC` is
//!   `Region::Code` (comment reflow via `[code.<lang>]`, optional formatters)
//! - **LaTeX**: preamble and math preserved; `minted` / `minted*` / `lstlisting` / `lstlisting*` / `Piton` are code regions
//! - **Markdown**: front matter and headings preserved; fenced blocks are code regions
//! - **RST**: directives and literals preserved; admonition/figure/topic/sidebar/container/parsed-literal/epigraph/highlights/pull-quote/compound bodies reflow; `.. code-block::` is a code region
//! - **Plaintext**: everything treated as prose
//! - **Typst**: headings, code calls, raw blocks, and math preserved; markup paragraphs and list bodies reflow
//! - **AsciiDoc**: headings, attributes, listings, and macros preserved; paragraphs, list bodies, and example interiors reflow
//!
//! ## Library usage
//!
//! ```rust
//! use snapper_fmt::{format_text, FormatConfig};
//! use snapper_fmt::format::Format;
//!
//! let input = "Hello world. This is a test. Another sentence.";
//! let config = FormatConfig {
//!     format: Format::Plaintext,
//!     ..Default::default()
//! };
//! let output = format_text(input, &config).unwrap();
//! assert_eq!(output, "Hello world.\nThis is a test.\nAnother sentence.");
//! ```

pub mod abbreviations;
#[cfg(not(target_arch = "wasm32"))]
pub mod changed;
pub mod check;
#[cfg(feature = "cli")]
pub mod cli;
pub mod code_block;
pub mod config;
mod detect;
pub mod diff;
#[cfg(not(target_arch = "wasm32"))]
pub mod files;
pub mod format;
#[cfg(not(target_arch = "wasm32"))]
pub mod git_diff;
#[cfg(feature = "cli")]
pub mod init;
#[cfg(feature = "lsp")]
pub mod lsp;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod oracle;
pub mod output;
pub mod parser;
pub mod reflow;
#[cfg(not(target_arch = "wasm32"))]
pub mod sdiff;
pub mod sentence;
#[cfg(feature = "treesitter")]
mod ts_comments;
#[cfg(feature = "wasm")]
pub mod wasm;
#[cfg(feature = "watch")]
pub mod watch;

use std::collections::HashMap;

use anyhow::Result;

use crate::config::CodeLang;
use crate::format::Format;
use crate::reflow::ReflowConfig;
use crate::sentence::SentenceSplitter;
use crate::sentence::unicode::UnicodeSentenceSplitter;

/// Configuration for the formatting pipeline.
pub struct FormatConfig {
    pub format: Format,
    pub max_width: usize,
    pub use_neural: bool,
    pub neural_lang: String,
    pub neural_model_path: Option<std::path::PathBuf>,
    pub extra_abbreviations: Vec<String>,
    pub use_pandoc: bool,
    /// Pandoc input format string (for pandoc backend).
    pub pandoc_format: Option<String>,
    /// How to obtain the pandoc AST when `use_pandoc` is set.
    /// `Ffi` uses in-process Haskell/C bindings for parse; `Cli` uses a
    /// subprocess. Write is in-process only when the library exports a
    /// writer; otherwise it still needs `pandoc` on PATH.
    #[cfg(feature = "pandoc")]
    pub pandoc_backend: parser::pandoc::PandocBackend,
    /// Per-language code-block configuration loaded from `[code]` in
    /// `.snapperrc.toml`. Empty by default; an empty map disables all
    /// per-language code-block behaviour (block passes through untouched).
    pub code: HashMap<String, CodeLang>,
    /// When `true`, the reflow stage invokes each language's `formatter`
    /// after comment reflow. Default `false` preserves v0.7.7 behaviour
    /// (no subprocess is spawned).
    pub format_code: bool,
    /// Prefer soft breaks after independent-clause punctuation (sembr
    /// rule 5). When `max_width` is 0, every such mark that is already
    /// followed by whitespace starts a new line. When `max_width` is
    /// greater than 0, overflowing sentences prefer those marks.
    /// Default `false` keeps one sentence per line (greedy wrap only
    /// under `max_width`).
    pub clause_breaks: bool,
    /// Open a sentence at a lowercase word after a terminator and a space.
    /// Abbreviations such as `e.g.` still join. Default `false` keeps UAX #29 SB8.
    pub lowercase_starts: bool,
    /// Run `format_text` to a byte fixpoint (cap 4). Production default
    /// `true`; tests set `false` so a planner that needs the backstop fails.
    pub fixpoint_backstop: bool,
    /// After the fixpoint, a format-local oracle mismatch returns the
    /// original document. Production default `true`; tests set `false`
    /// and assert the oracle themselves.
    pub render_backstop: bool,
    /// Extra LaTeX environments treated as code (no reflow), added to
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
    /// luaexec, and codehigh.sty codehigh/demohigh plus starred twins.
    /// Empty keeps the built-in list.
    pub latex_verbatim_envs: Vec<String>,
    /// Extra LaTeX environments treated as structure (no reflow), added
    /// to `NON_PROSE_ENVS`. Empty keeps the built-in list.
    pub latex_structure_envs: Vec<String>,
    /// Extra LaTeX command names tokenized like `\verb` before split.
    /// Empty keeps verb/lstinline/lstinputlisting/spverb/mintinline/inputminted/mint/Verb/SaveVerb/VerbatimInput/piton/verbatiminput/PitonInputFile/PitonInputFileT/PitonInputFileF/PitonInputFileTF/tcbinputlisting/tcboxverb/inputpy/inputpycon/inputpylab/inputpylabcon/inputsympy/inputsympycon/py/pyc/pys/pyb/pyv/pycon and twins/sympy/pylab and twins/usefamily ruby/rb/julia/jl/matlab/octave/bash/sage/rust/rs/R/perl/pl/perlsix/psix/javascript/js and twins/pyq/pycq/pyfile/pyfileq/pyfilerepl/inputpygments/pygment/pythontexcustomc/inputpython/inputpythonfile/CatchFileBetweenTags/CatchFileBetweenDelims/ExecuteMetaData/CatchFileDef/CatchFileEdef/listinginput/verbatimtabinput/verbatimwrite/listingcont/sageinput/sageplot/sagestr/pyth/Scontents/typestored/getstored/mergesc/meaningsc/foreachsc/setupsc/countsc/cleanseqsc/UseVerb/UseVerbatim/LUseVerbatim/BUseVerbatim/DefineShortVerb/UndefineShortVerb/EscVerb/inputsc/VerbatimInsertBuffer/VerbatimClearBuffer/InsertBuffer/IterateBuffer/ClearBuffer/WriteBuffer/lstset/lstdefinestyle/lstMakeShortInline/lstDeleteShortInline/lstnewenvironment/verbinput/verbwrite.
    pub latex_verbatim_commands: Vec<String>,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            format: Format::Plaintext,
            max_width: 0,
            use_neural: false,
            neural_lang: "en".to_string(),
            neural_model_path: None,
            extra_abbreviations: vec![],
            use_pandoc: false,
            pandoc_format: None,
            #[cfg(feature = "pandoc")]
            pandoc_backend: parser::pandoc::PandocBackend::default(),
            code: HashMap::new(),
            format_code: false,
            clause_breaks: false,
            lowercase_starts: false,
            fixpoint_backstop: true,
            render_backstop: true,
            latex_verbatim_envs: vec![],
            latex_structure_envs: vec![],
            latex_verbatim_commands: vec![],
        }
    }
}

/// Typed error for invalid UTF-8 input. Branch with `error.downcast_ref`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("input is not valid UTF-8")]
pub struct InvalidUtf8Error;

/// Historical: the pandoc path used to refuse because the AST has no
/// source offsets. `format_text` now writes the reflowed AST through
/// pandoc's writer instead of splicing original bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("pandoc backend cannot splice original source bytes")]
pub struct PandocCannotSplice;

/// Maximum pipeline passes including the first. Cap hit (or an A/B cycle)
/// returns the original document unchanged.
const MAX_FORMAT_PASSES: usize = 4;

impl FormatConfig {
    /// Tests that assert planner properties (idempotence, oracle, splice)
    /// must disable both backstops so a planner that needs them fails.
    pub fn without_safety_backstops(mut self) -> Self {
        self.fixpoint_backstop = false;
        self.render_backstop = false;
        self
    }
}

/// Run `step` until the output is a byte fixpoint or the cap is hit.
///
/// A cycle (including A/B) or a cap miss returns `original`. `enabled`
/// false runs `step` once. Public so tests can inject a cycling step.
pub fn run_fixpoint<F>(original: &str, enabled: bool, mut step: F) -> Result<String>
where
    F: FnMut(&str) -> Result<String>,
{
    let once = step(original)?;
    if !enabled {
        return Ok(once);
    }
    let mut cur = once;
    let mut seen = std::collections::HashSet::new();
    seen.insert(original.to_string());
    seen.insert(cur.clone());
    for _ in 1..MAX_FORMAT_PASSES {
        let next = step(&cur)?;
        if next == cur {
            return Ok(cur);
        }
        if !seen.insert(next.clone()) {
            return Ok(original.to_string());
        }
        cur = next;
    }
    Ok(original.to_string())
}

/// Build the appropriate sentence splitter from config.
pub fn build_splitter(config: &FormatConfig) -> Result<Box<dyn SentenceSplitter>> {
    if config.use_neural {
        #[cfg(feature = "neural")]
        {
            let neural = if let Some(ref path) = config.neural_model_path {
                sentence::neural::NeuralSentenceSplitter::from_path_with_extras(
                    path,
                    &config.neural_lang,
                    &config.extra_abbreviations,
                )
            } else {
                sentence::neural::NeuralSentenceSplitter::with_extras(
                    &config.neural_lang,
                    &config.extra_abbreviations,
                )
            };
            Ok(Box::new(
                neural
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                    .with_format(config.format)
                    .with_verbatim_commands(config.latex_verbatim_commands.clone())
                    .with_lowercase_starts(config.lowercase_starts),
            ))
        }
        #[cfg(not(feature = "neural"))]
        {
            Err(anyhow::anyhow!(
                "neural sentence splitting requires the 'neural' feature"
            ))
        }
    } else {
        Ok(Box::new(
            UnicodeSentenceSplitter::for_lang(&config.neural_lang, &config.extra_abbreviations)
                .with_format(config.format)
                .with_verbatim_commands(config.latex_verbatim_commands.clone())
                .with_lowercase_starts(config.lowercase_starts),
        ))
    }
}

/// Format text with semantic line breaks.
pub fn format_text(input: &str, config: &FormatConfig) -> Result<String> {
    let splitter = build_splitter(config)?;
    format_text_with_splitter(input, config, splitter.as_ref())
}

/// Format raw bytes. Invalid UTF-8 is a hard error ([`InvalidUtf8Error`]).
pub fn format_bytes(input: &[u8], config: &FormatConfig) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(input).map_err(|_| anyhow::Error::new(InvalidUtf8Error))?;
    format_text(s, config).map(|s| s.into_bytes())
}

/// Format text using a pre-constructed splitter (avoids reloading models per file).
pub fn format_text_with_splitter(
    input: &str,
    config: &FormatConfig,
    splitter: &dyn SentenceSplitter,
) -> Result<String> {
    let had_trailing_newline = input.ends_with('\n');
    let uses_crlf = input.contains("\r\n");

    // Normalize to LF for processing, restore CRLF at the end if needed.
    let normalized;
    let work_input = if uses_crlf {
        normalized = input.replace("\r\n", "\n");
        &normalized
    } else {
        input
    };

    // The pandoc writer is not a native splice: it normalizes markup, so
    // the native region-tree oracle and byte fixpoint would veto a correct
    // write-through and return the original (no sentence breaks).
    // Markdown opted in via `--use-pandoc` is the exception: it is read and
    // written as gfm, and a render mismatch returns the input unchanged.
    let candidate = if config.use_pandoc {
        let written = format_once(work_input, config, splitter, config.format_code)?;
        if pandoc_markdown_oracle(config)
            && config.render_backstop
            && written != work_input
            && !oracle::matches_ex(
                config.format,
                work_input,
                &written,
                config.format_code,
                Some(config),
            )
        {
            eprintln!("snapper: pandoc markdown render check failed; left input unchanged");
            work_input.to_string()
        } else if pandoc_writes_markdown(config)
            && config.render_backstop
            && written != work_input
            && !oracle::inline_code_kept(work_input, &written)
        {
            eprintln!("snapper: pandoc changed an inline code span; left input unchanged");
            work_input.to_string()
        } else {
            written
        }
    } else {
        let once = format_once(work_input, config, splitter, config.format_code)?;
        // Later passes prove prose stability. External code formatters
        // already ran on the first pass; re-invoking them multiplies
        // timeout budgets and is not part of the planner fixpoint.
        let candidate = run_fixpoint(work_input, config.fixpoint_backstop, |cur| {
            if cur == work_input {
                Ok(once.clone())
            } else {
                format_once(cur, config, splitter, false)
            }
        })?;

        if config.render_backstop
            && candidate != work_input
            && !oracle::matches_ex(
                config.format,
                work_input,
                &candidate,
                config.format_code,
                Some(config),
            )
        {
            salvage_regions(work_input, config, splitter)?
        } else {
            candidate
        }
    };

    let mut output = candidate;

    // Preserve the original file's trailing newline convention.
    if had_trailing_newline && !output.ends_with('\n') {
        output.push('\n');
    } else if !had_trailing_newline {
        while output.ends_with('\n') {
            output.pop();
        }
    }

    // Restore CRLF if the input used it.
    if uses_crlf {
        output = output.replace('\n', "\r\n");
    }

    Ok(output)
}

/// When the whole-document render check fails, keep each region whose
/// own rewrite still matches and leave the rest as original bytes.
fn salvage_regions(
    work_input: &str,
    config: &FormatConfig,
    splitter: &dyn SentenceSplitter,
) -> Result<String> {
    use crate::reflow::{ReflowConfig, apply_rewrites, region_rewrites};

    let spanned =
        parser::parser_for_format_config(config.format, Some(config)).parse_full(work_input);
    let reflow_config = ReflowConfig {
        max_width: config.max_width,
        code: Some(&config.code),
        format_code: config.format_code,
        clause_breaks: config.clause_breaks,
        format: config.format,
    };
    let rewrites = match region_rewrites(work_input, &spanned, splitter, &reflow_config) {
        Ok(rewrites) => rewrites,
        Err(_) => {
            let (start, end) = byte_line_span(work_input, 0, work_input.len());
            eprintln!("snapper: render check failed; left lines {start}-{end} unchanged");
            return Ok(work_input.to_string());
        }
    };
    let mut kept = Vec::new();
    let mut rejected = Vec::new();
    for (start, end, repl) in rewrites {
        let mut candidate = String::with_capacity(work_input.len() - (end - start) + repl.len());
        candidate.push_str(&work_input[..start]);
        candidate.push_str(&repl);
        candidate.push_str(&work_input[end..]);
        if oracle::matches_ex(
            config.format,
            work_input,
            &candidate,
            config.format_code,
            Some(config),
        ) {
            kept.push((start, end, repl));
        } else {
            rejected.push(byte_line_span(work_input, start, end));
        }
    }
    if !rejected.is_empty() {
        let ranges = rejected
            .iter()
            .map(|(start, end)| format!("{start}-{end}"))
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("snapper: render check failed; left lines {ranges} unchanged");
    }
    apply_rewrites(work_input, &kept).map_err(|e| anyhow::anyhow!("{e}"))
}

/// 1-indexed inclusive line numbers covering `[start, end)`.
fn byte_line_span(source: &str, start: usize, end: usize) -> (usize, usize) {
    let start_line = byte_to_line(source, start);
    let end_at = end.saturating_sub(1).max(start);
    let end_line = byte_to_line(source, end_at);
    (start_line, end_line.max(start_line))
}

fn byte_to_line(source: &str, byte: usize) -> usize {
    let byte = byte.min(source.len());
    source[..byte].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Pandoc reader/writer name for this config.
/// Markdown defaults to `gfm` so `--use-pandoc` does not emit pandoc tables.
fn pandoc_input_format(config: &FormatConfig) -> &str {
    if let Some(fmt) = config.pandoc_format.as_deref() {
        return fmt;
    }
    match config.format {
        Format::Org => "org",
        Format::Latex => "latex",
        Format::Markdown => "gfm",
        Format::Rst => "rst",
        Format::Plaintext => "markdown",
        // Pandoc's typst reader is not the native path. The name is here
        // so `--use-pandoc --format typst` asks pandoc for typst instead
        // of silently rereading the file as markdown.
        Format::Typst => "typst",
        // Pandoc's name for AsciiDoc. Native `.adoc` files do not use it
        // unless `--use-pandoc` is set.
        Format::AsciiDoc => "asciidoc",
    }
}

/// True when a pandoc markdown write should be checked with the render oracle.
fn pandoc_markdown_oracle(config: &FormatConfig) -> bool {
    config.format == Format::Markdown && pandoc_input_format(config) == "gfm"
}

/// True when the pandoc path writes markdown back, whatever the reader was.
/// Pandoc's `markdown` reader trims the space before an inner backtick, so
/// `` `a (b `c`)` `` comes back as `` `a (b`c`)` ``.
fn pandoc_writes_markdown(config: &FormatConfig) -> bool {
    matches!(config.format, Format::Markdown | Format::Plaintext)
}

/// One parse+reflow pass. Native parsers splice into original bytes;
/// pandoc reflows the AST and writes through pandoc's writer.
fn format_once(
    work_input: &str,
    config: &FormatConfig,
    splitter: &dyn SentenceSplitter,
    format_code: bool,
) -> Result<String> {
    use crate::parser::SpannedRegion;
    use crate::reflow::reflow_spanned;

    let reflow_config = ReflowConfig {
        max_width: config.max_width,
        code: Some(&config.code),
        format_code,
        clause_breaks: config.clause_breaks,
        format: config.format,
    };

    // Two pipelines:
    // - use_pandoc: parse → reflow Para/Plain in the AST → pandoc writer.
    // - else: native line parsers (markdown/org/…) then splice. Never mixed after success.
    if config.use_pandoc {
        #[cfg(feature = "pandoc")]
        {
            let pandoc_fmt = pandoc_input_format(config);
            return parser::pandoc::format_via_pandoc(
                work_input,
                pandoc_fmt,
                config.pandoc_backend,
                splitter,
                &reflow_config,
            )
            .map_err(|e| anyhow::anyhow!("{e}"));
        }
        #[cfg(not(feature = "pandoc"))]
        {
            return Err(anyhow::anyhow!(
                "pandoc backend requires the 'pandoc' feature"
            ));
        }
    }

    let spanned: Vec<SpannedRegion> =
        parser::parser_for_format_config(config.format, Some(config)).parse_full(work_input);
    match reflow_spanned(work_input, &spanned, splitter, &reflow_config) {
        Ok(out) => Ok(out),
        Err(_) => Ok(work_input.to_string()),
    }
}

/// Format only lines within a range (1-indexed, inclusive).
/// Lines outside the range pass through unchanged.
///
/// Each source line keeps the ending it had (`\n`, `\r\n`, `\r`, or none).
/// A file that mixes LF and CRLF is not rewritten to one style. New sentence
/// breaks inside a line use that line's ending.
pub fn format_range(
    input: &str,
    config: &FormatConfig,
    start: usize,
    end: usize,
) -> Result<String> {
    let lines = split_keep_endings(input);
    let total = lines.len();

    let start = start.max(1);
    let end = end.min(total);

    if start > total || start > end {
        return Ok(input.to_string());
    }

    let mut result = String::new();
    for (text, ending) in &lines[..start - 1] {
        result.push_str(text);
        result.push_str(ending);
    }

    let range = &lines[start - 1..end];
    let range_text = range
        .iter()
        .map(|(text, _)| *text)
        .collect::<Vec<_>>()
        .join("\n");
    let endings: Vec<&str> = range.iter().map(|(_, ending)| *ending).collect();
    let first_ending = endings.first().copied().unwrap_or("\n");
    let mixed_endings = endings.iter().any(|ending| *ending != first_ending);
    // A uniform ending is rewritten in one pass below, so it stays one
    // `format_text`. Mixed endings reuse that formatted string. Sentence
    // counts from the splitter built for it attribute each source line,
    // instead of formatting the range a second time.
    let (formatted, expanded) = if !mixed_endings {
        let formatted = format_text(&range_text, config)?;
        (formatted, vec![first_ending])
    } else if !sentence_fast_path_allowed(config)
        || (config.format == Format::Markdown && markdown_range_joins(range))
    {
        let formatted = format_text(&range_text, config)?;
        let expanded = endings_for_split_lines(range, &formatted, config, None);
        (formatted, expanded)
    } else {
        let splitter = build_splitter(config)?;
        let formatted = format_text_with_splitter(&range_text, config, splitter.as_ref())?;
        let expanded = endings_for_split_lines(range, &formatted, config, Some(splitter.as_ref()));
        (formatted, expanded)
    };
    let formatted = apply_range_endings(&formatted, &expanded);
    let formatted_ends = formatted.ends_with('\n');
    result.push_str(&formatted);

    if end < total && !formatted_ends {
        let sep = endings.last().copied().unwrap_or("\n");
        if sep.is_empty() {
            result.push('\n');
        } else {
            result.push_str(sep);
        }
    }
    for (text, ending) in &lines[end..] {
        result.push_str(text);
        result.push_str(ending);
    }

    if end == total {
        let last = endings.last().copied().unwrap_or("");
        if !last.is_empty() && !result.ends_with('\n') {
            result.push_str(last);
        }
    }

    Ok(result)
}

/// `(text, ending)` pairs. `ending` is `\n`, `\r\n`, `\r`, or empty at EOF.
fn split_keep_endings(input: &str) -> Vec<(&str, &str)> {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            out.push((&input[start..i], &input[i..i + 1]));
            i += 1;
            start = i;
        } else if bytes[i] == b'\r' {
            let end = if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i + 2
            } else {
                i + 1
            };
            out.push((&input[start..i], &input[i..end]));
            i = end;
            start = i;
        } else {
            i += 1;
        }
    }
    if start < input.len() {
        out.push((&input[start..], ""));
    }
    out
}

/// How many lines `format_text` emits, ignoring a trailing break.
fn output_line_count(formatted: &str) -> usize {
    let trimmed = formatted.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() {
        1
    } else {
        trimmed.lines().count().max(1)
    }
}

fn formatted_line_count(text: &str, config: &FormatConfig) -> usize {
    if text.is_empty() {
        return 1;
    }
    match format_text(text, config) {
        Ok(formatted) => output_line_count(&formatted),
        Err(_) => 1,
    }
}

/// Sentence counts may attribute a mixed-ending range only when each source
/// line's own sentences are the formatted lines assigned to it.
///
/// A sum that matches while two lines over- and under-count is rejected.
/// `max_width` and `clause_breaks` change the line count without a second
/// sentence, so they never take this path.
fn sentence_counts_matching_output(
    range: &[(&str, &str)],
    formatted: &str,
    config: &FormatConfig,
    splitter: Option<&dyn crate::sentence::SentenceSplitter>,
) -> Option<Vec<usize>> {
    if !sentence_fast_path_allowed(config) {
        return None;
    }
    let splitter = splitter?;
    let counts: Vec<usize> = range
        .iter()
        .map(|(text, _)| {
            if text.trim().is_empty() {
                1
            } else {
                splitter.split(text).len().max(1)
            }
        })
        .collect();
    let lines = formatted_body_lines(formatted);
    if counts.iter().sum::<usize>() != lines.len() {
        return None;
    }
    if !counts_match_each_line(range, &lines, &counts, splitter) {
        return None;
    }
    #[cfg(test)]
    note_fast_path();
    Some(counts)
}

/// Width and clause breaks insert line breaks the sentence splitter does not count.
fn sentence_fast_path_allowed(config: &FormatConfig) -> bool {
    config.max_width == 0 && !config.clause_breaks
}

fn formatted_body_lines(formatted: &str) -> Vec<&str> {
    let trimmed = formatted.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() {
        vec![""]
    } else {
        trimmed.lines().collect()
    }
}

fn counts_match_each_line(
    range: &[(&str, &str)],
    lines: &[&str],
    counts: &[usize],
    splitter: &dyn crate::sentence::SentenceSplitter,
) -> bool {
    let mut offset = 0usize;
    for ((text, _), &count) in range.iter().zip(counts) {
        let end = offset + count;
        if end > lines.len() {
            return false;
        }
        if !source_matches_slice(text, &lines[offset..end], splitter) {
            return false;
        }
        offset = end;
    }
    offset == lines.len()
}

fn source_matches_slice(
    text: &str,
    slice: &[&str],
    splitter: &dyn crate::sentence::SentenceSplitter,
) -> bool {
    if text.trim().is_empty() {
        return slice.len() == 1 && slice[0].trim().is_empty();
    }
    let sentences = splitter.split(text);
    sentences.len() == slice.len()
        && sentences
            .iter()
            .zip(slice)
            .all(|(sentence, line)| sentence.trim() == line.trim())
}

#[cfg(test)]
thread_local! {
    static FAST_PATH_HITS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn note_fast_path() {
    FAST_PATH_HITS.with(|hits| hits.set(hits.get() + 1));
}

#[cfg(test)]
fn reset_fast_path_hits() {
    FAST_PATH_HITS.with(|hits| hits.set(0));
}

#[cfg(test)]
fn fast_path_hits() -> u32 {
    FAST_PATH_HITS.with(|hits| hits.get())
}

/// How many formatted lines each source line becomes.
///
/// The fallback when sentence counts from the already formatted text do not
/// match its line count. Plain text uses one `format_text` of the range,
/// with a private-use mark between lines. Markdown that joins a blockquote,
/// list, or heading (including a lazy continuation) cannot use that mark, so
/// those ranges count each source line on its own.
fn line_counts(range: &[(&str, &str)], config: &FormatConfig) -> Vec<usize> {
    if config.format == Format::Markdown && markdown_range_joins(range) {
        return per_line_counts(range, config);
    }
    batched_line_counts(range, config).unwrap_or_else(|| per_line_counts(range, config))
}

fn per_line_counts(range: &[(&str, &str)], config: &FormatConfig) -> Vec<usize> {
    range
        .iter()
        .map(|(text, _)| formatted_line_count(text, config))
        .collect()
}

/// A Markdown range whose lines the parser can join.
///
/// A later line with no marker is still a lazy continuation of a quote or
/// list earlier in the range, so one container is enough to skip the batch.
fn markdown_range_joins(range: &[(&str, &str)]) -> bool {
    range.iter().any(|(text, _)| markdown_container_line(text))
}

fn markdown_container_line(text: &str) -> bool {
    let trimmed = text.trim_start();
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    if bytes[0] == b'>' || bytes[0] == b'#' {
        return true;
    }
    if bytes.len() >= 2 && matches!(bytes[0], b'-' | b'*' | b'+') && bytes[1].is_ascii_whitespace()
    {
        return true;
    }
    let mut digits = 0usize;
    while digits < bytes.len() && bytes[digits].is_ascii_digit() {
        digits += 1;
        if digits > 9 {
            return false;
        }
    }
    digits > 0
        && digits + 1 < bytes.len()
        && matches!(bytes[digits], b'.' | b')')
        && bytes[digits + 1].is_ascii_whitespace()
}

fn batched_line_counts(range: &[(&str, &str)], config: &FormatConfig) -> Option<Vec<usize>> {
    if range.is_empty() {
        return Some(Vec::new());
    }
    if range.len() == 1 {
        return Some(vec![formatted_line_count(range[0].0, config)]);
    }
    const MARK: &str = "\u{E000}";
    if range.iter().any(|(text, _)| text.contains(MARK)) {
        return None;
    }
    let mut doc = String::new();
    for (index, (text, _)) in range.iter().enumerate() {
        if index > 0 {
            doc.push('\n');
            doc.push_str(MARK);
            doc.push('\n');
        }
        doc.push_str(text);
    }
    let formatted = format_text(&doc, config).ok()?;
    if formatted.matches(MARK).count() != range.len() - 1 {
        return None;
    }
    let parts: Vec<&str> = formatted.split(MARK).collect();
    if parts.len() != range.len() {
        return None;
    }
    Some(
        parts
            .iter()
            .map(|part| {
                let trimmed = part.trim_matches(['\n', '\r']);
                if trimmed.is_empty() {
                    1
                } else {
                    output_line_count(trimmed)
                }
            })
            .collect(),
    )
}

/// One ending per formatted line.
///
/// A source line that splits into several sentences is repeated, so each new
/// line keeps the ending of the line it came from. When the joined reflow
/// disagrees with the per-line counts, the extra breaks stay with the line
/// that split instead of borrowing the next line's ending.
fn endings_for_split_lines<'a>(
    range: &[(&'a str, &'a str)],
    formatted: &str,
    config: &FormatConfig,
    splitter: Option<&dyn crate::sentence::SentenceSplitter>,
) -> Vec<&'a str> {
    if range.is_empty() {
        return Vec::new();
    }
    // Every source line already shares an ending, so the formatted text is
    // reused as-is. `apply_range_endings` rewrites a uniform ending in one
    // pass and does not need a per-line count.
    let first = range[0].1;
    if range.iter().all(|(_, ending)| *ending == first) {
        return vec![first];
    }
    let output_lines = output_line_count(formatted);
    let mut counts =
        if let Some(counts) = sentence_counts_matching_output(range, formatted, config, splitter) {
            counts
        } else {
            line_counts(range, config)
        };
    let sum: usize = counts.iter().sum();
    if output_lines > sum {
        let extra = output_lines - sum;
        let idx = counts
            .iter()
            .rposition(|count| *count > 1)
            .unwrap_or(counts.len() - 1);
        counts[idx] += extra;
    } else if output_lines < sum {
        let mut drop_n = sum - output_lines;
        for count in counts.iter_mut().rev() {
            if drop_n == 0 {
                break;
            }
            let drop = count.saturating_sub(1).min(drop_n);
            *count -= drop;
            drop_n -= drop;
        }
    }
    let mut expanded = Vec::with_capacity(output_lines);
    for (&count, (_, ending)) in counts.iter().zip(range.iter()) {
        expanded.extend(std::iter::repeat_n(*ending, count));
    }
    expanded
}

/// Put each formatted newline back in the ending of the output line it closes.
///
/// `endings` has one entry per formatted line, already expanded so a split
/// line repeats its own ending. A uniform range is rewritten in one pass.
fn apply_range_endings(formatted: &str, endings: &[&str]) -> String {
    if endings.is_empty() {
        return formatted.to_string();
    }
    let uniform = endings.iter().all(|ending| *ending == endings[0]);
    if uniform {
        return match endings[0] {
            "\r\n" => formatted.replace('\n', "\r\n"),
            "\r" => formatted.replace('\n', "\r"),
            _ => formatted.to_string(),
        };
    }
    let parts: Vec<&str> = formatted.split('\n').collect();
    let mut out = String::new();
    let mut src = 0usize;
    for (index, part) in parts.iter().enumerate() {
        out.push_str(part);
        if index + 1 == parts.len() {
            break;
        }
        let ending = if src < endings.len() {
            let ending = endings[src];
            src += 1;
            ending
        } else {
            endings[endings.len() - 1]
        };
        if ending.is_empty() {
            out.push('\n');
        } else {
            out.push_str(ending);
        }
    }
    out
}

#[cfg(test)]
mod region_fallback_tests {
    use super::*;
    use crate::format::Format;

    #[test]
    fn format_range_keeps_each_lines_ending() {
        let cfg = FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        };
        let mixed = "Already fine.\nOther fine.\r\n";
        let kept = format_range(mixed, &cfg, 1, 1).unwrap();
        assert_eq!(
            kept, mixed,
            "an untouched LF line must stay LF when a later line is CRLF"
        );
        let input = "Two sentences. Here.\r\n\r\nLeave this.\n";
        let out = format_range(input, &cfg, 1, 1).unwrap();
        assert_eq!(
            out, "Two sentences.\r\nHere.\r\n\r\nLeave this.\n",
            "the reflowed line keeps CRLF and the untouched line keeps LF"
        );
    }

    fn md_cfg() -> FormatConfig {
        FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        }
    }

    /// `format_range`, but line counts always come from formatting each
    /// source line on its own. Markdown joins blockquotes, lists, and lazy
    /// continuations across a batch mark, so this is the reference the
    /// batched counts have to match.
    fn format_range_per_line(
        input: &str,
        config: &FormatConfig,
        start: usize,
        end: usize,
    ) -> Result<String> {
        let lines = split_keep_endings(input);
        let total = lines.len();
        let start = start.max(1);
        let end = end.min(total);
        if start > total || start > end {
            return Ok(input.to_string());
        }
        let mut result = String::new();
        for (text, ending) in &lines[..start - 1] {
            result.push_str(text);
            result.push_str(ending);
        }
        let range = &lines[start - 1..end];
        let range_text = range
            .iter()
            .map(|(text, _)| *text)
            .collect::<Vec<_>>()
            .join("\n");
        let formatted = format_text(&range_text, config)?;
        let endings: Vec<&str> = range.iter().map(|(_, ending)| *ending).collect();
        let expanded = endings_per_line(range, &formatted, config);
        let formatted = apply_range_endings(&formatted, &expanded);
        let formatted_ends = formatted.ends_with('\n');
        result.push_str(&formatted);
        if end < total && !formatted_ends {
            let sep = endings.last().copied().unwrap_or("\n");
            if sep.is_empty() {
                result.push('\n');
            } else {
                result.push_str(sep);
            }
        }
        for (text, ending) in &lines[end..] {
            result.push_str(text);
            result.push_str(ending);
        }
        if end == total {
            let last = endings.last().copied().unwrap_or("");
            if !last.is_empty() && !result.ends_with('\n') {
                result.push_str(last);
            }
        }
        Ok(result)
    }

    fn endings_per_line<'a>(
        range: &[(&'a str, &'a str)],
        formatted: &str,
        config: &FormatConfig,
    ) -> Vec<&'a str> {
        if range.is_empty() {
            return Vec::new();
        }
        let output_lines = output_line_count(formatted);
        let mut counts: Vec<usize> = range
            .iter()
            .map(|(text, _)| formatted_line_count(text, config))
            .collect();
        let sum: usize = counts.iter().sum();
        if output_lines > sum {
            let extra = output_lines - sum;
            let idx = counts
                .iter()
                .rposition(|count| *count > 1)
                .unwrap_or(counts.len() - 1);
            counts[idx] += extra;
        } else if output_lines < sum {
            let mut drop_n = sum - output_lines;
            for count in counts.iter_mut().rev() {
                if drop_n == 0 {
                    break;
                }
                let drop = count.saturating_sub(1).min(drop_n);
                *count -= drop;
                drop_n -= drop;
            }
        }
        let mut expanded = Vec::with_capacity(output_lines);
        for (&count, (_, ending)) in counts.iter().zip(range.iter()) {
            expanded.extend(std::iter::repeat_n(*ending, count));
        }
        expanded
    }

    fn assert_markdown_matches_per_line(input: &str) {
        let cfg = md_cfg();
        let n = split_keep_endings(input).len();
        let out = format_range(input, &cfg, 1, n).unwrap();
        let expected = format_range_per_line(input, &cfg, 1, n).unwrap();
        assert_eq!(out, expected, "input {input:?}");
    }

    #[test]
    fn markdown_heading_keeps_its_lf_beside_a_blockquote() {
        let input = "> Quote one. Quote two.\r# Heading\nOne. Two.\r";
        assert_markdown_matches_per_line(input);
        let out = format_range(input, &md_cfg(), 1, 3).unwrap();
        assert!(
            out.contains("# Heading\n"),
            "a heading's LF must not become CR, got {out:?}"
        );
        assert!(
            !out.contains("# Heading\r"),
            "a heading must not take a neighboring CR, got {out:?}"
        );
    }

    #[test]
    fn markdown_blockquote_split_keeps_its_own_ending() {
        let input = "> Quote one. Quote two.\r\nSentence without a break\r1. First. Second.\n";
        assert_markdown_matches_per_line(input);
    }

    #[test]
    fn markdown_list_split_keeps_its_own_ending() {
        let input = "1. First. Second.\n- Item one. Item two.\r\n> Quote one. Quote two.\r\n";
        assert_markdown_matches_per_line(input);
    }

    #[test]
    fn markdown_lazy_continuation_keeps_its_own_ending() {
        let input = "> Quote starts.\nlazy continuation. Second sentence.\r\n";
        assert_markdown_matches_per_line(input);
    }

    #[test]
    fn markdown_endings_match_per_line_counts_on_random_input() {
        let snippets = [
            "One. Two.",
            "Already fine.",
            "# Heading",
            "## Sub heading",
            "> Quote one. Quote two.",
            "> Fine.",
            "- Item one. Item two.",
            "- Fine.",
            "1. First. Second.",
            "lazy continuation. Second sentence.",
            "",
            "> ",
            "Sentence without a break",
            "Mr. Smith went home. Then he left.",
        ];
        let endings = ["\n", "\r\n", "\r"];
        let mut state = 0xC0FFEE_u64;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            state
        };
        let mut diffs = 0usize;
        let mut cr_bad = 0usize;
        let mut plain_diffs = 0usize;
        let mut shown = 0usize;
        for _ in 0..400 {
            let n = 2 + (next() as usize % 5);
            let mut input = String::new();
            for i in 0..n {
                input.push_str(snippets[next() as usize % snippets.len()]);
                if i + 1 == n && next() % 5 == 0 {
                    continue;
                }
                input.push_str(endings[next() as usize % endings.len()]);
            }
            let plain = next() % 4 == 0;
            let cfg = if plain {
                FormatConfig {
                    format: Format::Plaintext,
                    ..Default::default()
                }
            } else {
                md_cfg()
            };
            let lines = split_keep_endings(&input);
            let out = format_range(&input, &cfg, 1, lines.len()).unwrap();
            let expected = format_range_per_line(&input, &cfg, 1, lines.len()).unwrap();
            if out != expected {
                diffs += 1;
                if plain {
                    plain_diffs += 1;
                }
                if out.matches('\r').count() != expected.matches('\r').count() {
                    cr_bad += 1;
                }
                if shown < 5 {
                    shown += 1;
                    eprintln!("IN  {input:?}\nOUT {out:?}\nEXP {expected:?}");
                }
            }
        }
        assert_eq!(
            diffs, 0,
            "{diffs} of 400 random documents disagreed with per-line counts ({cr_bad} lost or doubled a CR, {plain_diffs} plaintext)"
        );
    }

    /// Release timing for Markdown range formatting.
    ///
    /// `format_text` is one parse. `format_range` of the same prose has to
    /// stay in that neighborhood. The ignored test is the release measurement
    /// at 500, 1000, 2000 and 4000 lines.
    #[test]
    #[ignore = "release timing; run with cargo test --release -- --ignored"]
    fn markdown_range_scale_stays_near_one_format_pass() {
        let cfg = md_cfg();
        let mut ratio_2000 = 0.0;
        for n in [500usize, 1000, 2000, 4000] {
            let mut doc = String::new();
            for i in 0..n {
                doc.push_str(&format!(
                    "Sentence {i} stands here today. Another follows it immediately.\n"
                ));
            }
            let lines = split_keep_endings(&doc).len();
            let start_line = (lines / 4).max(1);
            let end_line = (lines * 3 / 4).max(start_line);
            let slice = split_keep_endings(&doc)[start_line - 1..end_line]
                .iter()
                .map(|(text, ending)| format!("{text}{ending}"))
                .collect::<String>();
            let mut whole = Vec::new();
            let mut ranged = Vec::new();
            let mut once_whole = Vec::new();
            let mut once_range = Vec::new();
            let runs = 5;
            for i in 0..runs + 1 {
                let start = std::time::Instant::now();
                let out = format_range(&doc, &cfg, 1, lines).unwrap();
                let whole_s = start.elapsed().as_secs_f64();
                std::hint::black_box(&out);
                let start = std::time::Instant::now();
                let out = format_text(&doc, &cfg).unwrap();
                let once_whole_s = start.elapsed().as_secs_f64();
                std::hint::black_box(&out);
                let start = std::time::Instant::now();
                let out = format_range(&doc, &cfg, start_line, end_line).unwrap();
                let range_s = start.elapsed().as_secs_f64();
                std::hint::black_box(&out);
                let start = std::time::Instant::now();
                let out = format_text(&slice, &cfg).unwrap();
                let once_range_s = start.elapsed().as_secs_f64();
                std::hint::black_box(&out);
                if i == 0 {
                    continue;
                }
                whole.push(whole_s);
                once_whole.push(once_whole_s);
                ranged.push(range_s);
                once_range.push(once_range_s);
            }
            let med = |samples: &mut Vec<f64>| {
                samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
                samples[samples.len() / 2]
            };
            let whole_m = med(&mut whole);
            let once_m = med(&mut once_whole);
            let range_m = med(&mut ranged);
            let once_range_m = med(&mut once_range);
            let ratio = whole_m / once_m;
            eprintln!(
                "n={n} whole_median={whole_m:.6}s one_pass_median={once_m:.6}s whole/one={ratio:.2} range_median={range_m:.6}s range_one_pass={once_range_m:.6}s range/one={:.2} whole={whole:?}",
                range_m / once_range_m
            );
            if n == 2000 {
                ratio_2000 = ratio;
            }
        }
        assert!(
            ratio_2000 < 2.0,
            "a 2000-line Markdown range must stay within 2x of one format pass, got {ratio_2000:.2}x"
        );
    }

    /// Mixed LF and CRLF must not format the range a second time.
    #[test]
    #[ignore = "release timing; run with cargo test --release -- --ignored"]
    fn mixed_ending_range_stays_near_one_format_pass() {
        let cfg = md_cfg();
        let mut ratio_1000 = 0.0;
        for n in [400usize, 800, 1600] {
            let mut doc = String::new();
            for i in 0..n {
                let ending = if i % 2 == 0 { "\n" } else { "\r\n" };
                doc.push_str(&format!(
                    "Sentence {i} stands here today. Another follows it immediately.{ending}"
                ));
            }
            let lines = split_keep_endings(&doc).len();
            let mut whole = Vec::new();
            let mut once = Vec::new();
            for i in 0..4 {
                let start = std::time::Instant::now();
                let out = format_range(&doc, &cfg, 1, lines).unwrap();
                let whole_s = start.elapsed().as_secs_f64();
                std::hint::black_box(&out);
                let start = std::time::Instant::now();
                let out = format_text(&doc, &cfg).unwrap();
                let once_s = start.elapsed().as_secs_f64();
                std::hint::black_box(&out);
                if i == 0 {
                    continue;
                }
                whole.push(whole_s);
                once.push(once_s);
            }
            whole.sort_by(|a, b| a.partial_cmp(b).unwrap());
            once.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let whole_m = whole[whole.len() / 2];
            let once_m = once[once.len() / 2];
            let ratio = whole_m / once_m;
            eprintln!(
                "mixed n={n} whole_median={whole_m:.6}s one_pass_median={once_m:.6}s whole/one={ratio:.2} whole={whole:?}"
            );
            if n == 800 {
                ratio_1000 = ratio;
            }
        }
        assert!(
            ratio_1000 < 1.1,
            "a mixed-ending Markdown range must stay within 1.1x of one format pass, got {ratio_1000:.2}x"
        );
    }

    #[test]
    fn offsetting_sentence_counts_are_not_accepted() {
        struct Offset;
        impl crate::sentence::SentenceSplitter for Offset {
            fn split(&self, text: &str) -> Vec<String> {
                if text.contains("Alpha") {
                    vec!["a".into(), "b".into(), "c".into()]
                } else {
                    vec!["d".into()]
                }
            }
        }
        let range = [("Alpha. Beta.", "\n"), ("Gamma. Delta.", "\r\n")];
        let formatted = "Alpha.\nBeta.\nGamma.\nDelta.\n";
        let got = sentence_counts_matching_output(
            &range,
            formatted,
            &FormatConfig::default(),
            Some(&Offset),
        );
        assert_eq!(
            got, None,
            "counts that only sum to the output length must not attribute endings, got {got:?}"
        );
    }

    #[test]
    fn mixed_prose_keeps_each_lines_ending_on_the_fast_path() {
        let cfg = md_cfg();
        let input = "One. Two.\nThree. Four.\r\n";
        reset_fast_path_hits();
        let out = format_range(input, &cfg, 1, 2).unwrap();
        assert_eq!(out, "One.\nTwo.\nThree.\r\nFour.\r\n");
        assert!(
            fast_path_hits() > 0,
            "plain mixed prose must reuse the formatted sentences"
        );
    }

    #[test]
    fn width_or_clause_breaks_stay_off_the_fast_path_when_counts_would_match() {
        let input = "One. Two.\nThree. Four.\r\n";
        for (max_width, clause_breaks) in [(80usize, false), (0, true)] {
            let cfg = FormatConfig {
                format: Format::Markdown,
                max_width,
                clause_breaks,
                ..Default::default()
            };
            reset_fast_path_hits();
            let out = format_range(input, &cfg, 1, 2).unwrap();
            assert_eq!(out, "One.\nTwo.\nThree.\r\nFour.\r\n");
            assert_eq!(
                fast_path_hits(),
                0,
                "max_width={max_width} clause_breaks={clause_breaks} must stay off the sentence fast path even when each line's sentences match the formatted lines"
            );
        }
    }

    #[test]
    fn width_and_clause_breaks_skip_the_sentence_fast_path() {
        let input = "Alpha, beta stands here. Gamma follows.\n\
Delta, epsilon stands here. Zeta follows.\r\n";
        for (max_width, clause_breaks) in [(24usize, false), (0, true)] {
            let cfg = FormatConfig {
                format: Format::Markdown,
                max_width,
                clause_breaks,
                ..Default::default()
            };
            reset_fast_path_hits();
            let out = format_range(input, &cfg, 1, 2).unwrap();
            let expected = format_range_per_line(input, &cfg, 1, 2).unwrap();
            assert_eq!(
                out, expected,
                "max_width={max_width} clause_breaks={clause_breaks} must match per-line endings"
            );
            assert_eq!(
                fast_path_hits(),
                0,
                "max_width={max_width} clause_breaks={clause_breaks} must not use sentence counts"
            );
        }
    }

    #[test]
    fn a_crlf_line_that_splits_keeps_crlf_on_every_new_line() {
        let cfg = FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        };
        let input = "One. Two. Three.\r\nAlready fine.\n";
        let out = format_range(input, &cfg, 1, 2).unwrap();
        assert_eq!(
            out, "One.\r\nTwo.\r\nThree.\r\nAlready fine.\n",
            "each sentence split out of the CRLF line must keep CRLF"
        );
    }

    #[test]
    fn a_split_in_the_middle_of_a_mixed_range_keeps_that_lines_ending() {
        let cfg = FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        };
        let input = "Keep.\nOne. Two.\r\nTail.\n";
        let out = format_range(input, &cfg, 1, 3).unwrap();
        assert_eq!(
            out, "Keep.\nOne.\r\nTwo.\r\nTail.\n",
            "the extra sentence must keep the CRLF of the line it was split from"
        );
    }

    #[test]
    fn rejected_paragraph_does_not_revert_the_file() {
        let input = "**Note.** Next sentence here. Third one.\n\nA. a0.A.\n";
        let out = format_text(
            input,
            &FormatConfig {
                format: Format::Markdown,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            out.starts_with("**Note.**\nNext sentence here.\nThird one.\n\n"),
            "first paragraph must split, got:\n{out}"
        );
        assert!(
            out.ends_with("A. a0.A.\n"),
            "rejected paragraph must stay unchanged, got:\n{out}"
        );
    }

    #[test]
    fn emphasis_closer_paragraph_splits_without_reverting() {
        let input =
            "**Note.** Next sentence here. Third one.\n\nThis is done.** Next starts here.\n";
        let out = format_text(
            input,
            &FormatConfig {
                format: Format::Markdown,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            out,
            "**Note.**\nNext sentence here.\nThird one.\n\nThis is done.**\nNext starts here.\n"
        );
    }
}

#[cfg(test)]
mod markdown_block_marker_tests {
    use super::*;
    use crate::format::Format;

    fn format_md(input: &str) -> String {
        format_text(
            input,
            &FormatConfig {
                format: Format::Markdown,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn opens_block_line(line: &str) -> bool {
        let t = line.trim_start();
        t == "1."
            || t.starts_with("1. ")
            || t.starts_with("1) ")
            || t.starts_with("- ")
            || t.starts_with("+ ")
            || t.starts_with("* ")
            || t.starts_with("# ")
            || t.starts_with("> ")
            || (!t.is_empty() && (t.bytes().all(|b| b == b'-') || t.bytes().all(|b| b == b'=')))
    }

    #[test]
    fn continuation_lines_do_not_start_with_block_markers() {
        let enumerated = format_md("All set. 1. Then foo happens.\n");
        assert_eq!(enumerated, "All set. 1.\nThen foo happens.\n");
        assert!(
            !enumerated.lines().any(|l| l.trim() == "1."),
            "{enumerated}"
        );

        let dashed = format_md("Done. - not a list.\n");
        assert!(
            dashed.lines().skip(1).all(|l| !opens_block_line(l)),
            "{dashed}"
        );
        assert!(dashed.contains("Done."));
        assert!(dashed.contains("not a list."));
        assert!(dashed.contains('-'));
    }
}

#[cfg(test)]
mod lowercase_starts_tests {
    use super::*;
    use crate::format::Format;

    fn cfg(on: bool) -> FormatConfig {
        FormatConfig {
            format: Format::Markdown,
            lowercase_starts: on,
            ..Default::default()
        }
    }

    fn lines(on: bool, input: &str) -> Vec<String> {
        format_text(input, &cfg(on))
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn lowercase_starts_split_the_documented_cases() {
        let tool = "There are two. setup-python moved to PyPy 8.0.0.";
        let meson = "`meson test`: 58 of 58 pass. pytest: 661 pass and 31 skip.";
        let abbrev = "Use e.g. this, i.e. that, vs. the other, etc. and more. Dr. Smith agreed.";
        assert_eq!(lines(false, tool), vec![tool]);
        assert_eq!(lines(false, meson), vec![meson]);
        assert_eq!(
            lines(true, tool),
            vec![
                "There are two.".to_string(),
                "setup-python moved to PyPy 8.0.0.".to_string()
            ]
        );
        assert_eq!(
            lines(true, meson),
            vec![
                "`meson test`: 58 of 58 pass.".to_string(),
                "pytest: 661 pass and 31 skip.".to_string()
            ]
        );
        assert_eq!(
            lines(true, abbrev),
            vec![
                "Use e.g. this, i.e. that, vs. the other, etc. and more.".to_string(),
                "Dr. Smith agreed.".to_string()
            ]
        );
    }

    #[test]
    fn period_in_code_span_or_escape_stays_one_line() {
        let pip = "`pip install -e .` and then the rest.";
        let into = "`1.` into the list.";
        let esc = "1\\. and the rest.";
        let seen = "See 1\\. and the rest.";
        for text in [pip, into, esc, seen] {
            assert_eq!(lines(false, text), vec![text], "default {text:?}");
            assert_eq!(lines(true, text), vec![text], "lowercase {text:?}");
        }
    }
}

/// GitHub #538: a list item or blockquote that ends the file with no final
/// newline must still split. The same inputs with a final newline are the
/// controls. A real render change at that position stays unchanged.
#[cfg(test)]
mod eof_newline_render_check_tests {
    use super::*;
    use crate::format::Format;

    fn format_md(input: &str) -> String {
        format_text(
            input,
            &FormatConfig {
                format: Format::Markdown,
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn trailing_list_item_without_final_newline_splits() {
        assert_eq!(format_md("- One. Two."), "- One.\n  Two.");
        assert_eq!(format_md("1. One. Two."), "1. One.\n   Two.");
        assert_eq!(format_md("x\n\n- One. Two."), "x\n\n- One.\n  Two.");
        let out = format_md("- One. Two.");
        assert_eq!(format_md(&out), out);
    }

    #[test]
    fn trailing_nested_list_item_without_final_newline_splits() {
        let input = "- outer\n  - One. Two.";
        let out = format_md(input);
        assert_eq!(out, "- outer\n  - One.\n    Two.");
        assert_eq!(format_md(&out), out);
    }

    #[test]
    fn trailing_blockquote_without_final_newline_splits() {
        let out = format_md("> One. Two.");
        assert_eq!(out, "> One.\n> Two.");
        assert_eq!(format_md(&out), out);
    }

    #[test]
    fn trailing_list_item_with_final_newline_splits() {
        assert_eq!(format_md("- One. Two.\n"), "- One.\n  Two.\n");
        assert_eq!(format_md("1. One. Two.\n"), "1. One.\n   Two.\n");
        assert_eq!(format_md("x\n\n- One. Two.\n"), "x\n\n- One.\n  Two.\n");
    }

    #[test]
    fn trailing_nested_list_item_with_final_newline_splits() {
        assert_eq!(
            format_md("- outer\n  - One. Two.\n"),
            "- outer\n  - One.\n    Two.\n"
        );
    }

    #[test]
    fn trailing_blockquote_with_final_newline_splits() {
        assert_eq!(format_md("> One. Two.\n"), "> One.\n> Two.\n");
    }

    #[test]
    fn render_check_keeps_a_real_difference_at_eof() {
        // Splitting `a0.A.` changes the prose words. The missing final
        // newline must not make the check accept that.
        assert_eq!(format_md("- A. a0.A."), "- A. a0.A.");
        assert_eq!(format_md("- A. a0.A.\n"), "- A. a0.A.\n");
    }

    #[test]
    fn trailing_hard_break_without_final_newline_splits() {
        // Two spaces or a backslash are a Markdown hard break. With a final
        // newline the sentences split and the break stays on the last line.
        // The same bytes without that newline must split the same way.
        assert_eq!(format_md("- One. Two.  "), "- One.\n  Two.  ");
        assert_eq!(format_md("> One. Two.  "), "> One.\n> Two.  ");
        assert_eq!(format_md("One. Two.  "), "One.\nTwo.  ");
        assert_eq!(format_md("- One. Two.\\"), "- One.\n  Two.\\");
        assert_eq!(format_md("One. Two.   "), "One.\nTwo.   ");
        assert_eq!(format_md("x\r\n- One. Two.  "), "x\r\n- One.\r\n  Two.  ");
        let out = format_md("- One. Two.  ");
        assert_eq!(format_md(&out), out);
        assert_eq!(format_md("- One. Two.  \n"), "- One.\n  Two.  \n");
        assert_eq!(format_md("One. Two.  \n"), "One.\nTwo.  \n");
        assert_eq!(format_md("- One. Two.\\\n"), "- One.\n  Two.\\\n");
    }

    #[test]
    fn lone_hard_break_line_at_eof_still_splits() {
        // A last line that holds only the break appends no word to the
        // prose, so the newline before it is the previous line's
        // terminator and not the paragraph's own trailing newline.
        // Suppressing that newline deletes a line and the render check
        // then keeps the paragraph fused.
        assert_eq!(format_md("One. Two.\n\\"), "One.\nTwo.\n\\");
        assert_eq!(
            format_md("Alpha. Beta. Gamma.\n\\"),
            "Alpha.\nBeta.\nGamma.\n\\"
        );
        assert_eq!(format_md("One. Two.\r\n\\"), "One.\r\nTwo.\r\n\\");
        assert_eq!(format_md("A. B.\n    x. y.\n\\"), "A. B.\nx. y.\n\\");
        assert_eq!(format_md("One. Two.\n\\\n"), "One.\nTwo.\n\\\n");
        let out = format_md("One. Two.\n\\");
        assert_eq!(format_md(&out), out);
    }

    #[test]
    fn lone_hard_break_line_after_an_indented_item_line_still_splits() {
        // Inside a list item the continuation line keeps its terminator out
        // of the prose span, so a following lone hard break has to take that
        // byte from the pending terminator rather than from the span. The
        // split reindents the line to column zero, where the same document
        // does take it from the span, and the render check then compares two
        // trees that disagree over a newline neither rewrite touched.
        assert_eq!(
            format_md("- One only.    \n    indented. code.\n\\\n"),
            "- One only.    \nindented. code.\n\\\n"
        );
        assert_eq!(
            format_md("- One. Two.  \n    Three. Four.\n\\\n"),
            "- One.\n  Two.  \nThree.\nFour.\n\\\n"
        );
        assert_eq!(
            format_md("1. One. Two.  \n    Three. Four.\n\\\n"),
            "1. One.\n   Two.  \nThree.\nFour.\n\\\n"
        );
        assert_eq!(
            format_md("- a. b.\n  - One only.  \n      Three. Four.\n\\\n"),
            "- a. b.\n  - One only.  \nThree.\nFour.\n\\\n"
        );
        assert_eq!(
            format_md("- One. Two.  \n    Three. Four.\n\\"),
            "- One.\n  Two.  \nThree.\nFour.\n\\"
        );
        let out = format_md("- One. Two.  \n    Three. Four.\n\\\n");
        assert_eq!(format_md(&out), out);
    }

    #[test]
    fn eof_hard_break_splits_under_every_block_marker() {
        assert_eq!(format_md("1. One. Two.  "), "1. One.\n   Two.  ");
        assert_eq!(format_md("1. One. Two.\\"), "1. One.\n   Two.\\");
        assert_eq!(format_md("* One. Two.  "), "* One.\n  Two.  ");
        assert_eq!(format_md("+ One. Two.\\"), "+ One.\n  Two.\\");
        assert_eq!(format_md("> One. Two.\\"), "> One.\n> Two.\\");
        assert_eq!(format_md("One. Two.\\"), "One.\nTwo.\\");
        assert_eq!(format_md("> > One. Two.  "), "> > One.\n> > Two.  ");
        assert_eq!(format_md("> > One. Two.\\"), "> > One.\n> > Two.\\");
        assert_eq!(format_md("> - One. Two.  "), "> - One.\n> Two.  ");
        assert_eq!(
            format_md("- outer\n  - One. Two.  "),
            "- outer\n  - One.\n    Two.  "
        );
        assert_eq!(
            format_md("- outer\n  - One. Two.\\"),
            "- outer\n  - One.\n    Two.\\"
        );
        assert_eq!(
            format_md("- a\n  - b\n    - One. Two.  "),
            "- a\n  - b\n    - One.\n      Two.  "
        );
        assert_eq!(
            format_md("- One. Two. Three.  "),
            "- One.\n  Two.\n  Three.  "
        );
        assert_eq!(
            format_md("- One.\n\n  Two. Three.  "),
            "- One.\n\n  Two.\n  Three.  "
        );
        assert_eq!(format_md("x\r\n- One. Two.\\"), "x\r\n- One.\r\n  Two.\\");
        // One sentence has nothing to split, so the bytes stay.
        assert_eq!(format_md("- One.  "), "- One.  ");
    }

    #[test]
    fn eof_bytes_that_are_not_a_markdown_hard_break() {
        // An even run of backslashes is an escaped backslash, not a break.
        assert_eq!(format_md("- One. Two.\\\\"), "- One.\n  Two.\\\\");
        assert_eq!(format_md("- One. Two.\\\\\\\\"), "- One.\n  Two.\\\\\\\\");
        // An odd run ends in a break, and only the last byte is the break.
        assert_eq!(format_md("- One. Two.\\\\\\"), "- One.\n  Two.\\\\\\");
        // One space, or a tab, is ordinary trailing whitespace.
        assert_eq!(format_md("- One. Two. "), "- One.\n  Two.");
        assert_eq!(format_md("- One. Two.\t\t"), "- One.\n  Two.");
        // Four spaces are still a break and still survive the split.
        assert_eq!(format_md("- One. Two.    "), "- One.\n  Two.    ");
    }

    #[test]
    fn eof_hard_break_leaves_code_alone() {
        // A backslash inside a code span is not a break, and the span
        // stays whole while the sentences split around it.
        assert_eq!(
            format_md("- One. Two is `a\\b`.  "),
            "- One.\n  Two is `a\\b`.  "
        );
        // A backslash inside a code block is code, open fence or not.
        assert_eq!(format_md("```\nOne. Two.\\\n```"), "```\nOne. Two.\\\n```");
        assert_eq!(format_md("```\nOne. Two.\\"), "```\nOne. Two.\\");
        assert_eq!(format_md("    One. Two.\\"), "    One. Two.\\");
    }

    #[test]
    fn hard_break_mid_file_and_at_eof_both_survive() {
        assert_eq!(
            format_md("One. Two.  \nThree. Four.  "),
            "One.\nTwo.  \nThree.\nFour.  "
        );
        assert_eq!(
            format_md("One. Two.  \nThree. Four.\n"),
            "One.\nTwo.  \nThree.\nFour.\n"
        );
        assert_eq!(
            format_md("- One. Two.  \n- Three. Four."),
            "- One.\n  Two.  \n- Three.\n  Four."
        );
        assert_eq!(
            format_md("One. Two.  \r\nThree. Four.  "),
            "One.\r\nTwo.  \r\nThree.\r\nFour.  "
        );
    }

    #[test]
    fn eof_hard_break_is_markdown_only() {
        // Two spaces and a lone backslash mean nothing at the end of a
        // plaintext, RST, or LaTeX file, so the trailing bytes go.
        for format in [Format::Plaintext, Format::Rst, Format::Latex] {
            let config = FormatConfig {
                format,
                ..Default::default()
            };
            assert_eq!(
                format_text("One. Two.  ", &config).unwrap(),
                "One.\nTwo.",
                "{format:?}"
            );
        }
        // Org marks a break with `\\`, and already keeps it at end of file.
        let org = FormatConfig {
            format: Format::Org,
            ..Default::default()
        };
        assert_eq!(
            format_text("One. Two.\\\\", &org).unwrap(),
            "One.\nTwo.\\\\"
        );
    }
}
