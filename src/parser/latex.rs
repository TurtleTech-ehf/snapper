use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{FormatParser, Region, flush_prose};

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

static DISPLAY_MATH_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\\\[").unwrap());

static DISPLAY_MATH_CLOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\\]\s*$").unwrap());

pub struct LatexParser;

impl LatexParser {
    fn is_comment(line: &str) -> bool {
        line.trim_start().starts_with('%')
    }

    fn is_non_prose_env(name: &str) -> bool {
        NON_PROSE_ENVS.contains(&name)
    }
}

impl FormatParser for LatexParser {
    fn parse(&self, input: &str) -> Vec<Region> {
        let mut regions: Vec<Region> = Vec::new();
        let mut current_prose = String::new();
        let mut in_preamble = true;
        let mut in_non_prose_env: Option<String> = None;
        let mut in_display_math = false;
        let mut pragma_off = false;

        for line in input.lines() {
            // Check for snapper:off/on pragmas
            if let Some(on) = super::check_pragma(line) {
                flush_prose(&mut current_prose, &mut regions);
                pragma_off = !on;
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            if pragma_off {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Preamble: everything before \begin{document} is structure
            if in_preamble {
                if line.contains(r"\begin{document}") {
                    in_preamble = false;
                }
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Inside non-prose environment
            if let Some(ref env_name) = in_non_prose_env {
                flush_prose(&mut current_prose, &mut regions);
                if let Some(caps) = END_ENV_RE.captures(line) {
                    if caps.get(1).unwrap().as_str() == env_name {
                        in_non_prose_env = None;
                    }
                }
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Inside display math \[...\]
            if in_display_math {
                flush_prose(&mut current_prose, &mut regions);
                if DISPLAY_MATH_CLOSE.is_match(line) {
                    in_display_math = false;
                }
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Blank line
            if line.trim().is_empty() {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::BlankLines(format!("{line}\n")));
                continue;
            }

            // Comment
            if Self::is_comment(line) {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // \end{document}
            if line.contains(r"\end{document}") {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Begin non-prose environment
            if let Some(caps) = BEGIN_ENV_RE.captures(line) {
                let env_name = caps.get(1).unwrap().as_str().to_string();
                if Self::is_non_prose_env(&env_name) {
                    flush_prose(&mut current_prose, &mut regions);
                    // Check if \end is on the same line
                    if let Some(end_caps) = END_ENV_RE.captures(line) {
                        if end_caps.get(1).unwrap().as_str() == env_name {
                            regions.push(Region::Structure(format!("{line}\n")));
                            continue;
                        }
                    }
                    in_non_prose_env = Some(env_name);
                    regions.push(Region::Structure(format!("{line}\n")));
                    continue;
                }
            }

            // Display math \[
            if DISPLAY_MATH_OPEN.is_match(line) && !DISPLAY_MATH_CLOSE.is_match(line) {
                flush_prose(&mut current_prose, &mut regions);
                in_display_math = true;
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Single-line display math \[...\]
            if DISPLAY_MATH_OPEN.is_match(line) && DISPLAY_MATH_CLOSE.is_match(line) {
                flush_prose(&mut current_prose, &mut regions);
                regions.push(Region::Structure(format!("{line}\n")));
                continue;
            }

            // Regular prose line
            if !current_prose.is_empty() {
                current_prose.push(' ');
            }
            current_prose.push_str(line.trim());
        }

        flush_prose(&mut current_prose, &mut regions);
        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
