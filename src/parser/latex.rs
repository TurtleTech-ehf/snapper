use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{ByteSpan, FormatParser, SpannedRegion, flush_prose_spanned, iter_lines};

// Environments whose content is NOT prose (math, code, figures, tables)
static NON_PROSE_ENVS: &[&str] = &[
    "equation",
    "equation*",
    "align",
    "align*",
    "gather",
    "gather*",
    "multline",
    "multline*",
    "eqnarray",
    "eqnarray*",
    "figure",
    "figure*",
    "table",
    "table*",
    "tabular",
    "tabular*",
    "lstlisting",
    "verbatim",
    "minted",
    "tikzpicture",
    "array",
    "matrix",
    "pmatrix",
    "bmatrix",
];

static BEGIN_ENV_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\begin\{(\w+\*?)\}").unwrap());

static END_ENV_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\end\{(\w+\*?)\}").unwrap());

/// `\begin{minted}{LANG}` -- the language is the brace argument after the env.
static MINTED_LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\begin\{minted\}\s*(?:\[[^\]]*\])?\s*\{([^}]+)\}").unwrap());

/// `\begin{lstlisting}[language=LANG, ...]` -- language is an option key.
static LSTLISTING_LANG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\begin\{lstlisting\}\s*\[[^\]]*language\s*=\s*([A-Za-z0-9_+.\-]+)").unwrap()
});

/// Source-code environments whose body should be emitted as `Region::Code`.
fn is_code_env(name: &str) -> bool {
    matches!(name, "minted" | "lstlisting" | "verbatim")
}

static DISPLAY_MATH_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\\\[").unwrap());

static DISPLAY_MATH_CLOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\\]\s*$").unwrap());

/// Sectioning commands whose brace argument is prose (titles can be long).
/// Captures: (1) command + opening brace prefix, (2) argument body, (3) closing brace + rest.
static SECTION_CMD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\s*\\(?:part|chapter|section|subsection|subsubsection|paragraph|subparagraph)\*?\{)([^}]*)(\}.*)$",
    )
    .unwrap()
});

pub struct LatexParser;

impl LatexParser {
    fn is_comment(line: &str) -> bool {
        line.trim_start().starts_with('%')
    }

