use regex::Regex;
use std::sync::LazyLock;

use crate::parser::{
    ByteSpan, FormatParser, Line, Region, RegionOrigin, SpannedRegion, flush_prose_spanned,
    iter_lines, join_prose_gap, push_prose_line,
};

static HEADLINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\*+\s+(?:TODO\s+|DONE\s+|NEXT\s+|WAIT\s+)?)(.*)$").unwrap());

/// Org-syntax 4.2.6 / orgize: `*` is a bullet only when indent > 0.
/// Column-0 `*` is a headline (`HEADLINE_RE`). `-`/`+`/`1.` stay unindented.
static LIST_ITEM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*(?:[-+]|\d+[.)]) |[ \t]+\* )(.*)$").unwrap());

/// Matches LaTeX \begin{env} lines embedded in org prose.
static LATEX_BEGIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\\begin\{([^}]+)\}").unwrap());

/// Matches LaTeX \end{env} lines.
static LATEX_END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\\end\{([^}]+)\}").unwrap());

/// Matches org inline export snippets: @@backend:value@@
static EXPORT_SNIPPET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@@[a-zA-Z]+:[^@]*@@").unwrap());

/// org-footnote.el `org-footnote-definition-re`: `^\[fn:([-_[:word:]]+)\]`.
/// Trailing spaces after `]` stay in the marker so reflow hangs the body.
static FOOTNOTE_DEFINITION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[fn:[-_\w]+\][ \t]*").unwrap());

/// Byte length of a column-0 org footnote-definition opener on `line`.
/// Includes spaces after `]`. `None` when the line is not a definition
/// (`[fn:: …]` / `[fn:name: …]` inline notes, indented lines, prose).
pub(crate) fn org_footnote_definition_marker_len(line: &str) -> Option<usize> {
    FOOTNOTE_DEFINITION_RE.find(line).map(|m| m.end())
}

/// org-element-keyword-re / org-element--affiliated-re (case-fold):
/// `^[ \t]*#\+CAPTION(?:\[(.*)\])?\+?:[ \t]*`.
///
/// `org-element-parsed-keywords` is CAPTION only, so only this opener
/// splits: the keyword plus optional `[short]` (dual form) and trailing
/// spaces after `:` are Structure; the value hangs as Prose.
/// `#+NAME:` / `#+ATTR_*` stay whole-line Structure.
pub(crate) fn org_caption_marker_len(line: &str) -> Option<usize> {
    let lead = line.len() - line.trim_start_matches([' ', '\t']).len();
    let rest = &line[lead..];
    const KEY: &str = "#+CAPTION";
    if rest.len() < KEY.len() || !rest[..KEY.len()].eq_ignore_ascii_case(KEY) {
        return None;
    }
    let mut i = KEY.len();
    let bytes = rest.as_bytes();
    if i < bytes.len() && bytes[i] == b'[' {
        let close = rest[i + 1..].find(']')?;
        i = i + 1 + close + 1;
    }
    if i < bytes.len() && bytes[i] == b'+' {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b':' {
        return None;
    }
    i += 1;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    Some(lead + i)
}

/// org-element / org-mode display math (`$$` or `\[`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayMathDelim {
    Bracket,
    Dollars,
}

pub struct OrgParser;

impl OrgParser {
    /// First token after `#+BEGIN_` / `#+END_`, uppercased (org-element NAME).
    fn block_directive_name(line: &str, prefix: &str) -> Option<String> {
        let trimmed = line.trim_start();
        let upper = trimmed.to_ascii_uppercase();
        if !upper.starts_with(prefix) {
            return None;
        }
        let name = trimmed[prefix.len()..].split_whitespace().next()?;
        if name.is_empty() {
            return None;
        }
        Some(name.to_ascii_uppercase())
    }

    fn block_begin_name(line: &str) -> Option<String> {
        Self::block_directive_name(line, "#+BEGIN_")
    }

    fn block_end_name(line: &str) -> Option<String> {
        Self::block_directive_name(line, "#+END_")
    }

    /// Check if a line starts a block (#+BEGIN_NAME).
    fn is_block_begin(line: &str) -> bool {
        Self::block_begin_name(line).is_some()
    }

    /// Check if a line starts a source code block (#+BEGIN_SRC LANG ARGS...).
    /// Returns the language token if present, or `Some(None)` for a bare
    /// `#+BEGIN_SRC`. Returns `None` for non-src blocks.
    fn is_src_begin(line: &str) -> Option<Option<String>> {
        let trimmed = line.trim_start();
        let upper = trimmed.to_ascii_uppercase();
        if !upper.starts_with("#+BEGIN_SRC") {
            return None;
        }
        // Slice the original (case-preserving) tail past the directive.
        let rest = trimmed["#+BEGIN_SRC".len()..].trim_start();
        if rest.is_empty() {
            return Some(None);
        }
        // Language is the first whitespace-delimited token.
        let lang = rest.split_whitespace().next().map(|s| s.to_string());
        Some(lang)
    }

    /// Check if a line ends a block (#+END_NAME).
    fn is_block_end(line: &str) -> bool {
        Self::block_end_name(line).is_some()
    }

    /// Check if a line ends a source code block (`#+END_SRC` only).
    fn is_src_end(line: &str) -> bool {
        Self::block_end_name(line).as_deref() == Some("SRC")
    }

    /// org-element greater blocks with paragraph contents: quote, verse,
    /// center, and special-block (`#+BEGIN_NOTE` and any other unknown NAME).
    /// Lesser blocks (example, export, comment) stay literal. SRC is Code.
    fn block_has_inner_prose(name: &str) -> bool {
        !matches!(name, "SRC" | "EXAMPLE" | "EXPORT" | "COMMENT")
    }

    /// True when any open block is literal, so nested quote inside example
    /// stays frozen with the example body.
    fn in_literal_block(stack: &[String]) -> bool {
        stack.iter().any(|n| !Self::block_has_inner_prose(n))
    }

    fn in_verse_block(stack: &[String]) -> bool {
        stack.last().map(String::as_str) == Some("VERSE")
    }

    /// org-element-dynamic-block-open-re: `^[ \t]*#\+BEGIN:[ \t]+\S`
    /// (`case-fold-search t`). A name is required; bare `#+BEGIN:` is a keyword.
    fn is_dynamic_block_begin(line: &str) -> bool {
        let t = line.trim_start_matches([' ', '\t']);
        let upper = t.to_ascii_uppercase();
        let Some(rest) = upper.strip_prefix("#+BEGIN:") else {
            return false;
        };
        !rest.trim_start_matches([' ', '\t']).is_empty()
    }

    /// org-element-dynamic-block-parser closer: `^[ \t]*#\+END:?[ \t]*$`
    /// (`case-fold-search t`). Does not match `#+END_NAME`.
    fn is_dynamic_block_end(line: &str) -> bool {
        let t = line.trim_start_matches([' ', '\t']);
        let upper = t.to_ascii_uppercase();
        let rest = if let Some(r) = upper.strip_prefix("#+END:") {
            r
        } else if let Some(r) = upper.strip_prefix("#+END") {
            r
        } else {
            return false;
        };
        rest.trim_matches([' ', '\t']).is_empty()
    }

    /// org-element drawer: `:NAME:` with NAME = `[A-Za-z_-]+`. Not `:END:`.
    fn is_drawer_begin(line: &str) -> bool {
        let trimmed = line.trim();
        let Some(name) = trimmed.strip_prefix(':').and_then(|s| s.strip_suffix(':')) else {
            return false;
        };
        !name.is_empty()
            && !name.eq_ignore_ascii_case("END")
            && name
                .bytes()
                .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-'))
    }

    /// Check if a line ends a drawer
    fn is_drawer_end(line: &str) -> bool {
        line.trim().eq_ignore_ascii_case(":END:")
    }

    /// org-element fixed-width: `: ` payload or a lone `:`.
    fn is_fixed_width(line: &str) -> bool {
        let t = line.trim_start_matches([' ', '\t']);
        t == ":" || t.starts_with(": ")
    }

    /// org-element horizontal rule: five or more dashes.
    fn is_horizontal_rule(line: &str) -> bool {
        let t = line.trim();
        t.len() >= 5 && t.bytes().all(|b| b == b'-')
    }

    /// org-element planning (`DEADLINE:`/`SCHEDULED:`/`CLOSED:`) or clock (`CLOCK:`).
    /// Leading space/tab is allowed; keywords are the default org strings.
    fn is_planning_or_clock(line: &str) -> bool {
        let t = line.trim_start_matches([' ', '\t']);
        t.starts_with("DEADLINE:")
            || t.starts_with("SCHEDULED:")
            || t.starts_with("CLOSED:")
            || t.starts_with("CLOCK:")
    }

    /// Check if a line is a keyword/directive (#+KEYWORD:)
    fn is_keyword(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("#+")
            && !Self::is_block_begin(line)
            && !Self::is_block_end(line)
            && !Self::is_dynamic_block_begin(line)
    }

    /// org.el `org-comment-regexp`: `^[ \t]*#(?: |$)`.
    /// `#foo` is prose; `#` and `# comment` are comments. `#+` is a keyword.
    fn is_comment(line: &str) -> bool {
        let trimmed = line.trim_start_matches([' ', '\t']);
        trimmed == "#" || trimmed.starts_with("# ")
    }

    /// Check if a line is a table row
    fn is_table_row(line: &str) -> bool {
        line.trim_start().starts_with('|')
    }