    /// Byte offset of the first `%` that is not escaped as `\%`.
    fn unescaped_percent(line: &str) -> Option<usize> {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if bytes[i] == b'%' {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    fn is_non_prose_env(name: &str) -> bool {
        NON_PROSE_ENVS.contains(&name)
    }
}

impl FormatParser for LatexParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let mut regions: Vec<SpannedRegion> = Vec::new();
        let mut current_prose = String::new();
        let mut prose_span: Option<ByteSpan> = None;
        let mut in_preamble = true;
        let mut in_non_prose_env: Option<String> = None;
        // Code environment bookkeeping.
        let mut in_code_env: Option<String> = None;
        let mut code_lang: Option<String> = None;
        let mut code_header = ByteSpan::default();
        let mut code_body_start = 0usize;
        let mut in_display_math = false;
        let mut pragma_off = false;
        // A `%` comment eats the newline, so the next physical line joins
        // this prose with no inserted space (`foo%\nbar` is TeX `foobar`).
        let mut nospace_join = false;

        for line in iter_lines(input) {
            let line_text = line.text;
            // Check for snapper:off/on pragmas; inside a code environment
            // the per-language reflow path handles pragmas instead.
            if in_code_env.is_none() {
                if let Some(on) = super::check_pragma(line_text) {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    pragma_off = !on;
                    regions.push(SpannedRegion::structure(input, line.span()));
                    continue;
                }

                if pragma_off {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(input, line.span()));
                    continue;
                }
            }

            // Preamble: everything before \begin{document} is structure
            if in_preamble {
                if line_text.contains(r"\begin{document}") {
                    in_preamble = false;
                }
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Inside code environment -- buffer body
            if let Some(env_name) = in_code_env.clone() {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let ends = END_ENV_RE
                    .captures(line_text)
                    .map(|c| c.get(1).unwrap().as_str() == env_name)
                    .unwrap_or(false);
                if ends {
                    in_code_env = None;
                    regions.push(SpannedRegion::code(
                        input,
                        code_lang.take(),
                        code_header,
                        ByteSpan::new(code_body_start, line.start),
                        line.span(),
                    ));
                }
                continue;
            }

            // Inside non-prose environment
            if let Some(ref env_name) = in_non_prose_env {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if let Some(caps) = END_ENV_RE.captures(line_text) {
                    if caps.get(1).unwrap().as_str() == env_name {
                        in_non_prose_env = None;
                    }
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Inside display math \[...\]
            if in_display_math {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if DISPLAY_MATH_CLOSE.is_match(line_text) {
                    in_display_math = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Blank line
            if line_text.trim().is_empty() {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                nospace_join = false;
                regions.push(SpannedRegion::blank(input, line.span()));
                continue;
            }

            // Comment
            if Self::is_comment(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                nospace_join = false;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Mid-line `%`: prefix is prose; `%` eats the newline (nospace
            // join). A comment with text is Structure after the prefix.
            if let Some(idx) = Self::unescaped_percent(line_text) {
                let code = &line_text[..idx];
                let comment = &line_text[idx..];
                if !code.trim().is_empty() {
                    if !current_prose.is_empty() && !nospace_join {
                        current_prose.push(' ');
                    }
                    current_prose.push_str(code.trim_start());
                    let start = line.start + (line_text.len() - line_text.trim_start().len());
                    let content_end = line.start + idx;
                    match &mut prose_span {
                        None => prose_span = Some(ByteSpan::new(start, content_end)),
                        Some(s) => s.end = content_end,
                    }
                }
                nospace_join = true;
                if comment.trim() != "%" {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    let comment_span = ByteSpan::new(line.start + idx, line.end);
                    regions.push(SpannedRegion::structure(input, comment_span));
                } else if let Some(s) = &mut prose_span {
                    // Lone `%` plus the newline stay inside the prose span so
                    // the rewrite covers `foo%\nbar` as one TeX word.
                    s.end = line.end;
                }
                continue;
            }

            // \end{document}
            if line_text.contains(r"\end{document}") {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Begin non-prose environment
            if let Some(caps) = BEGIN_ENV_RE.captures(line_text) {
                let env_name = caps.get(1).unwrap().as_str().to_string();
                if Self::is_non_prose_env(&env_name) {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    // Single-line \begin{...}...\end{...}: emit as Structure
                    // (or as an empty-body Code region for code envs) -- the
                    // common case is multi-line, so keep this path simple.
                    if let Some(end_caps) = END_ENV_RE.captures(line_text) {
                        if end_caps.get(1).unwrap().as_str() == env_name {
                            if is_code_env(&env_name) {
                                let empty = ByteSpan::new(line.end, line.end);
                                regions.push(SpannedRegion::code(
                                    input,
                                    None,
                                    line.span(),
                                    empty,
                                    empty,
                                ));
                            } else {
                                regions.push(SpannedRegion::structure(input, line.span()));
                            }
                            continue;
                        }
                    }
                    if is_code_env(&env_name) {
                        code_lang = if env_name == "minted" {
                            MINTED_LANG_RE
                                .captures(line_text)
                                .map(|c| c.get(1).unwrap().as_str().to_string())
                        } else if env_name == "lstlisting" {
                            LSTLISTING_LANG_RE
                                .captures(line_text)
                                .map(|c| c.get(1).unwrap().as_str().to_string())
                        } else {
                            None
                        };
                        code_header = line.span();
                        code_body_start = line.end;
                        in_code_env = Some(env_name);
                    } else {
                        in_non_prose_env = Some(env_name);
                        regions.push(SpannedRegion::structure(input, line.span()));
                    }
                    continue;
                }
            }

            // Sectioning commands: keep the entire line as Structure.
            // Splitting Structure(\section{)+Prose(title)+Structure(}) reflowed
            // multi-sentence titles mid-brace. Single-line sectioning is not
            // prose; do not reflow titles.
            if SECTION_CMD_RE.is_match(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Display math \[
            if DISPLAY_MATH_OPEN.is_match(line_text) && !DISPLAY_MATH_CLOSE.is_match(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_display_math = true;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Single-line display math \[...\]
            if DISPLAY_MATH_OPEN.is_match(line_text) && DISPLAY_MATH_CLOSE.is_match(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Regular prose line
            if !current_prose.is_empty() && !nospace_join {
                current_prose.push(' ');
            }
            current_prose.push_str(line_text.trim());
            let end = line.end;
            match &mut prose_span {
                None => prose_span = Some(ByteSpan::new(line.start, end)),
                Some(s) => s.end = end,
            }
            nospace_join = false;
        }

        flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
        if in_code_env.is_some() {
            let eof = ByteSpan::new(input.len(), input.len());
            regions.push(SpannedRegion::code(
                input,
                code_lang.take(),
                code_header,
                ByteSpan::new(code_body_start, input.len()),
                eof,
            ));
        }
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Region;

    #[test]
    fn section_command_title_is_structure_not_prose() {
        let input = "\\begin{document}\n\\section{A long title. With two sentences.}\nBody.\n\\end{document}\n";
        let regions = LatexParser.parse(input);
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
        };
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
    fn preamble_is_structure() {
        let input = r"\documentclass{article}
\usepackage{amsmath}
\begin{document}
Hello world.
\end{document}";
        let regions = LatexParser.parse(input);
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
        let regions = LatexParser.parse(input);
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
        let regions = LatexParser.parse(input);
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
        };
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
        };
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
        };
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("% TODO cite"),
            "trailing comment must be kept: {out}"
        );
        let regions = LatexParser.parse(input);
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
}