    /// A line that is only an org bracket link (`[[file:plot.png]]`).
    /// Figure payloads stay Structure so following prose does not join.
    fn is_standalone_org_link(line: &str) -> bool {
        let t = line.trim();
        let Some(inner) = t.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) else {
            return false;
        };
        if inner.is_empty() {
            return false;
        }
        match inner.split_once("][") {
            None => !inner.contains(['[', ']']),
            Some((link, desc)) => {
                !link.is_empty()
                    && !desc.is_empty()
                    && !link.contains(['[', ']'])
                    && !desc.contains(['[', ']'])
            }
        }
    }

    /// Check if a line starts a LaTeX environment (\begin{...})
    fn is_latex_begin(line: &str) -> Option<String> {
        LATEX_BEGIN_RE
            .captures(line)
            .map(|caps| caps.get(1).unwrap().as_str().to_string())
    }

    /// Check if a line ends a LaTeX environment (\end{...})
    fn is_latex_end(line: &str, env: &str) -> bool {
        LATEX_END_RE
            .captures(line)
            .is_some_and(|caps| caps.get(1).unwrap().as_str() == env)
    }

    /// Same-line `\end{NAME}` after a leftover-start `\begin{NAME}`.
    /// Name-matched; a mismatched closer does not close the env.
    fn line_closes_latex_env(line: &str, env: &str) -> bool {
        let Some(caps) = LATEX_BEGIN_RE.captures(line) else {
            return false;
        };
        let after_begin = caps.get(0).unwrap().end();
        let needle = format!("\\end{{{env}}}");
        line[after_begin..].contains(&needle)
    }

    /// latexindent `(?<!\\)\\\[` / `(?<!\\)\\\]`. `\\[` is not display math.
    fn find_unescaped_display_bracket(s: &str, from: usize, closer: u8) -> Option<usize> {
        let bytes = s.as_bytes();
        let mut i = from;
        while i < bytes.len() {
            if bytes[i] == b'\\'
                && i + 1 < bytes.len()
                && bytes[i + 1] == closer
                && (i == 0 || bytes[i - 1] != b'\\')
            {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// org-syntax 5.2 latex-fragment: first same-line `\[CONTENTS\]`.
    /// Returns `(open, after_close)` byte offsets in `s`.
    fn same_line_bracket_fragment(s: &str) -> Option<(usize, usize)> {
        let open = Self::find_unescaped_display_bracket(s, 0, b'[')?;
        let close = Self::find_unescaped_display_bracket(s, open + 2, b']')?;
        Some((open, close + 2))
    }

    /// org-element latex-fragment display math: `\[` / `\]` or `$$` / `$$`.
    fn display_math_open(line: &str) -> Option<DisplayMathDelim> {
        let t = line.trim();
        if t == r"\[" {
            Some(DisplayMathDelim::Bracket)
        } else if t.starts_with("$$") {
            Some(DisplayMathDelim::Dollars)
        } else {
            None
        }
    }

    /// A line that is only `$$` is an opener, not a one-line `$$...$$` block.
    fn display_math_is_single_line(line: &str, delim: DisplayMathDelim) -> bool {
        match delim {
            DisplayMathDelim::Bracket => false,
            DisplayMathDelim::Dollars => {
                let t = line.trim();
                t != "$$" && t.ends_with("$$")
            }
        }
    }

    fn is_display_math_close(line: &str, delim: DisplayMathDelim) -> bool {
        match delim {
            DisplayMathDelim::Bracket => line.trim() == r"\]",
            DisplayMathDelim::Dollars => line.trim_end().ends_with("$$"),
        }
    }

    /// Check if a line is entirely an inline export snippet (@@backend:...@@)
    fn is_export_snippet_line(line: &str) -> bool {
        let trimmed = line.trim();
        EXPORT_SNIPPET_RE.is_match(trimmed) && trimmed.starts_with("@@")
    }

    /// Split same-line `\[CONTENTS\]` into Structure islands (Kang / org-syntax 5.2).
    /// Glue space before `\[` stays on the island so reflow does not break
    /// `The root is \[ x = a.b \]`. Leftover-start indent stays on the island.
    /// Returns true when at least one same-line pair was emitted.
    fn emit_same_line_bracket_fragments(
        input: &str,
        line: &Line<'_>,
        current_prose: &mut String,
        prose_span: &mut Option<ByteSpan>,
        regions: &mut Vec<SpannedRegion>,
    ) -> bool {
        let mut rel = 0;
        let mut found = false;
        while let Some((open, after_close)) = Self::same_line_bracket_fragment(&line.text[rel..]) {
            found = true;
            let open_abs = rel + open;
            let end_abs = rel + after_close;
            let prefix = &line.text[rel..open_abs];
            let lead_glue = if prefix.trim().is_empty() {
                0
            } else {
                prefix.len() - prefix.trim_end_matches([' ', '\t']).len()
            };
            // Prose span must stop before the glue space; splice copies
            // Structure from source and overlapping ranges drop the space.
            // Whitespace-only leftover-start indent is not prose: pushing
            // it writes prose_span then flush skips take(), so the next
            // line extends over the math and splice drops it.
            if !prefix.trim().is_empty() && prefix.len() > lead_glue {
                let lead = Line {
                    start: line.start + rel,
                    end: line.start + open_abs - lead_glue,
                    text: &line.text[rel..open_abs - lead_glue],
                };
                push_prose_line(current_prose, prose_span, &lead, true, false);
            }
            flush_prose_spanned(current_prose, prose_span, regions);
            let island_start = if prefix.trim().is_empty() {
                line.start + rel
            } else {
                line.start + open_abs - lead_glue
            };
            let rest_after = &line.text[end_abs..];
            let trail_glue = if rest_after.trim().is_empty() {
                0
            } else {
                rest_after.len() - rest_after.trim_start_matches([' ', '\t']).len()
            };
            let island_end = if rest_after.trim().is_empty() {
                line.end
            } else {
                line.start + end_abs + trail_glue
            };
            regions.push(SpannedRegion::structure(
                input,
                ByteSpan::new(island_start, island_end),
            ));
            rel = end_abs + trail_glue;
            if rest_after.trim().is_empty() {
                return true;
            }
        }
        if !found {
            return false;
        }
        if rel < line.text.len() && !line.text[rel..].trim().is_empty() {
            let rest = Line {
                start: line.start + rel,
                end: line.end,
                text: &line.text[rel..],
            };
            push_prose_line(current_prose, prose_span, &rest, true, true);
        }
        true
    }
}

impl FormatParser for OrgParser {
    fn parse_full(&self, input: &str) -> Vec<SpannedRegion> {
        let mut regions: Vec<SpannedRegion> = Vec::new();
        let mut current_prose = String::new();
        let mut prose_span: Option<ByteSpan> = None;
        // Open greater-element names. `#+END_NAME` pops only a matching top
        // (org-element / orgize); a mismatched closer stays structure.
        let mut block_stack: Vec<String> = Vec::new();
        let mut in_src_block = false;
        let mut src_lang: Option<String> = None;
        let mut src_header = ByteSpan::default();
        let mut src_body_start = 0usize;
        let mut in_drawer = false;
        let mut in_dynamic_block = false;
        let mut in_latex_env: Option<String> = None;
        let mut in_display_math: Option<DisplayMathDelim> = None;
        let mut pragma_off = false;
        // Track list item context: indent level of the marker text.
        // Continuation lines indented at or beyond this level belong to the item.
        let mut list_item_indent: Option<usize> = None;

        for line in iter_lines(input) {
            let line_text = line.text;
            // Check for snapper:off/on pragmas. Inside a source block we
            // defer pragma handling to the code-block reflow so the
            // language's own comment marker controls the freeze.
            if !in_src_block {
                if let Some(on) = super::check_pragma(line_text) {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    pragma_off = !on;
                    regions.push(SpannedRegion::structure(input, line.span()));
                    continue;
                }

                // Inside pragma-off region: pass through unchanged
                if pragma_off {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(input, line.span()));
                    continue;
                }
            }

            // Inside a source block -- buffer body until #+END_SRC
            if in_src_block {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_src_end(line_text) {
                    in_src_block = false;
                    regions.push(SpannedRegion::code(
                        input,
                        src_lang.take(),
                        src_header,
                        ByteSpan::new(src_body_start, line.start),
                        line.span(),
                    ));
                }
                continue;
            }

            // Inside example/export/comment (literal lesser blocks): the
            // body is opaque structure. Nested BEGIN/END still match by
            // NAME (zyjn); do not parse inner quote/special-block prose here.
            if Self::in_literal_block(&block_stack) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if let Some(end_name) = Self::block_end_name(line_text) {
                    if block_stack.last() == Some(&end_name) {
                        block_stack.pop();
                    }
                } else if let Some(begin_name) = Self::block_begin_name(line_text) {
                    block_stack.push(begin_name);
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Transparent quote/verse/center/special-block: #+END_NAME pops only a match.
            // A mismatched closer stays structure so #+END_EXAMPLE cannot
            // drop a surrounding quote fence.
            if let Some(end_name) = Self::block_end_name(line_text) {
                if block_stack.last() == Some(&end_name) {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    block_stack.pop();
                    regions.push(SpannedRegion::structure(input, line.span()));
                    continue;
                }
                if !block_stack.is_empty() {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                    regions.push(SpannedRegion::structure(input, line.span()));
                    continue;
                }
            }

            // Inside a drawer -- everything is structure
            if in_drawer {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_drawer_end(line_text) {
                    in_drawer = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Inside a LaTeX environment -- everything is structure
            if let Some(ref env) = in_latex_env {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let done = Self::is_latex_end(line_text, env);
                regions.push(SpannedRegion::structure(input, line.span()));
                if done {
                    in_latex_env = None;
                }
                continue;
            }

            // Inside display math \[...\] / $$...$$ -- everything is structure
            if let Some(delim) = in_display_math {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_display_math_close(line_text, delim) {
                    in_display_math = None;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Inside a dynamic block (`#+BEGIN: NAME` … `#+END:`) -- structure.
            if in_dynamic_block {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if Self::is_dynamic_block_end(line_text) {
                    in_dynamic_block = false;
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Source block begin (#+BEGIN_SRC LANG ...)
            if let Some(lang) = Self::is_src_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_src_block = true;
                src_lang = lang;
                src_header = line.span();
                src_body_start = line.end;
                continue;
            }

            // #+BEGIN_NAME: fence is structure. quote/verse/center and
            // special-blocks parse inner content; example/export/comment
            // stay opaque via in_literal_block.
            if let Some(name) = Self::block_begin_name(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                block_stack.push(name);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // #+BEGIN: NAME -- org-element dynamic block. Body stays Structure
            // until #+END: / #+END. `#+BEGIN_` greater blocks are handled above.
            if Self::is_dynamic_block_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_dynamic_block = true;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Drawer begin (`:NAME:` only; `:See also:` is not a name)
            if Self::is_drawer_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                in_drawer = true;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Fixed-width (`: text`) is Structure, not a drawer.
            if Self::is_fixed_width(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Horizontal rule (`-----`) is Structure and a paragraph boundary.
            if Self::is_horizontal_rule(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // LaTeX environment begin (\begin{equation} etc.)
            if let Some(env) = Self::is_latex_begin(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                // Same-line `\begin{NAME}...\end{NAME}` must close here so
                // following prose is not swallowed as env body.
                if !Self::line_closes_latex_env(line_text, &env) {
                    in_latex_env = Some(env);
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Display math open (\[ or $$)
            if let Some(delim) = Self::display_math_open(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                if !Self::display_math_is_single_line(line_text, delim) {
                    in_display_math = Some(delim);
                }
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Export snippet line (@@latex:...@@)
            if Self::is_export_snippet_line(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Blank line
            if line_text.trim().is_empty() {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                list_item_indent = None;
                regions.push(SpannedRegion::blank(input, line.span()));
                continue;
            }

            // Affiliated CAPTION: org-element-parsed-keywords is CAPTION
            // only. Opener (optional [short] / append `+`) is Structure;
            // value hangs. `#+NAME:` / `#+ATTR_*` stay whole-line.
            if let Some(marker_len) = org_caption_marker_len(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                list_item_indent = Some(marker_len);
                let marker_span = ByteSpan::new(line.start, line.start + marker_len);
                regions.push(SpannedRegion::structure(input, marker_span));
                if line_text.len() > marker_len {
                    let text = &line_text[marker_len..];
                    regions.push(SpannedRegion::prose(
                        text.to_string(),
                        ByteSpan::new(line.start + marker_len, line.start + line_text.len()),
                    ));
                }
                let term = line.terminator_span();
                if !term.is_empty() {
                    regions.push(SpannedRegion::structure(input, term));
                }
                continue;
            }

            // Keyword/directive (`#+NAME:`, `#+ATTR_*`, `#+TITLE:`, …)
            if Self::is_keyword(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Comment
            if Self::is_comment(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Table row
            if Self::is_table_row(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Bare file/http links on their own line -- treat as structure
            if line_text.trim_start().starts_with("file:")
                || line_text.trim_start().starts_with("http://")
                || line_text.trim_start().starts_with("https://")
                || Self::is_standalone_org_link(line_text)
            {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                list_item_indent = None;
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Headline: keep the entire line as Structure.
            // Splitting Structure(stars)+Prose(title) reflowed multi-sentence
            // titles and left continuation lines without stars (orphan body).
            // Org headlines are single-line; do not reflow them.
            if HEADLINE_RE.is_match(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Planning / clock stay Structure so they do not join the next paragraph.
            if Self::is_planning_or_clock(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                regions.push(SpannedRegion::structure(input, line.span()));
                continue;
            }

            // Footnote definition: org-footnote-definition-re `^\[fn:LABEL\]`.
            // Marker is Structure; same-line body is hung Prose (GitHub #180).
            if let Some(marker_len) = org_footnote_definition_marker_len(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                list_item_indent = Some(marker_len);
                let marker_span = ByteSpan::new(line.start, line.start + marker_len);
                regions.push(SpannedRegion::structure(input, marker_span));
                if line_text.len() > marker_len {
                    let text = &line_text[marker_len..];
                    regions.push(SpannedRegion::prose(
                        text.to_string(),
                        ByteSpan::new(line.start + marker_len, line.start + line_text.len()),
                    ));
                }
                let term = line.terminator_span();
                if !term.is_empty() {
                    regions.push(SpannedRegion::structure(input, term));
                }
                continue;
            }

            // List item: marker is structure, rest is prose
            if let Some(caps) = LIST_ITEM_RE.captures(line_text) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                let marker = caps.get(1).unwrap().as_str();
                let text = caps.get(2).unwrap().as_str();
                // Track indent for continuation detection: text starts at marker length
                list_item_indent = Some(marker.len());
                let marker_span = ByteSpan::new(line.start, line.start + marker.len());
                regions.push(SpannedRegion::structure(input, marker_span));
                if !text.is_empty() {
                    regions.push(SpannedRegion::prose(
                        text.to_string(),
                        ByteSpan::new(line.start + marker.len(), line.start + line_text.len()),
                    ));
                }
                let term = line.terminator_span();
                if !term.is_empty() {
                    regions.push(SpannedRegion::structure(input, term));
                }
                continue;
            }

            // List item continuation: indented line following a list item
            if let Some(indent) = list_item_indent {
                let leading = line_text.len() - line_text.trim_start().len();
                if leading >= indent && !line_text.trim().is_empty() {
                    // Append to the previous Prose region of the list item.
                    // The last three regions are Structure(marker), Prose(text), Structure(\n)
                    // We want to extend the Prose region.
                    let is_term = matches!(
                        regions.last(),
                        Some(SpannedRegion {
                            region: Region::Structure(s),
                            ..
                        }) if s == "\n"
                    );
                    if is_term {
                        regions.pop();
                        if let Some(prev) = regions.last_mut() {
                            if let Region::Prose(prose) = &mut prev.region {
                                join_prose_gap(prose);
                                prose.push_str(line_text.trim());
                            }
                            if let Some(RegionOrigin::Whole(span)) = &mut prev.origin {
                                span.end = line.start + line_text.len();
                            }
                        }
                        let term = line.terminator_span();
                        if !term.is_empty() {
                            regions.push(SpannedRegion::structure(input, term));
                        }
                        continue;
                    }
                }
                // Not a continuation: leave list context
                list_item_indent = None;
            }

            // Same-line \[CONTENTS\] is Structure (org-syntax 5.2 / Kang).
            // Surrounding words stay Prose so "today. Next claim." still reflows.
            if Self::emit_same_line_bracket_fragments(
                input,
                &line,
                &mut current_prose,
                &mut prose_span,
                &mut regions,
            ) {
                if Self::in_verse_block(&block_stack) {
                    flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
                }
                continue;
            }

            // Regular prose line -- accumulate. Verse keeps physical lines.
            push_prose_line(&mut current_prose, &mut prose_span, &line, true, true);
            if Self::in_verse_block(&block_stack) {
                flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
            }
        }

        // Flush remaining
        flush_prose_spanned(&mut current_prose, &mut prose_span, &mut regions);
        // Unclosed source block at EOF: still emit as Code with empty footer.
        if in_src_block {
            let eof = ByteSpan::new(input.len(), input.len());
            regions.push(SpannedRegion::code(
                input,
                src_lang.take(),
                src_header,
                ByteSpan::new(src_body_start, input.len()),
                eof,
            ));
        }

        regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_prose() {
        let input = "Hello world. This is a test.\nAnother line here.";
        let regions = OrgParser.parse(input);
        assert_eq!(
            regions,
            vec![Region::Prose(
                "Hello world. This is a test.\nAnother line here.".to_string()
            )]
        );
    }

    #[test]
    fn preserves_blocks() {
        let input = "Some prose.\n#+BEGIN_SRC python\nprint('hello')\n#+END_SRC\nMore prose.";
        let regions = OrgParser.parse(input);
        assert_eq!(regions.len(), 3);
        assert!(matches!(&regions[0], Region::Prose(_)));
        match &regions[1] {
            Region::Code {
                lang,
                header,
                body,
                footer,
            } => {
                assert_eq!(lang.as_deref(), Some("python"));
                assert_eq!(header, "#+BEGIN_SRC python\n");
                assert_eq!(body, "print('hello')\n");
                assert_eq!(footer, "#+END_SRC\n");
            }
            other => panic!("expected Region::Code, got {other:?}"),
        }
        assert!(matches!(&regions[2], Region::Prose(_)));
    }

    #[test]
    fn preserves_keywords() {
        let input = "#+TITLE: My Document\n#+AUTHOR: Someone\n\nSome text here.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Structure(_)));
        assert!(matches!(&regions[1], Region::Structure(_)));
    }

    #[test]
    fn headline_is_structure_not_prose() {
        let input = "* TODO This is a headline";
        let regions = OrgParser.parse(input);
        assert_eq!(regions.len(), 1);
        assert_eq!(
            regions[0],
            Region::Structure("* TODO This is a headline".to_string())
        );
    }

    #[test]
    fn multi_sentence_headline_stays_one_line() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "** Multi sentence. Second sentence in title\nbody prose. Second body.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.lines()
                .any(|l| l == "** Multi sentence. Second sentence in title"),
            "headline must stay one line, got:\n{out}"
        );
        assert!(
            !out.contains("** Multi sentence.\nSecond"),
            "must not orphan second title sentence without stars:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn headline_trailing_angle_bracket_round_trips() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "* TODO R4 :: snapshot field is Box[T], not Vec[T]\nbody\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Vec[T]"),
            "trailing `>` must survive formatting, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn verbatim_inner_equals_does_not_orphan_closer() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        // The period after `note.` is inside the first span. Closing on the
        // inner `=` would emit a line that starts with `=` and leave the
        // document's markup unterminated.
        let input = "so =x = 1 -- note.= reflows while =s = \"x\"= does not.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, input,
            "verbatim spans with inner `=` must stay one sentence, got:\n{out}"
        );
        assert!(
            !out.lines().any(|l| l.starts_with('=')),
            "must not orphan a closer onto its own line, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn bold_emphasis_with_period_does_not_become_headline() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "End of first. *Bold spans period. Continues* after.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        // Emphasis with an internal period must stay on one line; splitting
        // would leave a line starting with `*Bold` and a dangling closer.
        let bold_lines: Vec<_> = out
            .lines()
            .filter(|l| l.contains("*Bold") || l.contains("Continues*"))
            .collect();
        assert_eq!(
            bold_lines.len(),
            1,
            "bold emphasis must not split across lines, got:\n{out}"
        );
        assert!(bold_lines[0].contains("*Bold spans period. Continues*"));
        // Org headlines are stars + space; ensure we never introduce one.
        for line in out.lines() {
            let stars = line.chars().take_while(|c| *c == '*').count();
            if stars > 0 {
                let rest = &line[stars..];
                assert!(
                    !rest.starts_with(' ') || rest.trim().is_empty() || line.starts_with("* "),
                    "unexpected star-line: {line}"
                );
            }
        }
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn table_preserved() {
        let input = "| Name | Age |\n|------+-----|\n| Alice | 30 |";
        let regions = OrgParser.parse(input);
        assert!(regions.iter().all(|r| matches!(r, Region::Structure(_))));
    }

    #[test]
    fn list_item_split() {
        let input = "- First item text\n- Second item text";
        let regions = OrgParser.parse(input);
        // Each list item: Structure(marker) + Prose(text) + Structure(\n)
        // The last item has no trailing newline, so no final Structure(\n).
        assert_eq!(regions.len(), 5);
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(regions[1], Region::Prose("First item text".to_string()));
    }

    #[test]
    fn list_item_continuation() {
        let input = "- First sentence of item.\n  Continuation of the same item.\n- Second item";
        let regions = OrgParser.parse(input);
        // First item: Structure("- ") + Prose + Structure("\n")
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("First sentence of item.\nContinuation of the same item.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        // Second item: Structure("- ") + Prose("Second item") + Structure("\n")
        assert_eq!(regions[3], Region::Structure("- ".to_string()));
        assert_eq!(regions[4], Region::Prose("Second item".to_string()));
    }

    #[test]
    fn drawer_preserved() {
        let input = ":PROPERTIES:\n:ID: abc123\n:END:\nSome text.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Structure(_))); // :PROPERTIES:
        assert!(matches!(&regions[1], Region::Structure(_))); // :ID:
        assert!(matches!(&regions[2], Region::Structure(_))); // :END:
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Some text."))),
            "text after :END: must stay Prose, got: {regions:?}"
        );
    }

    #[test]
    fn latex_environment_preserved() {
        let input = "Some text.\n\\begin{equation}\nx = 5\n\\end{equation}\nMore text.";
        let regions = OrgParser.parse(input);
        // Prose, Structure(\begin), Structure(x=5), Structure(\end), Prose
        assert!(matches!(&regions[0], Region::Prose(_)));
        assert!(matches!(&regions[1], Region::Structure(s) if s.contains("\\begin{equation}")));
        assert!(matches!(&regions[2], Region::Structure(s) if s.contains("x = 5")));
        assert!(matches!(&regions[3], Region::Structure(s) if s.contains("\\end{equation}")));
        assert!(matches!(&regions[4], Region::Prose(_)));
    }

    fn org_cfg() -> crate::FormatConfig {
        crate::FormatConfig {
            format: crate::format::Format::Org,
            ..Default::default()
        }
        .without_safety_backstops()
    }

    #[test]
    fn display_math_preserved() {
        let input = "Some text.\n\\[\nx = 5\n\\]\nMore text.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Prose(_)));
        assert!(matches!(&regions[1], Region::Structure(s) if s.contains("\\[")));
        assert!(matches!(&regions[2], Region::Structure(s) if s.contains("x = 5")));
        assert!(matches!(&regions[3], Region::Structure(s) if s.contains("\\]")));
        assert!(matches!(&regions[4], Region::Prose(_)));
    }

    #[test]
    fn dollar_dollar_display_math_is_structure_not_prose() {
        let input = "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$\n";
        let regions = OrgParser.parse(input);
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
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
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
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn dollar_dollar_two_sentences_do_not_split() {
        use crate::format_text;

        let input =
            "$$\nFirst sentence. Second sentence that would split if this were prose.\n$$\n";
        let out = format_text(input, &org_cfg()).unwrap();
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
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn dollar_dollar_single_line_display_is_structure() {
        use crate::format_text;

        let input = "Before the math. More before.\n$$E = mc^2$$\nAfter the math. More after.\n";
        let regions = OrgParser.parse(input);
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
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("$$E = mc^2$$"),
            "single-line $$ must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("Before the math.\nMore before."),
            "surrounding prose must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn bracket_display_math_still_structure() {
        use crate::format_text;

        let input = "\\[\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n\\]\n";
        let regions = OrgParser.parse(input);
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
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(
                "\\[\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n\\]"
            ),
            "\\[ display math must stay a structure block, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn inline_single_dollar_math_is_still_prose() {
        use crate::format_text;

        let input = "See $x = 1$ here. Next sentence.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("$x = 1$"))),
            "inline $...$ must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("See $x = 1$ here.\nNext sentence."),
            "inline $...$ must not open display math, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn export_snippet_preserved() {
        let input = "Text before.\n@@latex:\\newpage@@\nText after.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Prose(_)));
        assert!(matches!(&regions[1], Region::Structure(s) if s.contains("@@latex:")));
        assert!(matches!(&regions[2], Region::Prose(_)));
    }

    #[test]
    fn nested_latex_envs() {
        let input = "Prose.\n\\begin{align}\na &= b \\\\\nc &= d\n\\end{align}\nMore prose.";
        let regions = OrgParser.parse(input);
        assert!(matches!(&regions[0], Region::Prose(_)));
        // All lines inside align are structure
        let struct_count = regions
            .iter()
            .filter(|r| matches!(r, Region::Structure(_)))
            .count();
        assert!(struct_count >= 4); // \begin, two content lines, \end
    }

    #[test]
    fn list_multi_sentence_hangs_and_rejoins() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "- One. Two.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, "- One.\n  Two.\n");
        let second = format_text(&out, &cfg).unwrap();
        assert_eq!(second, out, "format_text twice must equal once");

        let regions = OrgParser.parse(&out);
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(regions[1], Region::Prose("One.\nTwo.".to_string()));
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions.len(), 3);
    }

    #[test]
    fn nested_list_stays_two_items_after_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "1. Parent one. Parent two.\n   - Child one. Child two.\n";
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            "1. Parent one.\n   Parent two.\n   - Child one.\n     Child two.\n"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let regions = OrgParser.parse(&out);
        assert_eq!(regions[0], Region::Structure("1. ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Parent one.\nParent two.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions[3], Region::Structure("   - ".to_string()));
        assert_eq!(
            regions[4],
            Region::Prose("Child one.\nChild two.".to_string())
        );
        assert_eq!(regions[5], Region::Structure("\n".to_string()));
        assert_eq!(regions.len(), 6);
    }

    /// Ticket fixture (Format::Org / GitHub #110): indented `* ` is a child
    /// list item (Structure marker + Prose hang), not a parent continuation.
    fn wci8_indented_star_list_fixture() -> &'static str {
        "- Parent one. Parent two.\n  * Child one. Child two.\n"
    }

    #[test]
    fn indented_star_list_is_not_parent_continuation() {
        let input = wci8_indented_star_list_fixture();
        let regions = OrgParser.parse(input);
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Parent one. Parent two.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions[3], Region::Structure("  * ".to_string()));
        assert_eq!(
            regions[4],
            Region::Prose("Child one. Child two.".to_string())
        );
        assert_eq!(regions[5], Region::Structure("\n".to_string()));
        assert_eq!(regions.len(), 6);
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("* Child")
            )),
            "indented * child must not join the parent Prose, got: {regions:?}"
        );
    }

    #[test]
    fn indented_star_list_hangs_and_rejoins() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = wci8_indented_star_list_fixture();
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        };
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out,
            "- Parent one.\n  Parent two.\n  * Child one.\n    Child two.\n"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let regions = OrgParser.parse(&out);
        assert_eq!(regions[0], Region::Structure("- ".to_string()));
        assert_eq!(
            regions[1],
            Region::Prose("Parent one.\nParent two.".to_string())
        );
        assert_eq!(regions[2], Region::Structure("\n".to_string()));
        assert_eq!(regions[3], Region::Structure("  * ".to_string()));
        assert_eq!(
            regions[4],
            Region::Prose("Child one.\nChild two.".to_string())
        );
        assert_eq!(regions[5], Region::Structure("\n".to_string()));
        assert_eq!(regions.len(), 6);
    }

    #[test]
    fn column_zero_star_stays_headline_not_list() {
        let input = "* Child one. Child two.\n";
        let regions = OrgParser.parse(input);
        assert_eq!(
            regions,
            vec![Region::Structure("* Child one. Child two.\n".to_string())]
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Child"))),
            "column-0 * must stay a headline, got: {regions:?}"
        );
    }

    #[test]
    fn column_zero_star_headline_keeps_indented_star_child() {
        use crate::format_text;

        let input = "* Parent one. Parent two.\n  * Child one. Child two.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.trim_end() == "* Parent one. Parent two."
            )),
            "column-0 * must stay a headline, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "  * ")),
            "indented * under a headline is still a list, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.lines().any(|l| l == "* Parent one. Parent two."),
            "headline must not reflow as a list, got:\n{out}"
        );
        assert!(
            out.contains("  * Child one.\n    Child two."),
            "indented * child must hang, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Quote containing example: `#+END_EXAMPLE` must not close the quote.
    fn quote_with_nested_example() -> &'static str {
        concat!(
            "#+BEGIN_QUOTE\n",
            "Quoted one. Quoted two.\n",
            "#+BEGIN_EXAMPLE\n",
            "foo. bar.\n",
            "#+END_EXAMPLE\n",
            "Still quoted. More quoted.\n",
            "#+END_QUOTE\n",
            "After. Next.\n",
        )
    }

    #[test]
    fn nested_example_does_not_close_quote_by_any_end() {
        let input = quote_with_nested_example();
        let regions = OrgParser.parse(input);
        let structure: String = regions
            .iter()
            .filter_map(|r| match r {
                Region::Structure(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            structure.contains("#+BEGIN_QUOTE"),
            "quote opener must stay Structure, got: {regions:?}"
        );
        assert!(
            structure.contains("#+END_QUOTE"),
            "matching #+END_QUOTE must stay Structure, not prose: {regions:?}"
        );
        assert!(
            !structure.contains("Still quoted. More quoted."),
            "post-example quote body must be Prose, not Structure: {regions:?}"
        );
        assert!(
            structure.contains("foo. bar."),
            "EXAMPLE body must stay Structure: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("#+END_QUOTE")
            )),
            "quote closer must not become Prose: {regions:?}"
        );
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            prose
                .iter()
                .any(|p| p.contains("Quoted one.") && p.contains("Quoted two.")),
            "quote body before EXAMPLE must be Prose, got: {regions:?}"
        );
        assert!(
            prose
                .iter()
                .any(|p| p.contains("Still quoted") && p.contains("More quoted")),
            "quote body after EXAMPLE must stay Prose inside the quote: {regions:?}"
        );
        assert!(
            prose
                .iter()
                .any(|p| p.contains("After.") && p.contains("Next.")),
            "prose after the quote must remain Prose, got: {regions:?}"
        );
    }

    #[test]
    fn nested_example_in_quote_closes_by_name_only() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = quote_with_nested_example();
        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            out.contains("Quoted one.\nQuoted two."),
            "quoted sentences must reflow inside the quote fence, got:\n{out}"
        );
        assert!(
            out.contains("foo. bar."),
            "EXAMPLE body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("foo.\nbar."),
            "EXAMPLE must stay literal, got:\n{out}"
        );
        assert!(
            out.contains("Still quoted.\nMore quoted."),
            "text after nested EXAMPLE must reflow inside the quote, got:\n{out}"
        );
        assert!(
            !out.contains("Still quoted. More quoted."),
            "inner quote body must become Prose, got:\n{out}"
        );
        assert!(
            out.contains("#+END_QUOTE\nAfter.\nNext.\n")
                || out.ends_with("#+END_QUOTE\nAfter.\nNext."),
            "real closer stays a fence; following prose reflows, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn example_export_comment_do_not_reflow() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        }
        .without_safety_backstops();
        for (name, body) in [
            ("EXAMPLE", "foo. bar."),
            ("EXPORT", "<p>Hello. World.</p>"),
            ("COMMENT", "Secret one. Secret two."),
        ] {
            let input = format!("#+BEGIN_{name}\n{body}\n#+END_{name}\nAfter. Next.\n");
            let out = format_text(&input, &cfg).unwrap();
            assert!(
                out.contains(body),
                "{name} body must not reflow, got:\n{out}"
            );
            assert!(
                out.contains(&format!("#+END_{name}\nAfter.\nNext.")),
                "{name} closer is name-matched; following prose reflows, got:\n{out}"
            );
            assert_eq!(format_text(&out, &cfg).unwrap(), out);
        }
    }

    /// Ticket fixture (Format::Org / GitHub #88): quote inner paragraphs reflow.
    fn quote_inner_prose_fixture() -> &'static str {
        concat!(
            "#+BEGIN_QUOTE\n",
            "Quoted one. Quoted two.\n",
            "#+END_QUOTE\n",
        )
    }

    fn quote_with_nested_src_fixture() -> &'static str {
        concat!(
            "#+BEGIN_QUOTE\n",
            "Before.\n",
            "\n",
            "#+BEGIN_SRC python\n",
            "print(\"a. b\")\n",
            "#+END_SRC\n",
            "\n",
            "After quote. More.\n",
            "#+END_QUOTE\n",
        )
    }

    #[test]
    fn quote_block_inner_is_prose_not_structure() {
        let input = quote_inner_prose_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_QUOTE"))),
            "quote opener must stay Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("#+END_QUOTE"))),
            "quote closer must stay Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Quoted one.") && p.contains("Quoted two.")
            )),
            "quote body must be Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Quoted one.")
            )),
            "quote body must not freeze as Structure, got: {regions:?}"
        );
    }

    #[test]
    fn quote_block_inner_prose_reflows() {
        use crate::format_text;

        let input = quote_inner_prose_fixture();
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("#+BEGIN_QUOTE\nQuoted one.\nQuoted two.\n#+END_QUOTE"),
            "quoted sentences must reflow inside the fence, got:\n{out}"
        );
        assert!(
            !out.contains("Quoted one. Quoted two."),
            "quoted sentences must not stay fused, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn quote_block_nested_src_closes_only_src() {
        use crate::format_text;

        let input = quote_with_nested_src_fixture();
        let regions = OrgParser.parse(input);
        match regions.iter().find(|r| matches!(r, Region::Code { .. })) {
            Some(Region::Code {
                lang,
                header,
                body,
                footer,
            }) => {
                assert_eq!(lang.as_deref(), Some("python"));
                assert!(header.contains("#+BEGIN_SRC python"));
                assert_eq!(body, "print(\"a. b\")\n");
                assert!(footer.contains("#+END_SRC"));
            }
            other => panic!("nested SRC must be Code, got: {other:?} in {regions:?}"),
        }
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("Before."))),
            "pre-src quote body must be Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("After quote.") && p.contains("More.")
            )),
            "post-src quote body must be Prose, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("print"))),
            "SRC body must not leak as Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("print(\"a. b\")"),
            "SRC body must not reflow, got:\n{out}"
        );
        assert!(
            !out.contains("print(\"a.\nb\")"),
            "SRC body must stay literal, got:\n{out}"
        );
        assert!(
            out.contains("After quote.\nMore."),
            "quote prose after SRC must reflow, got:\n{out}"
        );
        assert!(
            out.contains("#+END_SRC\n\nAfter quote.\nMore.\n#+END_QUOTE"),
            "END_SRC must not close the quote, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn center_block_inner_prose_reflows() {
        use crate::format_text;

        let input = "#+BEGIN_CENTER\nCentered one. Centered two.\n#+END_CENTER\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Centered one.") && p.contains("Centered two.")
            )),
            "center body must be Prose, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Centered one."))),
            "center inner must not be Structure, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("#+BEGIN_CENTER\nCentered one.\nCentered two.\n#+END_CENTER"),
            "center sentences must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn verse_block_inner_is_line_preserving_prose() {
        use crate::format_text;

        let input = "#+BEGIN_VERSE\nAlpha line without a period\nBeta line without a period\n#+END_VERSE\nAfter. Next.\n";
        let regions = OrgParser.parse(input);
        let prose: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                Region::Prose(p) => Some(p.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            prose.iter().any(|p| *p == "Alpha line without a period"),
            "verse lines must be Prose, got: {regions:?}"
        );
        assert!(
            prose.iter().any(|p| *p == "Beta line without a period"),
            "each verse line must stay its own Prose region, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Alpha line without a period")
            )),
            "verse inner must not be Structure, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("Alpha line without a period\nBeta line without a period"),
            "verse must keep physical lines, got:\n{out}"
        );
        assert!(
            !out.contains("Alpha line without a period Beta line"),
            "verse must not join physical lines, got:\n{out}"
        );
        assert!(
            out.contains("#+END_VERSE\nAfter.\nNext."),
            "prose after verse must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org / GitHub #179): special-block inner paragraphs reflow.
    fn note_special_block_fixture() -> &'static str {
        concat!(
            "#+BEGIN_NOTE\n",
            "Quoted one. Quoted two.\n",
            "#+END_NOTE\n",
            "After the note. More.\n",
        )
    }

    #[test]
    fn special_block_note_inner_is_prose_not_structure() {
        let input = note_special_block_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_NOTE"))),
            "NOTE opener must stay Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("#+END_NOTE"))),
            "NOTE closer must stay Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Quoted one.") && p.contains("Quoted two.")
            )),
            "NOTE body must be Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Quoted one.")
            )),
            "NOTE body must not freeze as Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("After the note.") && p.contains("More.")
            )),
            "prose after NOTE must remain Prose, got: {regions:?}"
        );
    }

    #[test]
    fn special_block_note_inner_prose_reflows() {
        use crate::format_text;

        let input = note_special_block_fixture();
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("#+BEGIN_NOTE\nQuoted one.\nQuoted two.\n#+END_NOTE"),
            "NOTE sentences must reflow inside the fence, got:\n{out}"
        );
        assert!(
            !out.contains("Quoted one. Quoted two."),
            "NOTE sentences must not stay fused, got:\n{out}"
        );
        assert!(
            out.contains("#+END_NOTE\nAfter the note.\nMore."),
            "prose after NOTE must still reflow, got:\n{out}"
        );
        assert!(
            !out.contains("After the note. More."),
            "fused prose after NOTE must not survive, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn leftover_special_block_names_reflow_like_note() {
        use crate::format_text;

        for name in ["TIP", "WARNING", "ABSTRACT", "PROOF"] {
            let input = format!(
                "#+BEGIN_{name}\nQuoted one. Quoted two.\n#+END_{name}\nAfter the note. More.\n"
            );
            let regions = OrgParser.parse(&input);
            assert!(
                regions.iter().any(|r| matches!(
                    r,
                    Region::Prose(p) if p.contains("Quoted one.") && p.contains("Quoted two.")
                )),
                "{name} body must be Prose, got: {regions:?}"
            );
            assert!(
                !regions.iter().any(|r| matches!(
                    r,
                    Region::Structure(s) if s.contains("Quoted one.")
                )),
                "{name} body must not freeze as Structure, got: {regions:?}"
            );
            let out = format_text(&input, &org_cfg()).unwrap();
            assert!(
                out.contains(&format!(
                    "#+BEGIN_{name}\nQuoted one.\nQuoted two.\n#+END_{name}"
                )),
                "{name} sentences must reflow inside the fence, got:\n{out}"
            );
            assert!(
                out.contains("After the note.\nMore."),
                "prose after {name} must still reflow, got:\n{out}"
            );
            assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
        }
    }

    /// Ticket fixture (Format::Org): `:See also:` is not a drawer,
    /// `: text` is fixed-width, `-----` is a rule.
    fn drawer_fixed_width_rule_fixture() -> &'static str {
        concat!(
            ":See also:\n",
            "This is a note. Second sentence.\n",
            "After the note. More.\n",
            "\n",
            ": First sentence. Second sentence.\n",
            "\n",
            "End of section.\n",
            "-----\n",
            "Start of next. More.\n",
        )
    }

    #[test]
    fn see_also_is_not_a_drawer() {
        use crate::format_text;

        let input = drawer_fixed_width_rule_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This is a note.") && p.contains("Second sentence.")
            )),
            ":See also: must not swallow following prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a note.")
            )),
            "notes after :See also: must not be drawer Structure, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("This is a note.\nSecond sentence."),
            ":See also: must not swallow notes as a drawer, got:\n{out}"
        );
        assert!(
            out.contains("After the note.\nMore."),
            "prose after :See also: must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn fixed_width_colon_space_is_structure() {
        use crate::format_text;

        // Isolated from any `:NAME:` so origin/main cannot pass by swallowing.
        let input = ": First sentence. Second sentence.\nAfter fixed. More after.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(": First sentence. Second sentence.")
            )),
            "fixed-width : text must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("First sentence.")
            )),
            "fixed-width must not be Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("After fixed.") && p.contains("More after.")
            )),
            "prose after fixed-width must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(": First sentence. Second sentence."),
            "fixed-width must keep the colon and not wrap, got:\n{out}"
        );
        assert!(
            !out.contains("First sentence.\nSecond sentence."),
            "fixed-width must not split as prose, got:\n{out}"
        );
        assert!(
            out.contains("After fixed.\nMore after."),
            "prose after fixed-width must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn horizontal_rule_is_structure_boundary() {
        use crate::format_text;

        let input = "End of section.\n-----\nStart of next. More.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "-----")),
            "----- must be Structure (rule), got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("-----"))),
            "----- must not join surrounding prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("End of section.\n-----\nStart of next."),
            "----- must stay a rule boundary, got:\n{out}"
        );
        assert!(
            out.contains("Start of next.\nMore."),
            "prose after the rule must reflow independently, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn named_drawer_still_structure_until_end() {
        use crate::format_text;

        let input = ":LOGBOOK:\nCLOCK: [2026-01-01] First. Second.\n:END:\nAfter drawer. More.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(":LOGBOOK:"))),
            "named drawer opener must be Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("CLOCK:") && s.contains("First. Second.")
            )),
            "drawer body must stay Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First. Second."))),
            "drawer body must not become Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(":LOGBOOK:\nCLOCK: [2026-01-01] First. Second.\n:END:"),
            "real :NAME: drawer must not reflow, got:\n{out}"
        );
        assert!(
            out.contains("After drawer.\nMore."),
            "prose after :END: must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn gyoz_fixture_does_not_swallow_or_drop_colon() {
        use crate::format_text;

        let input = drawer_fixed_width_rule_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(": First sentence. Second sentence.")
            )),
            "fixed-width line in the ticket fixture must be Structure, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "-----")),
            "rule in the ticket fixture must be Structure, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("This is a note.\nSecond sentence."),
            ":See also: must not swallow following prose, got:\n{out}"
        );
        assert!(
            out.contains("After the note.\nMore."),
            "sentences after the note must reflow, got:\n{out}"
        );
        assert!(
            out.lines()
                .any(|l| l == ": First sentence. Second sentence."),
            "fixed-width must keep the leading colon, got:\n{out}"
        );
        assert!(
            out.contains("End of section.\n-----\nStart of next.\nMore."),
            "rule is a boundary; following prose reflows, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org): same-line `\[...\]` and same-line
    /// `\begin{NAME}...\end{NAME}` (snapper-dvg8 / GitHub #108).
    fn dvg8_fixture() -> &'static str {
        concat!(
            "The root is \\[ x = a.b \\] today. Next claim.\n",
            "\n",
            "Prose before.\n",
            "\\begin{equation} x = 1 \\end{equation}\n",
            "This must stay prose. Second sentence.\n",
        )
    }

    #[test]
    fn same_line_bracket_display_math_is_structure() {
        use crate::format_text;

        let input = "The root is \\[ x = a.b \\] today. Next claim.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("x = a.b"))),
            "same-line \\[ x = a.b \\] must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("x = a.b"))),
            "same-line \\[ body must not be Prose, got: {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("The root is"))),
            "words before same-line \\[ must stay Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("today") && p.contains("Next claim")
            )),
            "words after same-line \\] must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("\\[ x = a.b \\]"),
            "same-line \\[...\\] must stay intact, got:\n{out}"
        );
        assert!(
            !out.contains("x = a.\n"),
            "must not split inside same-line display math, got:\n{out}"
        );
        assert!(
            out.contains("today.\nNext claim."),
            "prose after same-line \\] must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn same_line_latex_env_closes_and_does_not_swallow() {
        use crate::format_text;

        let input = "Prose before.\n\\begin{equation} x = 1 \\end{equation}\nThis must stay prose. Second sentence.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("\\begin{equation}") && s.contains("\\end{equation}")
            )),
            "same-line \\begin{{equation}}...\\end{{equation}} must be Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This must stay prose") && p.contains("Second sentence")
            )),
            "prose after same-line env must stay Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This must stay prose")
            )),
            "same-line env must not swallow following prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("\\begin{equation} x = 1 \\end{equation}"),
            "same-line env must stay intact, got:\n{out}"
        );
        assert!(
            out.contains("This must stay prose.\nSecond sentence."),
            "following prose must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn dvg8_fixture_same_line_math_and_env() {
        use crate::format_text;

        let input = dvg8_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("x = a.b"))),
            "fixture \\[ x = a.b \\] must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("x = a.b"))),
            "fixture math must not stay Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("\\begin{equation} x = 1 \\end{equation}")
            )),
            "fixture same-line env must be Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This must stay prose") && p.contains("Second sentence")
            )),
            "fixture following sentences must stay Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This must stay prose")
            )),
            "fixture env must not swallow the rest of the file, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("The root is \\[ x = a.b \\] today.\nNext claim."),
            "fixture prose around \\[...\\] must reflow, got:\n{out}"
        );
        assert!(
            out.contains(
                "\\begin{equation} x = 1 \\end{equation}\nThis must stay prose.\nSecond sentence."
            ),
            "fixture env must close; following prose reflows, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn leftover_start_same_line_bracket_does_not_splice_drop() {
        use crate::format_text;

        // Leftover-start indent stays on the Structure island so splice
        // cannot delete the math line.
        let input = "  \\[ x = a.b \\]\nThis must stay prose. Second sentence.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("\\[ x = a.b \\]") && s.starts_with("  \\[")
            )),
            "indented same-line \\[...\\] must be Structure with indent on the island, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("x = a.b"))),
            "leftover-start math must not be Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This must stay prose") && p.contains("Second sentence")
            )),
            "following sentences must stay Prose, got: {regions:?}"
        );

        let spanned = OrgParser.parse_full(input);
        assert!(
            !spanned.iter().any(|sr| {
                matches!(sr.region, Region::Prose(_))
                    && sr.origin.is_some_and(|o| o.whole().start == 0)
            }),
            "prose_span must not leak over leftover-start indent, got: {spanned:?}"
        );

        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("  \\[ x = a.b \\]\nThis must stay prose.\nSecond sentence."),
            "math line must remain; following prose reflows, got:\n{out}"
        );
        assert!(
            !out.contains("This must stay prose. Second sentence.\nThis must stay prose."),
            "splice must not replace the math line with following prose, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

        let after_prose = "The formula is\n  \\[ x = a.b \\]\nAfter. Next.\n";
        let after_out = format_text(after_prose, &org_cfg()).unwrap();
        assert!(
            after_out.contains("The formula is\n  \\[ x = a.b \\]\n"),
            "preceding prose must not glue onto leftover-start \\[, got:\n{after_out}"
        );
        assert!(
            !after_out.contains("The formula is  \\["),
            "must not glue preceding prose onto indented \\[, got:\n{after_out}"
        );
        assert!(
            after_out.contains("After.\nNext."),
            "prose after leftover-start \\[ must still reflow, got:\n{after_out}"
        );
        assert_eq!(format_text(&after_out, &org_cfg()).unwrap(), after_out);
    }

    /// Ticket fixture (Format::Org): planning and clock stay Structure
    /// (snapper-fcvd / GitHub #111).
    fn fcvd_fixture() -> &'static str {
        concat!(
            "* TODO Task\n",
            "DEADLINE: <2026-01-01 Wed>\n",
            "Body starts here. Second sentence.\n",
            "CLOCK: [2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00] =>  1:00\n",
            "Notes after clock. More notes.\n",
        )
    }

    #[test]
    fn deadline_line_is_structure_not_joined_prose() {
        use crate::format_text;

        let input = "* TODO Task\nDEADLINE: <2026-01-01 Wed>\nBody starts here. Second sentence.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("DEADLINE: <2026-01-01 Wed>")
            )),
            "DEADLINE: must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("DEADLINE:")
            )),
            "DEADLINE: must not join the following paragraph, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Body starts here.") && p.contains("Second sentence.")
            )),
            "prose after DEADLINE: must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("DEADLINE: <2026-01-01 Wed>\nBody starts here."),
            "DEADLINE: must stay its own line, got:\n{out}"
        );
        assert!(
            !out.contains("DEADLINE: <2026-01-01 Wed> Body starts here."),
            "DEADLINE: must not glue onto the next paragraph, got:\n{out}"
        );
        assert!(
            out.contains("Body starts here.\nSecond sentence."),
            "prose after DEADLINE: must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn clock_line_is_structure_not_joined_prose() {
        use crate::format_text;

        let input = concat!(
            "CLOCK: [2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00] =>  1:00\n",
            "Notes after clock. More notes.\n",
        );
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("CLOCK:") && s.contains("=>  1:00")
            )),
            "CLOCK: must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("CLOCK:"))),
            "CLOCK: must not join the following paragraph, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Notes after clock.") && p.contains("More notes.")
            )),
            "prose after CLOCK: must stay Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(
                "CLOCK: [2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00] =>  1:00\nNotes after clock."
            ),
            "CLOCK: must stay its own line, got:\n{out}"
        );
        assert!(
            !out.contains("=>  1:00 Notes after clock."),
            "CLOCK: must not glue onto the next paragraph, got:\n{out}"
        );
        assert!(
            out.contains("Notes after clock.\nMore notes."),
            "prose after CLOCK: must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn scheduled_and_closed_lines_are_structure() {
        use crate::format_text;

        let input = concat!(
            "* TODO Task\n",
            "SCHEDULED: <2026-01-02 Thu>\n",
            "CLOSED: [2026-01-01 Wed 09:00]\n",
            "Body starts here. Second sentence.\n",
        );
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("SCHEDULED: <2026-01-02 Thu>")
            )),
            "SCHEDULED: must be Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("CLOSED: [2026-01-01 Wed 09:00]")
            )),
            "CLOSED: must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("SCHEDULED:") || p.contains("CLOSED:")
            )),
            "planning keywords must not join prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(
                "SCHEDULED: <2026-01-02 Thu>\nCLOSED: [2026-01-01 Wed 09:00]\nBody starts here."
            ),
            "planning lines must stay their own lines, got:\n{out}"
        );
        assert!(
            out.contains("Body starts here.\nSecond sentence."),
            "prose after planning must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn fcvd_fixture_planning_and_clock_do_not_join_following_paragraph() {
        use crate::format_text;

        let input = fcvd_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("* TODO Task")
            )),
            "headline must stay Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("DEADLINE: <2026-01-01 Wed>")
            )),
            "fixture DEADLINE: must be Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("CLOCK:")
                        && s.contains("[2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00]")
            )),
            "fixture CLOCK: must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("DEADLINE:") || p.contains("CLOCK:")
            )),
            "planning/clock must not be Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Body starts here.") && p.contains("Second sentence.")
            )),
            "body after DEADLINE: must stay Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Notes after clock.") && p.contains("More notes.")
            )),
            "notes after CLOCK: must stay Prose, got: {regions:?}"
        );

        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("* TODO Task\nDEADLINE: <2026-01-01 Wed>\nBody starts here."),
            "DEADLINE: must remain the line after the headline, got:\n{out}"
        );
        assert!(
            !out.contains("DEADLINE: <2026-01-01 Wed> Body starts here."),
            "DEADLINE: must not glue onto the body, got:\n{out}"
        );
        assert!(
            out.contains("Body starts here.\nSecond sentence."),
            "body after DEADLINE: must reflow, got:\n{out}"
        );
        assert!(
            out.contains(
                "CLOCK: [2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00] =>  1:00\nNotes after clock."
            ),
            "CLOCK: must stay its own line, got:\n{out}"
        );
        assert!(
            !out.contains("=>  1:00 Notes after clock."),
            "CLOCK: must not glue onto the notes, got:\n{out}"
        );
        assert!(
            out.contains("Notes after clock.\nMore notes."),
            "notes after CLOCK: must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org / GitHub #112): org-element 5.5
    /// citations are atomic so `p.` inside is not a sentence boundary.
    fn cite_page_locator_fixture() -> &'static str {
        "See [cite/t:see;@foo p. 7;@bar pp. 4;by foo]. Next sentence.\n"
    }

    #[test]
    fn org_cite_page_locator_is_not_a_sentence_boundary() {
        use crate::format_text;

        let cite = "[cite/t:see;@foo p. 7;@bar pp. 4;by foo]";
        let input = cite_page_locator_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains(cite) && p.contains("Next sentence.")
            )),
            "citation stays inline Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("See [cite/t:see;@foo p. 7;@bar pp. 4;by foo].\nNext sentence."),
            "sentence after the citation must reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

        // max_width 30 wraps through the locator unless [cite...] is atomic.
        let wrap_cfg = crate::FormatConfig {
            format: crate::format::Format::Org,
            max_width: 30,
            ..Default::default()
        }
        .without_safety_backstops();
        let wrapped = format_text(input, &wrap_cfg).unwrap();
        assert!(
            wrapped.lines().any(|l| l.contains(cite)),
            "wrap must not cut inside the citation, got:\n{wrapped}"
        );
        assert!(
            !wrapped.contains("p.\n7"),
            "must not split on p. inside the citation, got:\n{wrapped}"
        );
        assert!(
            wrapped.contains("Next sentence."),
            "sentence after the citation must still reflow, got:\n{wrapped}"
        );
        assert_eq!(format_text(&wrapped, &wrap_cfg).unwrap(), wrapped);
    }

    /// Ticket fixture (Format::Org / GitHub #211): org-element radio
    /// targets stay one token so an interior period is not a sentence
    /// boundary. `Next sentence.` still splits.
    fn radio_target_fixture() -> &'static str {
        "See <<<the Fourier. transform>>> in the text. Next sentence.\n"
    }

    #[test]
    fn org_radio_target_interior_punct_is_not_a_sentence_boundary() {
        use crate::format_text;

        let radio = "<<<the Fourier. transform>>>";
        let input = radio_target_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains(radio) && p.contains("Next sentence.")
            )),
            "radio target stays inline Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("See <<<the Fourier. transform>>> in the text.\nNext sentence."),
            "sentence after the radio target must reflow, got:\n{out}"
        );
        assert!(
            !out.contains("Fourier.\n") && !out.contains("<<<the Fourier.\ntransform>>>"),
            "must not split inside the radio target, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

        let wrap_cfg = crate::FormatConfig {
            format: crate::format::Format::Org,
            max_width: 28,
            ..Default::default()
        }
        .without_safety_backstops();
        let wrapped = format_text(input, &wrap_cfg).unwrap();
        assert!(
            wrapped.lines().any(|l| l.contains(radio)),
            "wrap must not cut inside the radio target, got:\n{wrapped}"
        );
        assert!(
            !wrapped.contains("Fourier.\ntransform"),
            "must not wrap on the interior period, got:\n{wrapped}"
        );
        assert!(
            wrapped.contains("Next sentence."),
            "sentence after the radio target must still reflow, got:\n{wrapped}"
        );
        assert_eq!(format_text(&wrapped, &wrap_cfg).unwrap(), wrapped);
    }

    #[test]
    fn org_angle_target_interior_punct_is_not_a_sentence_boundary() {
        use crate::format_text;

        let target = "<<sec. intro>>";
        let input = "See <<sec. intro>> in the text. Next sentence.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains(target) && p.contains("Next sentence.")
            )),
            "angle target stays inline Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("See <<sec. intro>> in the text.\nNext sentence."),
            "sentence after the angle target must reflow, got:\n{out}"
        );
        assert!(
            !out.contains("sec.\nintro"),
            "must not split inside the angle target, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org / GitHub #212): org-element macros
    /// stay one token so an interior period is not a sentence boundary.
    /// `Next sentence.` still splits.
    fn org_macro_fixture() -> &'static str {
        "See {{{cite(Smith. 2020)}}} for the source. Next sentence.\n"
    }

    #[test]
    fn org_macro_interior_punct_is_not_a_sentence_boundary() {
        use crate::format_text;

        let mac = "{{{cite(Smith. 2020)}}}";
        let input = org_macro_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains(mac) && p.contains("Next sentence.")
            )),
            "macro stays inline Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("See {{{cite(Smith. 2020)}}} for the source.\nNext sentence."),
            "sentence after the macro must reflow, got:\n{out}"
        );
        assert!(
            !out.contains("Smith.\n") && !out.contains("{{{cite(Smith.\n2020)}}}"),
            "must not split inside the macro, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

        let wrap_cfg = crate::FormatConfig {
            format: crate::format::Format::Org,
            max_width: 28,
            ..Default::default()
        }
        .without_safety_backstops();
        let wrapped = format_text(input, &wrap_cfg).unwrap();
        assert!(
            wrapped.lines().any(|l| l.contains(mac)),
            "wrap must not cut inside the macro, got:\n{wrapped}"
        );
        assert!(
            !wrapped.contains("Smith.\n2020"),
            "must not wrap on the interior period, got:\n{wrapped}"
        );
        assert!(
            wrapped.contains("Next sentence."),
            "sentence after the macro must still reflow, got:\n{wrapped}"
        );
        assert_eq!(format_text(&wrapped, &wrap_cfg).unwrap(), wrapped);
    }

    /// Ticket fixture (Format::Org / GitHub #214): org-element inline
    /// `src_lang{...}` stays one token so an interior period is not a
    /// sentence boundary. `Next sentence.` still splits. Same class:
    /// `call_name(...)`.
    fn inline_src_fixture() -> &'static str {
        "Use src_python{print(1. 2)} today. Next sentence.\n"
    }

    #[test]
    fn org_inline_src_interior_punct_is_not_a_sentence_boundary() {
        use crate::format_text;

        let src = "src_python{print(1. 2)}";
        let input = inline_src_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains(src) && p.contains("Next sentence.")
            )),
            "inline src stays inline Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("Use src_python{print(1. 2)} today.\nNext sentence."),
            "sentence after the inline src must reflow, got:\n{out}"
        );
        assert!(
            !out.contains("1.\n") && !out.contains("print(1.\n2)"),
            "must not split inside the inline src, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

        let wrap_cfg = crate::FormatConfig {
            format: crate::format::Format::Org,
            max_width: 20,
            ..Default::default()
        }
        .without_safety_backstops();
        let wrapped = format_text(input, &wrap_cfg).unwrap();
        assert!(
            wrapped.lines().any(|l| l.contains(src)),
            "wrap must not cut inside the inline src, got:\n{wrapped}"
        );
        assert!(
            !wrapped.contains("1.\n2"),
            "must not wrap on the interior period, got:\n{wrapped}"
        );
        assert!(
            wrapped.contains("Next sentence."),
            "sentence after the inline src must still reflow, got:\n{wrapped}"
        );
        assert_eq!(format_text(&wrapped, &wrap_cfg).unwrap(), wrapped);
    }

    #[test]
    fn org_inline_call_interior_punct_is_not_a_sentence_boundary() {
        use crate::format_text;

        let call = "call_name(1. 2)";
        let input = "Use call_name(1. 2) today. Next sentence.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains(call) && p.contains("Next sentence.")
            )),
            "inline babel call stays inline Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("Use call_name(1. 2) today.\nNext sentence."),
            "sentence after the inline call must reflow, got:\n{out}"
        );
        assert!(
            !out.contains("1.\n") && !out.contains("call_name(1.\n2)"),
            "must not split inside the inline call, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org / GitHub #169): `file:\S+` must not
    /// swallow trailing `.!?` so `See file:/tmp/foo. Next` is two sentences.
    #[test]
    fn org_file_token_does_not_swallow_trailing_sentence_punct() {
        use crate::format_text;

        let two_line = "See file:/tmp/foo.\nNext sentence.\n";
        let out = format_text(two_line, &org_cfg()).unwrap();
        assert_eq!(
            out, two_line,
            "existing newline must stay two lines, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

        let same_line = "See file:/tmp/foo. Next sentence.\n";
        let out = format_text(same_line, &org_cfg()).unwrap();
        assert_eq!(
            out, two_line,
            "same-line file: period must split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

        let bang = "See file:/tmp/foo!\nNext sentence.\n";
        let out = format_text("See file:/tmp/foo! Next sentence.\n", &org_cfg()).unwrap();
        assert_eq!(out, bang, "same-line file: bang must split, got:\n{out}");
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org / GitHub #177): org-comment-regexp
    /// requires space or EOL after `#`. `#foo` is prose.
    fn hash_comment_space_or_eol_fixture() -> &'static str {
        concat!(
            "#not-a-comment This is a long sentence that must reflow as prose. Second sentence.\n",
            "# This is a real comment and must stay frozen.\n",
        )
    }

    #[test]
    fn hash_without_space_is_not_a_comment() {
        assert!(!OrgParser::is_comment(
            "#not-a-comment This is a long sentence that must reflow as prose."
        ));
        assert!(!OrgParser::is_comment("#foo"));
        assert!(!OrgParser::is_comment("#+TITLE: x"));
        assert!(!OrgParser::is_comment("prose"));
        assert!(OrgParser::is_comment("# This is a real comment"));
        assert!(OrgParser::is_comment("#"));
        assert!(OrgParser::is_comment("  # indented comment"));
        assert!(OrgParser::is_comment("\t# tab-indented comment"));
    }

    #[test]
    fn hash_without_space_is_prose_and_splits() {
        use crate::format_text;

        let input = hash_comment_space_or_eol_fixture();
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("#not-a-comment")
                        && p.contains("must reflow as prose.")
                        && p.contains("Second sentence.")
            )),
            "#not-a-comment line must be Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("#not-a-comment")
            )),
            "#not-a-comment must not freeze as Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains("# This is a real comment and must stay frozen.")
            )),
            "#<space> comment must stay Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("# This is a real comment")
            )),
            "#<space> comment must not be Prose, got: {regions:?}"
        );

        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(
                "#not-a-comment This is a long sentence that must reflow as prose.\nSecond sentence."
            ),
            "#not-a-comment prose must split, got:\n{out}"
        );
        assert!(
            !out.contains(
                "#not-a-comment This is a long sentence that must reflow as prose. Second sentence."
            ),
            "#not-a-comment must not stay fused, got:\n{out}"
        );
        assert!(
            out.lines()
                .any(|l| l == "# This is a real comment and must stay frozen."),
            "real comment must stay frozen, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn bare_hash_eol_is_a_comment() {
        use crate::format_text;

        let input = "#\nAfter. Next.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == "#")),
            "bare # must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains('#'))),
            "bare # must not be Prose, got: {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("#\nAfter.\nNext."),
            "bare # stays a comment; following prose reflows, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org / GitHub #178): `#+BEGIN: NAME` is a
    /// dynamic block, not a one-line keyword. Body stays Structure.
    fn dynamic_block_clocktable_fixture() -> &'static str {
        concat!(
            "#+BEGIN: clocktable :scope file\n",
            "This is a long sentence inside a dynamic block that must stay frozen. Second sentence.\n",
            "#+END:\n",
            "Following paragraph. Another sentence.\n",
        )
    }

    #[test]
    fn dynamic_block_begin_matches_org_element_open_re() {
        assert!(OrgParser::is_dynamic_block_begin(
            "#+BEGIN: clocktable :scope file"
        ));
        assert!(OrgParser::is_dynamic_block_begin("  #+begin: columnview"));
        assert!(OrgParser::is_dynamic_block_begin("\t#+BEGIN: clocktable"));
        assert!(!OrgParser::is_dynamic_block_begin("#+BEGIN:"));
        assert!(!OrgParser::is_dynamic_block_begin("#+BEGIN:   "));
        assert!(!OrgParser::is_dynamic_block_begin("#+BEGIN_SRC rust"));
        assert!(!OrgParser::is_dynamic_block_begin("#+BEGIN_QUOTE"));
        assert!(!OrgParser::is_dynamic_block_begin("prose"));
        assert!(OrgParser::is_dynamic_block_end("#+END:"));
        assert!(OrgParser::is_dynamic_block_end("  #+end:"));
        assert!(OrgParser::is_dynamic_block_end("#+END"));
        assert!(!OrgParser::is_dynamic_block_end("#+END_SRC"));
        assert!(!OrgParser::is_dynamic_block_end("#+END_QUOTE"));
        assert!(!OrgParser::is_dynamic_block_end("#+BEGIN: clocktable"));
    }

    #[test]
    fn dynamic_block_body_is_structure_following_prose_splits() {
        use crate::format_text;

        let input = dynamic_block_clocktable_fixture();
        let regions = OrgParser.parse(input);
        let structure: String = regions
            .iter()
            .filter_map(|r| match r {
                Region::Structure(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            structure.contains("#+BEGIN: clocktable :scope file")
                && structure.contains("must stay frozen. Second sentence.")
                && structure.contains("#+END:"),
            "dynamic block BEGIN through END must be Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("must stay frozen")
            )),
            "dynamic block body must not be Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("Following paragraph.") && p.contains("Another sentence.")
            )),
            "following paragraph must stay Prose, got: {regions:?}"
        );

        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(concat!(
                "#+BEGIN: clocktable :scope file\n",
                "This is a long sentence inside a dynamic block that must stay frozen. Second sentence.\n",
                "#+END:\n",
            )),
            "dynamic block must stay frozen, got:\n{out}"
        );
        assert!(
            !out.contains("must stay frozen.\nSecond sentence."),
            "dynamic block body must not split, got:\n{out}"
        );
        assert!(
            out.contains("#+END:\nFollowing paragraph.\nAnother sentence."),
            "following paragraph must still split, got:\n{out}"
        );
        assert!(
            !out.contains("Following paragraph. Another sentence."),
            "following paragraph must not stay fused, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org / GitHub #180): org-footnote-definition-re.
    fn footnote_definition_fixture() -> &'static str {
        concat!(
            "See the claim.[fn:1]\n",
            "\n",
            "[fn:1] This is a long footnote sentence that must stay inside the definition. Second sentence.\n",
        )
    }

    #[test]
    fn footnote_definition_marker_follows_org_footnote_el() {
        assert_eq!(org_footnote_definition_marker_len("[fn:1] "), Some(7));
        assert_eq!(org_footnote_definition_marker_len("[fn:1]"), Some(6));
        assert_eq!(org_footnote_definition_marker_len("[fn:note] "), Some(10));
        assert_eq!(
            org_footnote_definition_marker_len("[fn:foo-bar] body"),
            Some(13)
        );
        assert_eq!(
            org_footnote_definition_marker_len("[fn:foo_bar] body"),
            Some(13)
        );
        assert_eq!(org_footnote_definition_marker_len("  [fn:1] "), None);
        assert_eq!(
            org_footnote_definition_marker_len("See the claim.[fn:1]"),
            None
        );
        assert_eq!(org_footnote_definition_marker_len("[fn:: inline]"), None);
        assert_eq!(
            org_footnote_definition_marker_len("[fn:name: inline]"),
            None
        );
        assert_eq!(org_footnote_definition_marker_len("[fn:]"), None);
        assert_eq!(org_footnote_definition_marker_len("[fn:1:inline]"), None);
        assert_eq!(org_footnote_definition_marker_len("[cite:@foo]"), None);
        assert_eq!(org_footnote_definition_marker_len("[1] old style"), None);
    }

    #[test]
    fn footnote_opener_is_structure_body_is_hung_prose() {
        let regions = OrgParser.parse(footnote_definition_fixture());
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "[fn:1] ")),
            "[fn:1] must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("This is a long footnote sentence that must stay inside the definition.")
                        && s.contains("Second sentence.")
            )),
            "footnote body must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long footnote sentence")
            )),
            "footnote body must not stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("See the claim.[fn:1]")
            )),
            "inline [fn:1] reference must stay Prose, got {regions:?}"
        );
    }

    #[test]
    fn footnote_fixture_body_splits_and_hangs() {
        use crate::format_text;

        let input = footnote_definition_fixture();
        let out = format_text(input, &org_cfg()).unwrap();
        assert_eq!(
            out,
            concat!(
                "See the claim.[fn:1]\n",
                "\n",
                "[fn:1] This is a long footnote sentence that must stay inside the definition.\n",
                "       Second sentence.\n",
            ),
            "footnote opener stays; body hangs and splits, got:\n{out}"
        );
        assert!(
            !out.lines().any(|l| l == "Second sentence."),
            "must not emit a column-0 second sentence, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &org_cfg()).unwrap(),
            out,
            "hung footnote must be identity, got:\n{out}"
        );
    }

    #[test]
    fn named_footnote_is_structure_plus_hung_prose() {
        use crate::format_text;

        let input = "[fn:note] Footnote text. Second sentence.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "[fn:note] ")),
            "[fn:note] must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("Footnote text.") && s.contains("Second sentence.")
            )),
            "[fn:note] body must be Prose, got {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert_eq!(
            out, "[fn:note] Footnote text.\n          Second sentence.\n",
            "[fn:note] body must hang and split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    /// Ticket fixture (Format::Org / GitHub #204): org-element-parsed-keywords
    /// is CAPTION only. Opener is Structure; value hangs and splits.
    fn caption_fixture() -> &'static str {
        concat!(
            "#+CAPTION: This is a long figure caption that must reflow as prose. Second sentence.\n",
            "[[file:plot.png]]\n",
            "After the figure. More.\n",
        )
    }

    #[test]
    fn caption_marker_follows_org_element_keyword_re() {
        assert_eq!(org_caption_marker_len("#+CAPTION: "), Some(11));
        assert_eq!(org_caption_marker_len("#+CAPTION:"), Some(10));
        assert_eq!(org_caption_marker_len("#+caption: text"), Some(11));
        assert_eq!(
            org_caption_marker_len("#+CAPTION[Short. Title.]: "),
            Some("#+CAPTION[Short. Title.]: ".len())
        );
        assert_eq!(
            org_caption_marker_len(
                "#+CAPTION: This is a long figure caption that must reflow as prose."
            ),
            Some(11)
        );
        assert_eq!(org_caption_marker_len("  #+CAPTION: x"), Some(13));
        assert_eq!(org_caption_marker_len("#+CAPTION+: x"), Some(12));
        assert_eq!(org_caption_marker_len("#+CAPTION[s]+: x"), Some(15));
        assert_eq!(org_caption_marker_len("#+NAME: fig"), None);
        assert_eq!(org_caption_marker_len("#+ATTR_HTML: :alt x"), None);
        assert_eq!(org_caption_marker_len("#+ATTR_LATEX: :width 1"), None);
        assert_eq!(org_caption_marker_len("#+TITLE: x"), None);
        assert_eq!(org_caption_marker_len("#+CAPTIONS: x"), None);
        assert_eq!(org_caption_marker_len("#+BEGIN_SRC rust"), None);
        assert_eq!(org_caption_marker_len("See #+CAPTION: x"), None);
        assert_eq!(org_caption_marker_len("#+CAPTION : x"), None);
    }

    #[test]
    fn caption_opener_is_structure_value_is_hung_prose() {
        let regions = OrgParser.parse(caption_fixture());
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "#+CAPTION: ")),
            "#+CAPTION: must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("This is a long figure caption that must reflow as prose.")
                        && s.contains("Second sentence.")
            )),
            "CAPTION value must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long figure caption")
            )),
            "CAPTION value must not stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("[[file:plot.png]]")
            )),
            "figure link line must stay Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("After the figure.") && s.contains("More.")
            )),
            "following prose must still be Prose, got {regions:?}"
        );
    }

    #[test]
    fn caption_fixture_value_splits_and_hangs() {
        use crate::format_text;

        let input = caption_fixture();
        let out = format_text(input, &org_cfg()).unwrap();
        assert_eq!(
            out,
            concat!(
                "#+CAPTION: This is a long figure caption that must reflow as prose.\n",
                "           Second sentence.\n",
                "[[file:plot.png]]\n",
                "After the figure.\n",
                "More.\n",
            ),
            "CAPTION opener stays; value hangs and splits, got:\n{out}"
        );
        assert!(
            !out.lines().any(|l| l == "Second sentence."),
            "must not emit a column-0 second caption sentence, got:\n{out}"
        );
        assert!(
            !out.contains("[[file:plot.png]] After the figure."),
            "link line must not join following prose, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &org_cfg()).unwrap(),
            out,
            "hung CAPTION must be identity, got:\n{out}"
        );
    }

    #[test]
    fn caption_dual_short_title_stays_structure() {
        use crate::format_text;

        let input = "#+CAPTION[Short. Title.]: Long caption. Second.\n";
        let regions = OrgParser.parse(input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s == "#+CAPTION[Short. Title.]: "
            )),
            "dual [short] must stay in the Structure opener, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("Long caption.") && s.contains("Second.")
            )),
            "dual long value must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("Short. Title.")
            )),
            "short title must not be Prose, got {regions:?}"
        );
        let out = format_text(input, &org_cfg()).unwrap();
        assert_eq!(
            out,
            concat!(
                "#+CAPTION[Short. Title.]: Long caption.\n",
                "                          Second.\n",
            ),
            "dual short stays; long value hangs and splits, got:\n{out}"
        );
        assert!(
            out.contains("#+CAPTION[Short. Title.]:"),
            "Short. Title. must not split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }

    #[test]
    fn name_and_attr_stay_whole_line_structure() {
        use crate::format_text;

        let input = concat!(
            "#+NAME: First sentence. Second sentence.\n",
            "#+ATTR_HTML: :alt First sentence. Second.\n",
            "#+ATTR_LATEX: :width 0.9 :alt First sentence. Second sentence.\n",
            "#+CAPTION: Long caption. Second.\n",
            "[[file:plot.png]]\n",
            "After the figure. More.\n",
        );
        let regions = OrgParser.parse(input);
        for needle in [
            "#+NAME: First sentence. Second sentence.",
            "#+ATTR_HTML: :alt First sentence. Second.",
            "#+ATTR_LATEX: :width 0.9 :alt First sentence. Second sentence.",
        ] {
            assert!(
                regions
                    .iter()
                    .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
                "{needle} must stay Structure, got {regions:?}"
            );
            assert!(
                !regions
                    .iter()
                    .any(|r| matches!(r, Region::Prose(s) if s.contains(needle))),
                "{needle} must not be Prose, got {regions:?}"
            );
        }
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains("#+NAME: First sentence. Second sentence.\n"),
            "#+NAME: must stay whole-line, got:\n{out}"
        );
        assert!(
            out.contains("#+ATTR_HTML: :alt First sentence. Second.\n"),
            "#+ATTR_HTML: must stay whole-line, got:\n{out}"
        );
        assert!(
            out.contains("#+ATTR_LATEX: :width 0.9 :alt First sentence. Second sentence.\n"),
            "#+ATTR_LATEX: must stay whole-line, got:\n{out}"
        );
        assert!(
            out.contains("#+CAPTION: Long caption.\n           Second.\n"),
            "CAPTION value must still hang, got:\n{out}"
        );
        assert!(
            out.contains("After the figure.\nMore."),
            "following prose must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }
}
