use regex::Regex;
use std::sync::LazyLock;

/// Matches segments ending with sentence punctuation followed by closing quotes/parens,
/// where the punctuation is not a true sentence boundary (e.g., `"wow!" and`, `(emphasis!) loudly`).
static QUOTED_PUNCT_END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r##"[.!?]["')\]]+\s*$"##).expect("valid quoted-punct regex"));

use crate::abbreviations;
use crate::sentence::SentenceSplitter;

/// Patterns for inline tokens that should not be split across sentences.
/// These get replaced with safe placeholders before sentence detection.
static INLINE_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        &[
            r"\[\[[^\]]*\]\]",           // Org links: [[url]] or [[url][desc]]
            r"\[\[[^\]]*\]\[[^\]]*\]\]", // Org links with desc
            // org-element 5.5 citation: [cite/style:prefix;@key p. 7;suffix]
            // Must be atomic so a page locator is not a sentence boundary.
            r"\[cite(?:/[-a-zA-Z0-9_/]*)?:[^\]]*\]",
            // org-element-footnote-reference-parser / org-footnote-re
            // inline arm: [fn::def] / [fn:LABEL:def]. First ] closes.
            // Standard [fn:LABEL] is not a token (GitHub #231).
            r"\[fn:(?:[-_\w]+)?:[^\]]*\]",
            // org-element-radio-target-parser / org-radio-target-regexp:
            // <<<contents>>> with no <, >, or newline. Must precede <<...>>
            // so the triple-angle form stays one token (GitHub #211).
            r"<<<[^<>\n]+>>>",
            // org-element-target-parser / org-target-regexp: <<contents>>
            r"<<[^<>\n]+>>",
            // org-element-macro-parser / org-macro.el:
            // {{{name}}} or {{{name(args)}}}. Name is
            // [a-zA-Z][-a-zA-Z0-9_]*; args are non-greedy and may
            // span lines. Two-brace {{...}} is not a macro (GitHub #212).
            r"\{\{\{[a-zA-Z][-a-zA-Z0-9_]*(?:\((?:.|\n)*?\))?\}\}\}",
            r"\[[^\]]+\]\([^)]+\)",  // Markdown links: [text](url)
            r"!\[[^\]]*\]\([^)]+\)", // Markdown images: ![alt](url)
            // CommonMark 0.31.2 §6.3 full / collapsed reference links.
            // The label must follow the text immediately. Shortcut `[text]`
            // is not matched: that would swallow every bracket group.
            r"!\[[^\]]*\]\[[^\]]*\]", // Markdown reference images: ![alt][ref]
            r"\[[^\]]+\]\[[^\]]*\]",  // Markdown reference links: [text][ref]
            r"\$\$[^$\n]+\$\$",       // Display math: $$...$$
            r"\$[^$\n]+\$",           // Inline math: $...$
            r"\\\([^\\\n]+\\\)",      // LaTeX inline math: \(...\)
            r"\\\[[^\n]+?\\\]",       // Org / LaTeX display math fragment: \[...\]
            r"\\([a-zA-Z]+)\{[^}]*\}", // LaTeX commands: \cmd{arg}
            // Org emphasis must be protected before sentence splits so a line
            // cannot begin with `*rest` (false headline) or leave markers open.
            // Org requires a non-space immediately after the opener and before
            // the closer; content may include spaces and sentence punctuation.
            // (Rust `regex` has no lookbehind; encode the border as char classes.)
            r"\*[^*\s\n](?:[^*\n]*[^*\s\n])?\*", // Org bold: *text*
            r"/[^/\s\n](?:[^/\n]*[^/\s\n])?/",   // Org italic: /text/
            r"_[^_\s\n](?:[^_\n]*[^_\s\n])?_",   // Org underline: _text_
            r"\+[^\+\s\n](?:[^\+\n]*[^\+\s\n])?\+", // Org strike-through: +text+
            // Org `=verbatim=` / `~code~` and Markdown backtick spans are
            // paired below: a regex that forbids the delimiter inside the
            // span closes on the first inner copy and leaves the real closer
            // (and any period before it) unprotected.
            r"<[A-Za-z][A-Za-z0-9+.\-]*:[^\s<>]*>", // Autolink: <http://...>
            r"<[^\s<>@]+@[^\s<>]+>",                // Autolink: <user@host>
            r#"https?://\S+[^.\s!?,;:)\]'""]"#,     // URLs (don't swallow trailing punctuation)
            r#"file:\S+[^.\s!?,;:)\]'""]"#, // Org file: links (don't swallow trailing punctuation)
            r"@@[a-zA-Z]+:[^@]*@@",         // Org inline export snippets: @@backend:value@@
        ]
        .join("|"),
    )
    .expect("valid inline token regex")
});

// Static patterns removed -- now compiled per-instance in UnicodeSentenceSplitter::for_lang().

/// Sentence splitter using Unicode UAX #29 with abbreviation-aware merging.
pub struct UnicodeSentenceSplitter {
    /// Compiled regex for extra user-provided abbreviations, if any.
    extra_pattern: Option<Regex>,
    /// Compiled abbreviation pattern for the selected language.
    lang_abbrev_pattern: Regex,
    /// Compiled multi-abbreviation pattern for the selected language.
    lang_multi_pattern: Regex,
    /// Extra LaTeX command names tokenized like `\verb` before split.
    extra_verbatim_commands: Vec<String>,
}

impl UnicodeSentenceSplitter {
    /// Create a splitter with only built-in English abbreviations.
    pub fn new() -> Self {
        Self::for_lang("en", &[])
    }

    /// Create a splitter with additional user-provided abbreviations.
    pub fn with_extra_abbreviations(extras: &[String]) -> Self {
        Self::for_lang("en", extras)
    }

    /// Create a splitter for a specific language, optionally with extra abbreviations.
    pub fn for_lang(lang: &str, extras: &[String]) -> Self {
        let abbrevs = abbreviations::abbreviations_for_lang(lang);
        let multi = abbreviations::multi_abbrevs_for_lang(lang);

        let alts: Vec<&str> = abbrevs.to_vec();
        let pattern = format!(r#"(?:^|[\s"'`(\[])(?:{})$"#, alts.join("|"));
        let lang_abbrev_pattern = Regex::new(&pattern).expect("valid abbreviation regex");

        let multi_alts: Vec<String> = multi.iter().map(|a| regex::escape(a)).collect();
        let multi_pattern = format!(r"(?:^|\s)(?:{})$", multi_alts.join("|"));
        let lang_multi_pattern =
            Regex::new(&multi_pattern).expect("valid multi-abbreviation regex");

        let extra_pattern = if extras.is_empty() {
            None
        } else {
            let alts: Vec<String> = extras.iter().map(|a| regex::escape(a)).collect();
            let pattern = format!(r"(?:^|\s)(?:{})$", alts.join("|"));
            Some(Regex::new(&pattern).expect("valid extra abbreviation regex"))
        };

        Self {
            extra_pattern,
            lang_abbrev_pattern,
            lang_multi_pattern,
            extra_verbatim_commands: Vec::new(),
        }
    }

    /// Extra LaTeX command names tokenized like `\verb` before split.
    pub fn with_verbatim_commands(mut self, cmds: Vec<String>) -> Self {
        self.extra_verbatim_commands = cmds;
        self
    }

    pub(crate) fn verbatim_commands(&self) -> &[String] {
        &self.extra_verbatim_commands
    }
}

impl Default for UnicodeSentenceSplitter {
    fn default() -> Self {
        Self::new()
    }
}

/// Protect links, emphasis, math, and other inline tokens so a base segmenter
/// (UAX or neural) cannot cut inside them. Shared by rules and neural paths.
pub fn protect_inline_tokens(text: &str) -> (String, Vec<String>) {
    protect_inline_tokens_with(text, &[])
}

/// Like [`protect_inline_tokens`], with extra LaTeX command names treated
/// like `\verb` (delimiter is the next character).
pub fn protect_inline_tokens_with(
    text: &str,
    extra_verbatim_commands: &[String],
) -> (String, Vec<String>) {
    let mut placeholders: Vec<String> = Vec::new();
    let after_verb = protect_latex_verbatim(text, &mut placeholders, extra_verbatim_commands);
    // After `\verb|` so a configured delimiter is gone. Unlisted
    // `\Verb|a.b! c|` keeps the letter prefix, so it is not a
    // substitution_ref (Docutils start-string; GitHub #233).
    let after_rst = protect_rst_substitution_refs(&after_verb, &mut placeholders);
    // org-element inline src / babel-call before paired `=`/`~` so a body
    // like `src_python{~x~}` stays one object, not an Org code span.
    // Footnote references run in the same walk so a nested `[[link]]`
    // closer does not end `[fn:: …]` early (GitHub #231).
    let after_org = protect_org_inline_src_and_call(&after_rst, &mut placeholders);
    let after_spans = protect_paired_spans(&after_org, &mut placeholders);
    let protected = INLINE_TOKEN_RE.replace_all(&after_spans, |caps: &regex::Captures| {
        let idx = placeholders.len();
        placeholders.push(caps[0].to_string());
        format!("\x00PH{idx}\x00")
    });
    (protected.into_owned(), placeholders)
}

/// `\verb|...|` / `\lstinline[...]!...!` so inner `.!?%` cannot split or comment.
fn protect_latex_verbatim(
    text: &str,
    placeholders: &mut Vec<String>,
    extra_verbatim_commands: &[String],
) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        if bytes[i] == b'\\' {
            if let Some(end) = latex_verb_span_end_with(text, i, extra_verbatim_commands) {
                push_placeholder(&mut out, placeholders, &text[i..end]);
                i = end;
                continue;
            }
        }
        let ch = text[i..].chars().next().expect("i is in range");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Byte end of a `\verb` / `\lstinline` / extra-name span starting at `at`.
///
/// `\verb` / `\verb*`: next character is the delimiter; content runs to the
/// same character. `\lstinline` / `\lstinline*` may take optional `[...]`
/// before a delimiter or a `{...}` brace body. Extra names are tokenized
/// like `\verb`. With no closer, the span runs to end of line so an inner
/// `%` is not a comment.
pub(crate) fn latex_verb_span_end_with(
    text: &str,
    at: usize,
    extra_verbatim_commands: &[String],
) -> Option<usize> {
    let rest = text.get(at..)?;
    if !rest.starts_with('\\') {
        return None;
    }
    let after_bs = at + 1;
    let tail = text.get(after_bs..)?;
    let (mut i, is_lst) = if let Some(stripped) = tail.strip_prefix("lstinline") {
        if stripped.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return None;
        }
        (after_bs + "lstinline".len(), true)
    } else if let Some(stripped) = tail.strip_prefix("verb") {
        if stripped.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return None;
        }
        (after_bs + "verb".len(), false)
    } else {
        let name = match_extra_verb_command(tail, extra_verbatim_commands)?;
        (after_bs + name.len(), false)
    };

    if text.get(i..)?.starts_with('*') {
        i += 1;
    }

    if is_lst {
        i = skip_ascii_ws(text, i);
        if text.get(i..).is_some_and(|s| s.starts_with('[')) {
            match skip_bracket_group(text, i) {
                Some(end) => i = skip_ascii_ws(text, end),
                None => return Some(line_end(text, i)),
            }
        }
    }

    let delim = text.get(i..).and_then(|s| s.chars().next())?;
    if delim == '\n' {
        return None;
    }
    i += delim.len_utf8();

    if is_lst && delim == '{' {
        return Some(find_unescaped_brace_close(text, i).unwrap_or_else(|| line_end(text, i)));
    }

    while i < text.len() {
        let ch = text[i..].chars().next()?;
        if ch == '\n' {
            return Some(i);
        }
        if ch == delim {
            return Some(i + ch.len_utf8());
        }
        i += ch.len_utf8();
    }
    Some(text.len())
}

fn line_end(text: &str, from: usize) -> usize {
    text[from..]
        .find('\n')
        .map(|rel| from + rel)
        .unwrap_or(text.len())
}

/// Longest extra command name that is a prefix of `tail` and is not
/// followed by an ASCII letter (`\Verb` must not steal `\Verbatim`).
fn match_extra_verb_command<'a>(tail: &'a str, extras: &'a [String]) -> Option<&'a str> {
    let mut best: Option<&str> = None;
    for name in extras {
        if name.is_empty() || name == "verb" || name == "lstinline" {
            continue;
        }
        let Some(stripped) = tail.strip_prefix(name.as_str()) else {
            continue;
        };
        if stripped.starts_with(|c: char| c.is_ascii_alphabetic()) {
            continue;
        }
        if best.is_none_or(|b| name.len() > b.len()) {
            best = Some(name.as_str());
        }
    }
    best
}

fn skip_ascii_ws(text: &str, mut i: usize) -> usize {
    while i < text.len() && matches!(text.as_bytes()[i], b' ' | b'\t') {
        i += 1;
    }
    i
}

fn skip_bracket_group(text: &str, open_at: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open_at) != Some(&b'[') {
        return None;
    }
    let mut depth = 0;
    let mut i = open_at;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => return None,
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

fn find_unescaped_brace_close(text: &str, mut i: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            return None;
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'}' {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

/// Docutils Inliner.substitution_ref: `|text|` plus optional `_` / `__`.
/// Text may not begin or end with whitespace (line_block is `| `).
/// Start-string must be at BOS or after 7-bit non-alphanumeric, so
/// `\Verb|a.b|` is not a substitution (GitHub #233).
fn protect_rst_substitution_refs(text: &str, placeholders: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if let Some(end) = rst_substitution_ref_span_end(text, i) {
            push_placeholder(&mut out, placeholders, &text[i..end]);
            i = end;
            continue;
        }
        let ch = text[i..].chars().next().expect("i is in range");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Byte end of a substitution_ref starting at `at`, or None.
fn rst_substitution_ref_span_end(text: &str, at: usize) -> Option<usize> {
    if !text.get(at..)?.starts_with('|') {
        return None;
    }
    if at > 0 {
        let prev = text[..at].chars().next_back()?;
        // Docutils start-string: whitespace or 7-bit non-alphanumeric.
        // A preceding `\` is an escape, not a start.
        if prev == '\\' || !prev.is_ascii() || prev.is_ascii_alphanumeric() {
            return None;
        }
    }
    let mut chars = text[at + 1..].chars();
    let first = chars.next()?;
    if first == '|' || first == '\n' || first.is_whitespace() {
        return None;
    }
    let mut last = first;
    let mut consumed = first.len_utf8();
    for ch in chars {
        if ch == '\n' {
            return None;
        }
        if ch == '|' {
            if last.is_whitespace() {
                return None;
            }
            let mut end = at + 1 + consumed + '|'.len_utf8();
            let mut unders = 0;
            for c in text[end..].chars() {
                if unders < 2 && c == '_' {
                    end += 1;
                    unders += 1;
                    continue;
                }
                if c.is_ascii_alphanumeric() {
                    return None;
                }
                break;
            }
            return Some(end);
        }
        last = ch;
        consumed += ch.len_utf8();
    }
    None
}

/// org-element-inline-src-block-parser / org-element-inline-babel-call-parser.
/// `src_LANG[headers]{body}` and `call_NAME[inside](args)[end]` stay one
/// object so an interior period is not a sentence or wrap boundary.
/// Footnote references run in the same walk (GitHub #231).
fn protect_org_inline_src_and_call(text: &str, placeholders: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if let Some(end) = org_inline_src_or_call_span_end(text, i)
            .or_else(|| org_footnote_reference_span_end(text, i))
        {
            push_placeholder(&mut out, placeholders, &text[i..end]);
            i = end;
            continue;
        }
        let ch = text[i..].chars().next().expect("i is in range");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn org_inline_src_or_call_span_end(text: &str, at: usize) -> Option<usize> {
    org_inline_src_span_end(text, at).or_else(|| org_inline_call_span_end(text, at))
}

/// org-footnote-re inline arm + org-element-footnote-reference-parser.
///
/// `[fn::def]` and `[fn:LABEL:def]` only. Standard `[fn:LABEL]` stays
/// prose so `See the claim.[fn:1]` is not split at `claim.`.
/// Closing `]` is scan-lists on `[]` (nested brackets). A newline
/// ends the object (org-footnote-re `.` / org-element successor
/// `line-end-position`).
fn org_footnote_reference_span_end(text: &str, at: usize) -> Option<usize> {
    let rest = text.get(at..)?;
    if !rest.starts_with("[fn:") {
        return None;
    }
    let bytes = text.as_bytes();
    let mut i = at + 4;
    while i < bytes.len() {
        let ch = text[i..].chars().next()?;
        if ch == '-' || ch == '_' || ch.is_alphanumeric() {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    if i >= bytes.len() || bytes[i] != b':' {
        return None;
    }
    org_scan_lists_same_line(text, at, b'[', b']')
}

/// `scan-lists` on one pair, stopped at newline.
fn org_scan_lists_same_line(text: &str, open_at: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open_at) != Some(&open) {
        return None;
    }
    let mut depth = 1usize;
    let mut i = open_at + 1;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            return None;
        }
        if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

/// `\<` in org-element-inline-src-block-parser / inline-babel-call-parser
/// (`looking-at` `\<src_` / `\<call_`). Word-start, not symbol-start.
///
/// `org-element--object-lex` matches `[_^][-{(*+.,[:alnum:]]` first, so
/// after a word the subscript parser consumes `_src` / `_call` and the
/// leftover braces stay prose. `_src_` / `_call_` at BOL or after
/// whitespace is still an object (`\<` matches at `s`). Hyphen is not a
/// subscript opener here, so `foo-src_python` is an object. Word
/// characters (`asrc_`, `1src_`) are not.
fn org_inline_object_start(text: &str, at: usize) -> bool {
    if at == 0 {
        return true;
    }
    let mut prevs = text[..at].chars().rev();
    let prev = prevs.next().expect("at > 0");
    if prev.is_ascii_alphanumeric() {
        return false;
    }
    if prev == '_' {
        if let Some(before) = prevs.next() {
            if before.is_ascii_alphanumeric() {
                return false;
            }
        }
    }
    true
}

/// org-element `scan-lists` on a one-pair syntax table. Newlines are
/// allowed (org-element 2015). Unmatched opener is not an object.
fn org_scan_lists(text: &str, open_at: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open_at) != Some(&open) {
        return None;
    }
    let mut depth = 1usize;
    let mut i = open_at + 1;
    while i < bytes.len() {
        let b = bytes[i];
        if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

/// `src_LANG` then optional `[parameters]` then required `{body}`.
/// `case-fold-search` is nil; language is `[^ \t\n[{]+`.
fn org_inline_src_span_end(text: &str, at: usize) -> Option<usize> {
    let rest = text.get(at..)?;
    if !rest.starts_with("src_") || !org_inline_object_start(text, at) {
        return None;
    }
    let bytes = text.as_bytes();
    let mut i = at + 4;
    let lang_start = i;
    while i < bytes.len() && !matches!(bytes[i], b' ' | b'\t' | b'\n' | b'[' | b'{') {
        i += 1;
    }
    if i == lang_start {
        return None;
    }
    if i < bytes.len() && bytes[i] == b'[' {
        i = org_scan_lists(text, i, b'[', b']')?;
    }
    if i >= bytes.len() || bytes[i] != b'{' {
        return None;
    }
    org_scan_lists(text, i, b'{', b'}')
}

/// `call_NAME` then optional `[inside-header]`, required `(arguments)`,
/// optional `[end-header]`. Name is `[^ \t\n[(]+`. An unmatched end-header
/// is not part of the object (org-element looking-at).
fn org_inline_call_span_end(text: &str, at: usize) -> Option<usize> {
    let rest = text.get(at..)?;
    if !rest.starts_with("call_") || !org_inline_object_start(text, at) {
        return None;
    }
    let bytes = text.as_bytes();
    let mut i = at + 5;
    let name_start = i;
    while i < bytes.len() && !matches!(bytes[i], b' ' | b'\t' | b'\n' | b'[' | b'(') {
        i += 1;
    }
    if i == name_start {
        return None;
    }
    if i < bytes.len() && bytes[i] == b'[' {
        i = org_scan_lists(text, i, b'[', b']')?;
    }
    if i >= bytes.len() || bytes[i] != b'(' {
        return None;
    }
    i = org_scan_lists(text, i, b'(', b')')?;
    if i < bytes.len() && bytes[i] == b'[' {
        if let Some(end) = org_scan_lists(text, i, b'[', b']') {
            i = end;
        }
    }
    Some(i)
}

/// Org `=`/`~`, Markdown backtick spans, CommonMark `*`/`**`, and GFM `~~`,
/// paired to the real closer.
///
/// Org markers follow the same walk as pandoc's org reader
/// (`verbatimBetween` / `emphasisStart` / `emphasisEnd`, the Emacs
/// `org-emphasis-regexp-components` defaults). The opener sits after a pre
/// character (start of text, whitespace, or `('"{`), the first and last
/// interior characters are not whitespace, and the closer is the first
/// matching marker whose next character is a post character (end of text,
/// whitespace, or `-.,:!?;'")}[`). Inner copies of the marker are content.
/// `pandoc -f org` reports those spans as `Code` inlines with class
/// `verbatim` or bare `Code`.
///
/// Markdown inline code uses CommonMark / pandoc fence-length matching: a
/// run of `n` backticks closes on the next run of exactly `n` backticks, so
/// a double span can hold a single backtick.
///
/// Markdown `*` / `**` use CommonMark flanking (not Org's pre/post classes).
/// GFM `~~strike~~` is an exact two-tilde run.
fn protect_paired_spans(text: &str, placeholders: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        if bytes[i] == b'`' {
            if let Some(end) = find_md_code_span(text, i) {
                push_placeholder(&mut out, placeholders, &text[i..end]);
                i = end;
                continue;
            }
        } else if bytes[i] == b'=' {
            if let Some(end) = find_org_paired_span(text, i, '=') {
                push_placeholder(&mut out, placeholders, &text[i..end]);
                i = end;
                continue;
            }
        } else if bytes[i] == b'~' {
            // GFM `~~strike~~` before Org `~code~` so a double run is not
            // eaten as one org span that happens to close at the last tilde.
            if let Some(end) = find_md_strike_span(text, i) {
                push_placeholder(&mut out, placeholders, &text[i..end]);
                i = end;
                continue;
            }
            if let Some(end) = find_org_paired_span(text, i, '~') {
                push_placeholder(&mut out, placeholders, &text[i..end]);
                i = end;
                continue;
            }
        } else if bytes[i] == b'*' {
            if let Some(end) = find_md_emphasis_span(text, i) {
                push_placeholder(&mut out, placeholders, &text[i..end]);
                i = end;
                continue;
            }
        }
        let ch = text[i..].chars().next().expect("i is in range");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn push_placeholder(out: &mut String, placeholders: &mut Vec<String>, span: &str) {
    let idx = placeholders.len();
    placeholders.push(span.to_string());
    out.push_str(&format!("\x00PH{idx}\x00"));
}

fn find_org_paired_span(text: &str, open_at: usize, marker: char) -> Option<usize> {
    // pandoc org reader / org-emphasis-regexp-components defaults.
    // Border (forbidden at the inner edges) is whitespace.
    const PRE: &str = " \t\n('\"{";
    const POST: &str = " \t\n-.,:!?;'\")}[";

    if open_at > 0 {
        let prev = text[..open_at].chars().next_back()?;
        if !PRE.contains(prev) {
            return None;
        }
    }
    let after_open = open_at + marker.len_utf8();
    if after_open >= text.len() {
        return None;
    }
    let first = text[after_open..].chars().next()?;
    if first.is_whitespace() {
        return None;
    }

    let mut j = after_open;
    while j < text.len() {
        let ch = text[j..].chars().next()?;
        if ch == '\n' {
            return None;
        }
        // Markdown code can hold `=` / `~`; those are not Org closers.
        if ch == '`' {
            if let Some(end) = find_md_code_span(text, j) {
                j = end;
                continue;
            }
        }
        if ch == marker && j > after_open {
            let prev = text[..j].chars().next_back()?;
            if !prev.is_whitespace() {
                let after_close = j + marker.len_utf8();
                let post_ok =
                    after_close == text.len() || POST.contains(text[after_close..].chars().next()?);
                if post_ok {
                    return Some(after_close);
                }
            }
        }
        j += ch.len_utf8();
    }
    None
}

/// CommonMark flanking for `*` / `**` (and longer runs).
///
/// Edges of the text count as whitespace. A run can open when it is
/// left-flanking (and not also right-flanking unless the previous character
/// is punctuation). It closes on the nearest later run of `*` that is
/// right-flanking and satisfies the rule of three: the sum of opener and
/// closer lengths is not a multiple of 3, unless both lengths are.
fn find_md_emphasis_span(text: &str, open_at: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open_at) != Some(&b'*') {
        return None;
    }
    let n = count_ascii_run(bytes, open_at, b'*');
    if n == 0 {
        return None;
    }
    let before = md_edge_char(text, open_at, false);
    let after = md_edge_char(text, open_at + n, true);
    let (left, right) = md_flanking(before, after);
    if !(left && (!right || is_md_punctuation(before))) {
        return None;
    }

    let mut j = open_at + n;
    while j < text.len() {
        let ch = text[j..].chars().next()?;
        if ch == '*' {
            let m = count_ascii_run(bytes, j, b'*');
            let c_before = md_edge_char(text, j, false);
            let c_after = md_edge_char(text, j + m, true);
            let (c_left, c_right) = md_flanking(c_before, c_after);
            let can_close = c_right && (!c_left || is_md_punctuation(c_after));
            let three_ok = ((n + m) % 3 != 0) || (n % 3 == 0);
            if can_close && three_ok && j > open_at + n {
                return Some(j + m);
            }
            j += m;
            continue;
        }
        j += ch.len_utf8();
    }
    None
}

/// GFM strikethrough: a run of exactly two `~` that is not followed by
/// whitespace, closed by the next exact `~~` that is not preceded by
/// whitespace.
fn find_md_strike_span(text: &str, open_at: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open_at) != Some(&b'~') || bytes.get(open_at + 1) != Some(&b'~') {
        return None;
    }
    if bytes.get(open_at + 2) == Some(&b'~') {
        return None;
    }
    let after_open = open_at + 2;
    if after_open >= text.len() {
        return None;
    }
    let first = text[after_open..].chars().next()?;
    if first.is_whitespace() {
        return None;
    }
    let mut j = after_open;
    while j < text.len() {
        let ch = text[j..].chars().next()?;
        if ch == '~'
            && bytes.get(j + 1) == Some(&b'~')
            && bytes.get(j + 2) != Some(&b'~')
            && j > after_open
        {
            let prev = text[..j].chars().next_back()?;
            if !prev.is_whitespace() {
                return Some(j + 2);
            }
        }
        j += ch.len_utf8();
    }
    None
}

fn count_ascii_run(bytes: &[u8], start: usize, marker: u8) -> usize {
    let mut n = 0;
    while start + n < bytes.len() && bytes[start + n] == marker {
        n += 1;
    }
    n
}

fn md_edge_char(text: &str, byte: usize, after: bool) -> char {
    if after {
        if byte >= text.len() {
            '\n'
        } else {
            text[byte..].chars().next().unwrap_or('\n')
        }
    } else if byte == 0 {
        '\n'
    } else {
        text[..byte].chars().next_back().unwrap_or('\n')
    }
}

fn is_md_punctuation(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_punctuation()
    } else {
        !c.is_alphanumeric() && !c.is_whitespace()
    }
}

fn md_flanking(before: char, after: char) -> (bool, bool) {
    let after_ws = after.is_whitespace();
    let before_ws = before.is_whitespace();
    let after_p = is_md_punctuation(after);
    let before_p = is_md_punctuation(before);
    let left = !after_ws && (!after_p || before_ws || before_p);
    let right = !before_ws && (!before_p || after_ws || after_p);
    (left, right)
}

fn find_md_code_span(text: &str, open_at: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open_at) != Some(&b'`') {
        return None;
    }
    let mut n = 0usize;
    while open_at + n < bytes.len() && bytes[open_at + n] == b'`' {
        n += 1;
    }
    let mut j = open_at + n;
    while j < bytes.len() {
        if bytes[j] == b'\n' {
            return None;
        }
        if bytes[j] == b'`' {
            let mut m = 0usize;
            while j + m < bytes.len() && bytes[j + m] == b'`' {
                m += 1;
            }
            if m == n && j > open_at + n {
                return Some(j + m);
            }
            j += m;
        } else {
            j += 1;
        }
    }
    None
}

/// Byte ranges of inline tokens that wrapping must not split (links, images,
/// reference links `[text][ref]`, inline code, autolinks, math, Org `[[...]]`,
/// Org `[cite...]`, Org `[fn::…]` / `[fn:LABEL:…]`, Org `<<<...>>>` /
/// `<<...>>`, Org `{{{name}}}` / `{{{name(args)}}}`, Org `src_lang{...}` /
/// `call_name(...)`, RST `|fig. 1|` / `|name|_` / `|name|__`, paired spans).
///
/// Ranges are half-open `[start, end)`, sorted, non-overlapping, and merged
/// when a regex match wraps a paired span.
pub fn atomic_inline_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut org_src_call_spans = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        if let Some(end) = rst_substitution_ref_span_end(text, i) {
            spans.push((i, end));
            i = end;
            continue;
        }
        if let Some(end) = org_inline_src_or_call_span_end(text, i) {
            spans.push((i, end));
            org_src_call_spans.push((i, end));
            i = end;
            continue;
        }
        if let Some(end) = org_footnote_reference_span_end(text, i) {
            spans.push((i, end));
            i = end;
            continue;
        }
        if bytes[i] == b'`' {
            if let Some(end) = find_md_code_span(text, i) {
                spans.push((i, end));
                i = end;
                continue;
            }
        } else if bytes[i] == b'=' || bytes[i] == b'~' {
            let marker = bytes[i] as char;
            if let Some(end) = find_org_paired_span(text, i, marker) {
                spans.push((i, end));
                i = end;
                continue;
            }
        }
        let ch = text[i..].chars().next().expect("i is in range");
        i += ch.len_utf8();
    }
    for m in INLINE_TOKEN_RE.find_iter(text) {
        let (ms, me) = (m.start(), m.end());
        // Org `\<src_` / `\<call_` wins over the underline regex `_src_` /
        // `_call_` when `_src_python{...}` is an object (BOL / after
        // whitespace). Keep a regex match only when it wraps the whole
        // object (a link around the src).
        let steals_org = org_src_call_spans
            .iter()
            .any(|&(s, e)| ms < e && me > s && !(ms <= s && me >= e));
        if steals_org {
            continue;
        }
        spans.push((ms, me));
    }
    merge_byte_ranges(spans)
}

fn merge_byte_ranges(mut spans: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if spans.len() <= 1 {
        return spans;
    }
    spans.sort_unstable_by_key(|&(start, _)| start);
    let mut out = Vec::with_capacity(spans.len());
    let mut cur = spans[0];
    for &(start, end) in &spans[1..] {
        if start <= cur.1 {
            cur.1 = cur.1.max(end);
        } else {
            out.push(cur);
            cur = (start, end);
        }
    }
    out.push(cur);
    out
}

/// Restore placeholders produced by [`protect_inline_tokens`] into each segment.
///
/// Later placeholders can wrap earlier ones (the regex pass runs after the
/// paired-span walk and may match a markdown link that already contains
/// `\x00PHn\x00`). Restore from the last index first so an outer wrapper
/// expands before its inner tokens.
pub fn restore_inline_tokens(segments: Vec<String>, placeholders: &[String]) -> Vec<String> {
    segments
        .into_iter()
        .map(|s| {
            let mut restored = s.trim().to_string();
            for (i, original) in placeholders.iter().enumerate().rev() {
                let ph = format!("\x00PH{i}\x00");
                restored = restored.replace(&ph, original);
            }
            restored
        })
        .filter(|s| !s.is_empty())
        .collect()
}

impl SentenceSplitter for UnicodeSentenceSplitter {
    fn split(&self, text: &str) -> Vec<String> {
        let text = text.trim();
        if text.is_empty() {
            return vec![];
        }

        let (protected, placeholders) =
            protect_inline_tokens_with(text, &self.extra_verbatim_commands);

        // UAX #29 sentence bounds. `unicode_sentences()` filters
        // whitespace-only segments but also drops trailing closing
        // punctuation like `>` after a sentence-terminating `.`, which
        // clips inputs such as `Vec<...>` or `<a.>` at end-of-prose.
        // We re-collect from the unfiltered iterator and merge any
        // non-sentence tail back onto the preceding sentence.
        let raw_segments: Vec<&str> = merge_tail_punctuation(&protected);

        if raw_segments.is_empty() {
            return vec![text.to_string()];
        }

        // UAX SB8 will not break after ATerm when the next letter is
        // lowercase. Split same-line `iCloud` starts before abbreviation
        // merge so `e.g. iCloud` can rejoin.
        let expanded = split_before_lowercase_proper_nouns(raw_segments.iter().copied());
        let refs: Vec<&str> = expanded.iter().map(String::as_str).collect();
        let merged = self.refine_segments_from_strs(&refs);
        let restored = restore_inline_tokens(merged, &placeholders);
        split_after_markup_sentence_end(restored)
    }
}

impl UnicodeSentenceSplitter {
    /// Apply abbreviation + delimiter-span merges to an already-segmented list.
    ///
    /// Used by the neural backend so `--neural` shares the same post-pipeline
    /// guarantees (dialogue quotes, `Dr.`, balanced spans) as the UAX path.
    pub fn refine_segments(&self, segments: Vec<String>) -> Vec<String> {
        if segments.is_empty() {
            return segments;
        }
        let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
        self.refine_segments_from_strs(&refs)
    }

    fn refine_segments_from_strs(&self, raw_segments: &[&str]) -> Vec<String> {
        let merged = merge_abbreviation_splits(
            raw_segments,
            &self.lang_abbrev_pattern,
            &self.lang_multi_pattern,
            self.extra_pattern.as_ref(),
        );
        let merged = merge_quoted_punct_splits(merged);
        merge_splits_inside_delimiters(merged)
    }
}

/// Walk the UAX #29 sentence bounds and merge any trailing non-sentence
/// segments back onto the preceding sentence. Without this glue, a prose
/// region ending in characters like `>` after a sentence-terminating `.`
/// would lose those characters: `Vec<...>` becomes `Vec<...`. The standard
/// `unicode_sentences()` filter silently discards such tails because they
/// contain no letter/digit/quote.
///
/// We never *split* further than the bounds iterator does; we only merge
/// adjacent fragments where one is a real sentence and its neighbour is
/// content-free (no alphanumeric characters). This mirrors the existing
/// `unicode_sentences()` filter rule but reattaches the tail rather than
/// dropping it.
fn merge_tail_punctuation(text: &str) -> Vec<&str> {
    use unicode_segmentation::UnicodeSegmentation;

    fn has_content(s: &str) -> bool {
        s.chars().any(|c| c.is_alphanumeric())
    }

    let bounds: Vec<&str> = text.split_sentence_bounds().collect();
    if bounds.is_empty() {
        return Vec::new();
    }

    // Build a merged Vec<&str> by walking left to right and re-slicing the
    // original `text` so we return `&str`s. The slice boundaries align
    // because `split_sentence_bounds` returns adjacent subslices.
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(bounds.len());
    let mut cursor: usize = 0;
    for seg in &bounds {
        let start = cursor;
        let end = cursor + seg.len();
        if has_content(seg) {
            merged.push((start, end));
        } else if let Some(last) = merged.last_mut() {
            // Glue onto the previous sentence.
            last.1 = end;
        } else {
            // Leading whitespace/punctuation only: preserve as a segment;
            // the downstream pipeline trims it.
            merged.push((start, end));
        }
        cursor = end;
    }

    merged.into_iter().map(|(s, e)| &text[s..e]).collect()
}

/// UAX SB8 refuses a break after ATerm when the next letter is lowercase.
/// A lowercase-starting proper noun (`iCloud`, `eBay`) is still a new
/// sentence. Split after `.!?` plus whitespace when the next token starts
/// lowercase and contains a later uppercase letter. Inline spans stay
/// protected, so this must run before placeholder restore.
fn split_before_lowercase_proper_nouns<'a, I>(segments: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut out = Vec::new();
    for seg in segments {
        push_lowercase_proper_noun_splits(&mut out, seg);
    }
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

fn push_lowercase_proper_noun_splits(out: &mut Vec<String>, seg: &str) {
    // Match on the trimmed view so leading/trailing wrap space does not
    // hide `iCloud`. Keep the original bytes when there is no split so
    // abbreviation merge still sees UAX trailing space before `~` / `$`.
    if let Some((head, rest)) = take_lowercase_proper_noun_sentence(seg.trim()) {
        out.push(head);
        push_lowercase_proper_noun_splits(out, &rest);
    } else if !seg.is_empty() {
        out.push(seg.to_string());
    }
}

fn take_lowercase_proper_noun_sentence(seg: &str) -> Option<(String, String)> {
    static CAP: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)^(.*?[.!?])\s+([a-z]+[A-Z][A-Za-z0-9]*[\s\S]*)$")
            .expect("valid lowercase-proper-noun sentence regex")
    });
    let c = CAP.captures(seg)?;
    let head = c.get(1)?.as_str().trim();
    let rest = c.get(2)?.as_str().trim();
    if head.is_empty() || rest.is_empty() {
        return None;
    }
    Some((head.to_string(), rest.to_string()))
}

fn merge_abbreviation_splits(
    segments: &[&str],
    abbrev_re: &Regex,
    multi_re: &Regex,
    extra: Option<&Regex>,
) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(segments.len());

    for &segment in segments {
        let should_merge = if let Some(prev) = result.last() {
            is_abbreviation_ending(prev, abbrev_re, multi_re, extra)
        } else {
            false
        };

        if should_merge {
            let prev = result.last_mut().unwrap();
            push_segment_preserving_space(prev, segment);
        } else {
            result.push(segment.to_string());
        }
    }

    result
}

/// Append `piece` to `dest`, inserting a single space if neural/UAX segments
/// were trimmed and would otherwise glue `world.` + `How` into `world.How`.
///
/// Do not invent a space before a mark that was attached to the period in
/// the source. LaTeX `Eq.~\ref{}` uses `~` as a non-breaking space. Org
/// `~code~` pairing does not close before `\`, so abbreviation merge sees
/// `Eq.` + `~\ref` as two segments.
fn push_segment_preserving_space(dest: &mut String, piece: &str) {
    if piece.is_empty() {
        return;
    }
    let next = piece.chars().next();
    let need_space = dest.chars().last().is_some_and(|c| !c.is_whitespace())
        && next.is_some_and(|c| {
            !c.is_whitespace() && (c.is_alphanumeric() || matches!(c, '"' | '\'' | '`' | '('))
        });
    if need_space {
        dest.push(' ');
    }
    dest.push_str(piece);
}

/// Merge false splits caused by sentence punctuation inside quotes or parens.
/// E.g., `He said "wow!"` + `and left.` should stay as one sentence when
/// the next segment starts with a lowercase letter.
/// Split after `.!?` next to Markdown/Org closers, or after an RST
/// interpreted-text closer followed by `.!?`.
///
/// `**Bold sentence.** Next` is one UAX sentence because the period lives
/// inside the protected span. RST prefix `:role:`text`. puts the period
/// after the backtick; suffix `text`:role:. puts it after `:role:`.
/// `file:\S+` can also swallow the period on suffix `:file:.`, so UAX
/// sees one sentence. This post-pass runs after restore. Quotes are not
/// paired spans, so they already split. Mid-span periods (`**the end. Still bold**`)
/// have no closers after the period and stay one sentence.
pub(crate) fn split_after_markup_sentence_end(segments: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for seg in segments {
        push_markup_sentence_splits(&mut out, seg.trim());
    }
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

fn push_markup_sentence_splits(out: &mut Vec<String>, seg: &str) {
    if let Some((head, rest)) = take_markup_terminal_sentence(seg) {
        out.push(head);
        push_markup_sentence_splits(out, &rest);
    } else if !seg.is_empty() {
        out.push(seg.to_string());
    }
}

fn take_markup_terminal_sentence(seg: &str) -> Option<(String, String)> {
    // Terminal `.!?` immediately before `**` / `*` / `_` / backticks /
    // `~~` / `](url)`, or an RST interpreted-text closer then `.!?`,
    // then whitespace, then a new sentence (uppercase or opening quote).
    // Prefix closer is `:role:`text`. / `:domain:role:`text`. Suffix
    // closer is `text`:role:. / `text`:domain:role:. (the closer is
    // `:role:`, not the backtick). Opening ```` is not a closer. Role
    // charset has no `.` so `email:user.name:`host`. does not
    // false-split. A lowercase continuation (`[Example Inc.](url) now.`)
    // stays one sentence.
    static CAP: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?s)^(.*?(?:[.!?](?:\*{1,3}|_{1,3}|`+|~{1,2}|\]\([^)]*\))+|:[A-Za-z][A-Za-z0-9_-]*(?::[A-Za-z][A-Za-z0-9_-]*)*:`[^`\n]+`[.!?]|`[^`\n]+`:[A-Za-z][A-Za-z0-9_-]*(?::[A-Za-z][A-Za-z0-9_-]*)*:[.!?]))\s+([A-Z][\s\S]*|["'][A-Z][\s\S]*)$"#,
        )
        .expect("valid markup-terminal sentence regex")
    });
    let c = CAP.captures(seg)?;
    let head = c.get(1)?.as_str().trim();
    let rest = c.get(2)?.as_str().trim();
    if head.is_empty() || rest.is_empty() {
        return None;
    }
    Some((head.to_string(), rest.to_string()))
}

fn merge_quoted_punct_splits(segments: Vec<String>) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(segments.len());

    for segment in segments {
        let should_merge = if let Some(prev) = result.last() {
            // Previous segment ends with punctuation + closing quote/paren
            QUOTED_PUNCT_END_RE.is_match(prev.trim_end())
                // Next segment starts with lowercase (continuation, not new sentence)
                && segment
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_lowercase())
        } else {
            false
        };

        if should_merge {
            let prev = result.last_mut().unwrap();
            push_segment_preserving_space(prev, &segment);
        } else {
            result.push(segment);
        }
    }

    result
}

/// Rejoin UAX segments while any “span” is still open: ASCII/curly/guillemet
/// quotes (including dialogue single quotes with apostrophe heuristics),
/// LaTeX ```` / `''` style quotes, and balanced `()` / `[]` / `{}`.
/// Escaped `\"` / `\'` do not toggle quote state.
fn merge_splits_inside_delimiters(segments: Vec<String>) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(segments.len());
    let mut state = DelimState::default();

    for segment in segments {
        if state.is_inside() {
            if let Some(last) = result.last_mut() {
                push_segment_preserving_space(last, &segment);
            } else {
                result.push(segment.clone());
            }
        } else {
            result.push(segment.clone());
        }
        state.feed(&segment);
    }

    result
}

/// Tracks delimiter nesting for span-aware sentence merging and invariants.
/// Public to tests so property checks can share the exact production logic.
#[derive(Debug, Default, Clone)]
pub struct DelimState {
    ascii_double_open: bool,
    /// Dialogue-style ASCII single quotes (`'Hello.'`), not apostrophes.
    ascii_single_open: bool,
    curly_double_depth: i32,
    curly_single_depth: i32,
    guillemet_depth: i32,
    latex_quote_depth: i32,
    paren_depth: i32,
    bracket_depth: i32,
    brace_depth: i32,
    /// Last character fed (survives chunk boundaries for apostrophe heuristics).
    last_char: Option<char>,
    /// When the previous chunk ended in `\`, the next `"` / `'` is escaped.
    pending_escape: bool,
}

impl DelimState {
    pub fn is_inside(&self) -> bool {
        self.ascii_double_open
            || self.ascii_single_open
            || self.curly_double_depth > 0
            || self.curly_single_depth > 0
            || self.guillemet_depth > 0
            || self.latex_quote_depth > 0
            || self.paren_depth > 0
            || self.bracket_depth > 0
            || self.brace_depth > 0
    }

    /// Feed `text` and update nesting. Used both in the splitter merge pass
    /// and in regression/property tests that assert formatted output never
    /// places a newline while still inside a span.
    pub fn feed(&mut self, text: &str) {
        // Walk by char index without allocating a `Vec<char>` per call (hot
        // path: every segment in merge_splits_inside_delimiters + tests).
        let mut iter = text.chars().peekable();
        while let Some(ch) = iter.next() {
            let prev = self.last_char;
            let next = iter.peek().copied();

            if self.pending_escape {
                self.pending_escape = false;
                self.last_char = Some(ch);
                continue;
            }

            // LaTeX-style open `` and close '' (must run before single `'`).
            // Markdown fences use ``` — treat runs of 3+ backticks as neutral
            // so we do not leave latex_quote_depth stuck open across lines.
            if ch == '`' && next == Some('`') {
                let _ = iter.next(); // second `
                if iter.peek() == Some(&'`') {
                    while iter.peek() == Some(&'`') {
                        let _ = iter.next();
                    }
                    self.last_char = Some('`');
                    continue;
                }
                self.latex_quote_depth += 1;
                self.last_char = Some('`');
                continue;
            }
            if ch == '\'' && next == Some('\'') {
                let _ = iter.next();
                self.latex_quote_depth = (self.latex_quote_depth - 1).max(0);
                self.last_char = Some('\'');
                continue;
            }

            // Escaped ASCII quotes do not toggle (may span chunk boundary).
            if ch == '\\' && matches!(next, Some('"') | Some('\'')) {
                self.last_char = iter.next();
                continue;
            }
            if ch == '\\' && next.is_none() {
                self.pending_escape = true;
                self.last_char = Some('\\');
                continue;
            }

            match ch {
                '"' => self.ascii_double_open = !self.ascii_double_open,
                '\'' => self.feed_ascii_single(prev, next),
                // Curly doubles “ ”
                '\u{201C}' => self.curly_double_depth += 1,
                '\u{201D}' => self.curly_double_depth = (self.curly_double_depth - 1).max(0),
                // Curly singles ‘ ’
                '\u{2018}' => self.curly_single_depth += 1,
                '\u{2019}' => {
                    // U+2019 is also a common apostrophe; only close when open,
                    // otherwise ignore (it's / don't).
                    if self.curly_single_depth > 0 {
                        self.curly_single_depth -= 1;
                    }
                }
                '\u{00AB}' => self.guillemet_depth += 1,
                '\u{00BB}' => self.guillemet_depth = (self.guillemet_depth - 1).max(0),
                '(' => self.paren_depth += 1,
                ')' => self.paren_depth = (self.paren_depth - 1).max(0),
                '[' => self.bracket_depth += 1,
                ']' => self.bracket_depth = (self.bracket_depth - 1).max(0),
                '{' if prev != Some('\\') => self.brace_depth += 1,
                '}' if prev != Some('\\') => {
                    self.brace_depth = (self.brace_depth - 1).max(0);
                }
                _ => {}
            }
            self.last_char = Some(ch);
        }
    }

    /// ASCII `'` is ambiguous (dialogue vs apostrophe). Open only in opener
    /// context; never toggle on in-word apostrophes (`don't`, `it's`).
    fn feed_ascii_single(&mut self, prev: Option<char>, next: Option<char>) {
        let prev_alnum = prev.is_some_and(|c| c.is_alphanumeric());
        let next_alnum = next.is_some_and(|c| c.is_alphanumeric());
        // Classic apostrophe: letter/digit on both sides.
        if prev_alnum && next_alnum {
            return;
        }
        if self.ascii_single_open {
            // Prefer close; trailing possessive `papers'` has prev alnum and
            // no next alnum — treat as close if we were open, else ignore.
            self.ascii_single_open = false;
            return;
        }
        // Open only at dialogue-like boundaries.
        let opener = match prev {
            None => true,
            Some(c) if c.is_whitespace() => true,
            Some('(' | '[' | '{' | '"' | '\u{201C}' | '\u{00AB}') => true,
            Some('.' | '!' | '?' | ':' | ';' | ',') => true,
            _ => false,
        };
        if opener {
            self.ascii_single_open = true;
        }
    }
}

/// Return `true` if `formatted` never inserts a **mid-document** line break
/// while a delimiter span tracked by [`DelimState`] is still open.
///
/// A trailing final `\n` (POSIX text) is ignored even if a span is still open
/// (unbalanced input like a lone `{`). Any earlier `\n` while `is_inside()`
/// is rejected.
///
/// Inline code / links / emphasis are stripped via [`protect_inline_tokens`]
/// first so brackets inside `` `[` `` do not count as real spans (same as the
/// production splitter path).
///
/// Implementation feeds whole lines (not per-char) so apostrophe heuristics
/// see real `prev`/`next` neighbors; fails when a prior line left a span open.
pub fn newlines_respect_delimiter_spans(formatted: &str) -> bool {
    let trimmed_end = formatted.trim_end_matches('\n');
    if trimmed_end.is_empty() {
        return true;
    }
    let (protected, _) = protect_inline_tokens(trimmed_end);
    let mut state = DelimState::default();
    for line in protected.split('\n') {
        if state.is_inside() {
            return false;
        }
        state.feed(line);
    }
    true
}

fn is_abbreviation_ending(
    s: &str,
    abbrev_re: &Regex,
    multi_re: &Regex,
    extra: Option<&Regex>,
) -> bool {
    let trimmed = s.trim_end();
    if !trimmed.ends_with('.') {
        return false;
    }
    let before_dot = &trimmed[..trimmed.len() - 1];

    if abbrev_re.is_match(before_dot) {
        return true;
    }

    if multi_re.is_match(before_dot) {
        return true;
    }

    if let Some(re) = extra {
        if re.is_match(before_dot) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(text: &str) -> Vec<String> {
        UnicodeSentenceSplitter::new().split(text)
    }

    #[test]
    fn simple_sentences() {
        assert_eq!(
            split("Hello world. This is a test. Another sentence here."),
            vec!["Hello world.", "This is a test.", "Another sentence here."]
        );
    }

    #[test]
    fn abbreviation_dr() {
        assert_eq!(
            split("Dr. Smith went home. He was tired."),
            vec!["Dr. Smith went home.", "He was tired."]
        );
    }

    #[test]
    fn abbreviation_eg() {
        assert_eq!(
            split("Use a formatter, e.g. snapper. It works well."),
            vec!["Use a formatter, e.g. snapper.", "It works well."]
        );
    }

    #[test]
    fn abbreviation_fig() {
        assert_eq!(
            split("See Fig. 3 for details. The results are clear."),
            vec!["See Fig. 3 for details.", "The results are clear."]
        );
    }

    #[test]
    fn placeholder_restore_survives_regex_wrapping_backticks() {
        // Pathological backtick salad from proptest: the regex pass can wrap
        // a paired-span placeholder in a `[...](...)` match. Restore must
        // expand the outer token first or `\x00PHn\x00` leaks into output.
        let input = "`0`[`0``a`` `{``A`](`a` `)";
        let out = split(input);
        let joined = out.join("\n");
        assert!(!joined.contains('\u{0}'), "placeholder leaked: {joined:?}");
        let again = split(&joined);
        assert_eq!(again, out);
    }

    #[test]
    fn wrt_abbreviation_does_not_split() {
        assert_eq!(
            split("Computed w.r.t. $x$. Next."),
            vec!["Computed w.r.t. $x$.".to_string(), "Next.".to_string()]
        );
    }

    #[test]
    fn latex_inline_math_parens_stay_atomic() {
        assert_eq!(
            split(r"According to X, \(E=mc^2\). Next."),
            vec![
                r"According to X, \(E=mc^2\).".to_string(),
                "Next.".to_string(),
            ]
        );
    }

    #[test]
    fn latex_verb_inner_punct_stays_atomic() {
        let text = r"Use \verb|a.b! c| here. Next.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == r"\verb|a.b! c|"),
            "verb span must be protected, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![r"Use \verb|a.b! c| here.".to_string(), "Next.".to_string()]
        );
    }

    #[test]
    fn extra_verbatim_command_is_tokenized_like_verb() {
        let text = r"Use \Verb|a.b! c| here. Next.";
        let extras = ["Verb".to_string()];
        let (_, placeholders) = protect_inline_tokens_with(text, &extras);
        assert!(
            placeholders.iter().any(|p| p == r"\Verb|a.b! c|"),
            "extra Verb span must be protected, got {placeholders:?}"
        );
        assert!(
            !protect_inline_tokens(text)
                .1
                .iter()
                .any(|p| p == r"\Verb|a.b! c|"),
            "unlisted Verb must not be protected"
        );
    }

    #[test]
    fn extra_verb_does_not_steal_verbatim() {
        let extras = ["Verb".to_string()];
        assert_eq!(
            latex_verb_span_end_with(r"\Verbatim|x.y|", 0, &extras),
            None,
            "Verb must not match as a prefix of Verbatim"
        );
        assert_eq!(
            latex_verb_span_end_with(r"\Verb|x.y|", 0, &extras),
            Some(r"\Verb|x.y|".len())
        );
        let text = r"Use \Verbatim|x.y| here. Next.";
        let (_, placeholders) = protect_inline_tokens_with(text, &extras);
        assert!(
            placeholders.iter().all(|p| p != r"\Verbatim|x.y|"),
            "Verbatim must not become a verb span, got {placeholders:?}"
        );
        assert_eq!(
            UnicodeSentenceSplitter::new()
                .with_verbatim_commands(extras.to_vec())
                .split(text),
            vec![r"Use \Verbatim|x.y| here.".to_string(), "Next.".to_string()]
        );
    }

    #[test]
    fn latex_lstinline_inner_percent_stays_atomic() {
        let text = r"Code \lstinline!%! here. Next.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == r"\lstinline!%!"),
            "lstinline span must be protected, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![r"Code \lstinline!%! here.".to_string(), "Next.".to_string()]
        );
    }

    #[test]
    fn latex_lstinline_optional_args_stay_atomic() {
        let text = r"See \lstinline[language=TeX]!a.b%! please. Next.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders
                .iter()
                .any(|p| p == r"\lstinline[language=TeX]!a.b%!"),
            "lstinline with optional args must be protected, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                r"See \lstinline[language=TeX]!a.b%! please.".to_string(),
                "Next.".to_string()
            ]
        );
    }

    #[test]
    fn unmatched_latex_verb_extends_to_eol() {
        let text = r"See \verb|a%b. Next";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == r"\verb|a%b. Next"),
            "unmatched verb must run to EOL, got {placeholders:?}"
        );
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn latex_nbsp_after_abbrev_stays_attached() {
        // `Eq.~\ref{}` is one token in LaTeX. Org `~code~` pairing does
        // not take a closer before `\`, so abbreviation merge must not
        // insert a space between `Eq.` and `~`.
        let text = r"See Fig. ~1, Eq.~\ref{eq:diff}, and Dr. Smith. Next.";
        assert_eq!(
            split(text),
            vec![
                r"See Fig. ~1, Eq.~\ref{eq:diff}, and Dr. Smith.".to_string(),
                "Next.".to_string(),
            ]
        );
    }

    #[test]
    fn empty_input() {
        assert_eq!(split(""), Vec::<String>::new());
    }

    #[test]
    fn single_sentence() {
        assert_eq!(split("Just one sentence."), vec!["Just one sentence."]);
    }

    #[test]
    fn question_and_exclamation() {
        assert_eq!(
            split("Is this working? Yes! It is."),
            vec!["Is this working?", "Yes!", "It is."]
        );
    }

    #[test]
    fn no_trailing_period() {
        assert_eq!(
            split("First sentence. Second without period"),
            vec!["First sentence.", "Second without period"]
        );
    }

    #[test]
    fn extra_abbreviations() {
        // "Abstr" is not a built-in abbreviation, so the default splitter
        // would break at "Abstr." The extra list prevents that.
        let splitter = UnicodeSentenceSplitter::with_extra_abbreviations(&[
            "Abstr".to_string(),
            "Suppl".to_string(),
        ]);
        assert_eq!(
            splitter.split("See Abstr. 5 for details. The results follow."),
            vec!["See Abstr. 5 for details.", "The results follow."]
        );
        // Without extra, "Abstr." would cause a false break:
        let default = UnicodeSentenceSplitter::new();
        let result = default.split("See Abstr. 5 for details. The results follow.");
        // Default splits at "Abstr." since it doesn't know the abbreviation
        assert!(result.len() > 1);
    }

    #[test]
    fn inline_org_link_preserved() {
        assert_eq!(
            split("See [[https://example.com][Ex. Site]] for details. Then continue."),
            vec![
                "See [[https://example.com][Ex. Site]] for details.",
                "Then continue."
            ]
        );
    }

    #[test]
    fn inline_org_radio_target_interior_punct_is_not_a_sentence_boundary() {
        // GitHub #211 / snapper-gz10: org-element radio targets
        // `<<<contents>>>` stay one token so an interior period is not
        // a sentence boundary. `Next sentence.` still splits.
        let radio = "<<<the Fourier. transform>>>";
        let text = "See <<<the Fourier. transform>>> in the text. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == radio),
            "radio target must be one token, got {placeholders:?}"
        );
        assert!(
            !placeholders
                .iter()
                .any(|p| p == "<<the Fourier. transform>>"),
            "radio must not collapse to the inner angle target, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == radio),
            "radio target must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "See <<<the Fourier. transform>>> in the text.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_org_angle_target_interior_punct_is_not_a_sentence_boundary() {
        // Same class as radio: org-element-target-parser `<<sec. intro>>`.
        let target = "<<sec. intro>>";
        let text = "See <<sec. intro>> in the text. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == target),
            "angle target must be one token, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == target),
            "angle target must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "See <<sec. intro>> in the text.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_org_macro_interior_punct_is_not_a_sentence_boundary() {
        // GitHub #212 / snapper-aunh: org-element-macro-parser
        // {{{name}}} / {{{name(args)}}} stay one token so an interior
        // period is not a sentence boundary. `Next sentence.` still splits.
        let mac = "{{{cite(Smith. 2020)}}}";
        let text = "See {{{cite(Smith. 2020)}}} for the source. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == mac),
            "org macro must be one token, got {placeholders:?}"
        );
        assert!(
            !placeholders.iter().any(|p| p == "{{cite(Smith. 2020)}}"),
            "two-brace form is not a macro, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == mac),
            "macro must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "See {{{cite(Smith. 2020)}}} for the source.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_org_macro_without_args_is_one_token() {
        let mac = "{{{title}}}";
        let text = "See {{{title}}} for the source. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == mac),
            "no-arg macro must be one token, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                "See {{{title}}} for the source.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn two_brace_form_is_not_an_org_macro() {
        let text = "See {{cite(Smith. 2020)}} for the source. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            !placeholders
                .iter()
                .any(|p| p.contains("{{cite") || p.contains("Smith. 2020")),
            "two-brace {{...}} must not be a macro token, got {placeholders:?}"
        );
    }

    #[test]
    fn unclosed_org_macro_is_not_an_inline_token() {
        let text = "See {{{cite(Smith. 2020 for the source. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            !placeholders.iter().any(|p| p.contains("{{{cite")),
            "unclosed macro must not swallow the sentence, got {placeholders:?}"
        );
    }

    #[test]
    fn inline_org_macro_args_may_span_lines() {
        // org-element-macro-parser args are `(?:.|\n)*?`.
        let mac = "{{{cite(Smith.\n2020)}}}";
        let text = "See {{{cite(Smith.\n2020)}}} for the source. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == mac),
            "multiline-arg macro must be one token, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                "See {{{cite(Smith.\n2020)}}} for the source.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_org_src_interior_punct_is_not_a_sentence_boundary() {
        // GitHub #214 / snapper-pdtw: org-element-inline-src-block-parser
        // `src_lang{...}` stays one token. `Next sentence.` still splits.
        let src = "src_python{print(1. 2)}";
        let text = "Use src_python{print(1. 2)} today. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == src),
            "inline src must be one token, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == src),
            "inline src must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "Use src_python{print(1. 2)} today.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_org_src_headers_and_nested_braces_stay_one_token() {
        let src = "src_python[:results output]{d = {1. 2}}";
        let text = "Use src_python[:results output]{d = {1. 2}} today. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == src),
            "header + nested braces must stay one token, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                "Use src_python[:results output]{d = {1. 2}} today.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_org_call_interior_punct_is_not_a_sentence_boundary() {
        // Same class: org-element-inline-babel-call-parser `call_name(...)`.
        let call = "call_name(1. 2)";
        let text = "Use call_name(1. 2) today. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == call),
            "inline babel call must be one token, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == call),
            "inline babel call must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "Use call_name(1. 2) today.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_org_call_headers_and_nested_parens_stay_one_token() {
        let call = "call_name[:results output](f(1. 2))[:results html]";
        let text = "Use call_name[:results output](f(1. 2))[:results html] today. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == call),
            "call headers + nested parens must stay one token, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                "Use call_name[:results output](f(1. 2))[:results html] today.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn ordinary_call_parens_are_not_an_inline_org_call() {
        // `call_name(...)` is the babel object. Bare `print(1. 2)` is prose
        // and must not become an atomic wrap span.
        let text = "Use print(1. 2) today. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            !placeholders.iter().any(|p| p.contains("print(1. 2)")),
            "ordinary parens must not be an inline call, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            !spans
                .iter()
                .any(|&(s, e)| text[s..e].contains("print(1. 2)")),
            "ordinary parens must not be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        let parts = split(text);
        assert_eq!(parts.last().map(String::as_str), Some("Next sentence."));
        assert!(
            parts.iter().any(|p| p.contains("today.")),
            "prose after the parens must stay in the first sentence, got {parts:?}"
        );
    }

    #[test]
    fn inline_org_src_matches_after_leading_underscore() {
        // `_src_` at BOL / after space is `\<src_`. Hyphen is not a
        // subscript opener here, so `foo-src_python` is an object.
        let src = "src_python{print(1. 2)}";
        for (text, first) in [
            (
                "See _src_python{print(1. 2)} today. Next sentence.",
                "See _src_python{print(1. 2)} today.",
            ),
            (
                "See foo-src_python{print(1. 2)} today. Next sentence.",
                "See foo-src_python{print(1. 2)} today.",
            ),
        ] {
            let (_, placeholders) = protect_inline_tokens(text);
            assert!(
                placeholders.iter().any(|p| p == src),
                "src_ after leading _ or hyphen must be one token, got {placeholders:?} for {text:?}"
            );
            let spans = atomic_inline_spans(text);
            assert!(
                spans.iter().any(|&(s, e)| &text[s..e] == src),
                "src_ after leading _ or hyphen must be an atomic wrap span, got {:?} for {text:?}",
                spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
            );
            assert_eq!(
                split(text),
                vec![first.to_string(), "Next sentence.".to_string()]
            );
        }
    }

    #[test]
    fn inline_org_src_after_word_underscore_is_subscript() {
        // org-element--object-regexp matches the subscript first, so
        // `foo_src_` / `x_src_` are not inline-src-block. Leftover braces
        // stay prose.
        let src = "src_python{print(1. 2)}";
        for text in [
            "See foo_src_python{print(1. 2)} today. Next sentence.",
            "See x_src_python{print(1. 2)} today. Next sentence.",
        ] {
            let (_, placeholders) = protect_inline_tokens(text);
            assert!(
                !placeholders.iter().any(|p| p.contains(src) || p == src),
                "src_ after word-underscore is a subscript, got {placeholders:?} for {text:?}"
            );
            let spans = atomic_inline_spans(text);
            assert!(
                !spans.iter().any(|&(s, e)| text[s..e].contains(src)),
                "src_ after word-underscore must not be an atomic wrap span, got {:?} for {text:?}",
                spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
            );
            let parts = split(text);
            assert!(
                parts.iter().any(|p| p.contains("Next sentence.")),
                "Next sentence. still splits, got {parts:?} for {text:?}"
            );
        }
    }

    #[test]
    fn inline_org_src_requires_word_boundary() {
        // Word char before `s` is not `\<`. `case-fold-search` is nil.
        for text in [
            "asrc_python{print(1. 2)} today. Next sentence.",
            "1src_python{print(1. 2)} today. Next sentence.",
            "SRC_python{print(1. 2)} today. Next sentence.",
        ] {
            let (_, placeholders) = protect_inline_tokens(text);
            assert!(
                !placeholders
                    .iter()
                    .any(|p| p.contains("src_python{print(1. 2)}")
                        || p.contains("SRC_python{print(1. 2)}")),
                "src_ must not match after a word char or when folded, got {placeholders:?} for {text:?}"
            );
        }
    }

    #[test]
    fn inline_org_call_matches_after_leading_underscore() {
        let call = "call_name(1. 2)";
        for (text, first) in [
            (
                "See _call_name(1. 2) today. Next sentence.",
                "See _call_name(1. 2) today.",
            ),
            (
                "See foo-call_name(1. 2) today. Next sentence.",
                "See foo-call_name(1. 2) today.",
            ),
        ] {
            let (_, placeholders) = protect_inline_tokens(text);
            assert!(
                placeholders.iter().any(|p| p == call),
                "call_ after leading _ or hyphen must be one token, got {placeholders:?} for {text:?}"
            );
            let spans = atomic_inline_spans(text);
            assert!(
                spans.iter().any(|&(s, e)| &text[s..e] == call),
                "call_ after leading _ or hyphen must be an atomic wrap span, got {:?} for {text:?}",
                spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
            );
            assert_eq!(
                split(text),
                vec![first.to_string(), "Next sentence.".to_string()]
            );
        }
    }

    #[test]
    fn inline_org_call_after_word_underscore_is_subscript() {
        let call = "call_name(1. 2)";
        for text in [
            "See foo_call_name(1. 2) today. Next sentence.",
            "See x_call_name(1. 2) today. Next sentence.",
        ] {
            let (_, placeholders) = protect_inline_tokens(text);
            assert!(
                !placeholders.iter().any(|p| p.contains(call) || p == call),
                "call_ after word-underscore is a subscript, got {placeholders:?} for {text:?}"
            );
            let spans = atomic_inline_spans(text);
            assert!(
                !spans.iter().any(|&(s, e)| text[s..e].contains(call)),
                "call_ after word-underscore must not be an atomic wrap span, got {:?} for {text:?}",
                spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
            );
            let parts = split(text);
            assert!(
                parts.iter().any(|p| p.contains("Next sentence.")),
                "Next sentence. still splits, got {parts:?} for {text:?}"
            );
        }
    }

    #[test]
    fn inline_org_call_requires_word_boundary() {
        for text in [
            "acall_name(1. 2) today. Next sentence.",
            "1call_name(1. 2) today. Next sentence.",
        ] {
            let (_, placeholders) = protect_inline_tokens(text);
            assert!(
                !placeholders.iter().any(|p| p.contains("call_name(1. 2)")),
                "call_ must not match after a word char, got {placeholders:?} for {text:?}"
            );
        }
    }

    #[test]
    fn inline_org_cite_page_locator_is_not_a_sentence_boundary() {
        // EN_ABBREVIATIONS has `pp` but not `p`. Bracket-depth merge can
        // glue a UAX split, but wrap still cuts on `p. 7` unless the
        // org-element citation is one inline token.
        let cite = "[cite/t:see;@foo p. 7;@bar pp. 4;by foo]";
        let text = "See [cite/t:see;@foo p. 7;@bar pp. 4;by foo]. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == cite),
            "org citation must be one token, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == cite),
            "citation must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "See [cite/t:see;@foo p. 7;@bar pp. 4;by foo].".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_org_footnote_reference_interior_punct_is_not_a_sentence_boundary() {
        // GitHub #231 / snapper-gjkj: org-element-footnote-reference-parser
        // `[fn:: …]` stays one token so an interior period is not a
        // sentence boundary. `Next sentence.` still splits.
        let note = "[fn:: the Fourier. transform]";
        let text = "See [fn:: the Fourier. transform] in the notes. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == note),
            "inline footnote must be one token, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == note),
            "inline footnote must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "See [fn:: the Fourier. transform] in the notes.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn named_inline_org_footnote_reference_is_one_token() {
        let note = "[fn:note: the Fourier. transform]";
        let text = "See [fn:note: the Fourier. transform] in the notes. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == note),
            "named inline footnote must be one token, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                "See [fn:note: the Fourier. transform] in the notes.".to_string(),
                "Next sentence.".to_string()
            ]
        );
        let hyphen = "[fn:foo-bar: the Fourier. transform]";
        let hyphen_text = "See [fn:foo-bar: the Fourier. transform] in the notes. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(hyphen_text);
        assert!(
            placeholders.iter().any(|p| p == hyphen),
            "hyphen label must stay one token, got {placeholders:?}"
        );
        assert_eq!(
            split(hyphen_text),
            vec![
                "See [fn:foo-bar: the Fourier. transform] in the notes.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn standard_org_footnote_reference_is_not_an_inline_token() {
        // `[fn:1]` is org-footnote-re standard, not the inline arm.
        // Protecting it would split `See the claim.[fn:1]` at `claim.`.
        let text = "See the claim.[fn:1] Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            !placeholders.iter().any(|p| p == "[fn:1]"),
            "standard [fn:1] must not be a token, got {placeholders:?}"
        );
        let parts = split(text);
        assert!(
            parts.iter().any(|p| p.contains("See the claim.[fn:1]")),
            "standard ref must stay on the preceding sentence, got {parts:?}"
        );
    }

    #[test]
    fn unclosed_inline_org_footnote_is_not_an_inline_token() {
        let text = "See [fn:: the Fourier. transform in the notes. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            !placeholders.iter().any(|p| p.contains("[fn::")),
            "unclosed inline footnote must not swallow the sentence, got {placeholders:?}"
        );
    }

    #[test]
    fn nested_bracket_inline_org_footnote_is_one_token() {
        // org-element scan-lists on [] so `[fig. 1]` and `[[link]]` stay inside.
        let note = "[fn:: see [fig. 1]]";
        let text = "See [fn:: see [fig. 1]] in the notes. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == note),
            "nested-bracket footnote must be one token, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                "See [fn:: see [fig. 1]] in the notes.".to_string(),
                "Next sentence.".to_string()
            ]
        );
        let link = "[fn:: see [[https://ex.com][ex. site]]]";
        let link_text = "See [fn:: see [[https://ex.com][ex. site]]] in the notes. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(link_text);
        assert!(
            placeholders.iter().any(|p| p == link),
            "nested [[link]] must stay inside the footnote, got {placeholders:?}"
        );
        assert_eq!(
            split(link_text),
            vec![
                "See [fn:: see [[https://ex.com][ex. site]]] in the notes.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_rst_substitution_ref_interior_punct_is_not_a_sentence_boundary() {
        // GitHub #233 / snapper-15i9: Docutils Inliner.substitution_ref
        // `|fig. 1|` stays one token so an interior period is not a
        // sentence boundary. `Next sentence.` still splits.
        let sub = "|fig. 1|";
        let text = "See |fig. 1| in the caption. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == sub),
            "substitution_ref must be one token, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == sub),
            "substitution_ref must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "See |fig. 1| in the caption.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn rst_substitution_ref_hyperlink_suffix_stays_one_token() {
        for sub in ["|fig. 1|_", "|fig. 1|__"] {
            let text = format!("See {sub} in the caption. Next sentence.");
            let (_, placeholders) = protect_inline_tokens(&text);
            assert!(
                placeholders.iter().any(|p| p == sub),
                "suffixed substitution_ref must be one token, got {placeholders:?} for {sub}"
            );
            assert_eq!(
                split(&text),
                vec![
                    format!("See {sub} in the caption."),
                    "Next sentence.".to_string()
                ]
            );
        }
    }

    #[test]
    fn rst_line_block_and_open_bar_are_not_substitution_refs() {
        // Docutils line_block is `| ` (space after opener). Unclosed
        // `|fig. 1` is not substitution_ref. Leading/trailing space
        // inside the bars is invalid. A letter prefix (`\Verb|`) is
        // not a start-string.
        for text in [
            "| This is a line. Another sentence.",
            "See |fig. 1 in the caption. Next sentence.",
            "See | fig. 1| in the caption. Next sentence.",
            "See |fig. 1 | in the caption. Next sentence.",
            r"Use \Verb|a.b! c| here. Next sentence.",
        ] {
            let (_, placeholders) = protect_inline_tokens(text);
            assert!(
                !placeholders.iter().any(|p| p.contains("fig. 1")
                    || p.contains("This is a line")
                    || p.contains("a.b!")
                    || p.starts_with("| ")),
                "invalid / line-block `|` must not be a token, got {placeholders:?} for {text:?}"
            );
        }
        let parts = split("See |fig. 1 in the caption. Next sentence.");
        assert_eq!(parts.last().map(String::as_str), Some("Next sentence."));
        assert!(
            parts.iter().any(|p| p.contains("fig.")),
            "unclosed |fig. 1 must still expose the interior period, got {parts:?}"
        );
    }

    #[test]
    fn rst_version_substitution_ref_is_one_token_and_still_splits() {
        // snapper-t0th: `|version|` stays Prose. The closer period is
        // still a sentence end.
        let sub = "|version|";
        let text = "The current release is |version|. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == sub),
            "|version| must be one token, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                "The current release is |version|.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_math_preserved() {
        assert_eq!(
            split("The value $x = 3.14$ matters. Next sentence."),
            vec!["The value $x = 3.14$ matters.", "Next sentence."]
        );
    }

    #[test]
    fn inline_markdown_link_preserved() {
        assert_eq!(
            split("Visit [Example Inc.](https://example.com) now. Then read more."),
            vec![
                "Visit [Example Inc.](https://example.com) now.",
                "Then read more."
            ]
        );
    }

    #[test]
    fn inline_markdown_reference_link_interior_punct_is_not_a_sentence_boundary() {
        // GitHub #215 / snapper-e8g6: CM 6.3 `[text][ref]` must stay one
        // token so an interior period is not a sentence boundary.
        let link = "[the Fourier. transform][wiki]";
        let text = "See [the Fourier. transform][wiki] for details. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == link),
            "reference link must be one token, got {placeholders:?}"
        );
        let spans = atomic_inline_spans(text);
        assert!(
            spans.iter().any(|&(s, e)| &text[s..e] == link),
            "reference link must be an atomic wrap span, got {:?}",
            spans.iter().map(|&(s, e)| &text[s..e]).collect::<Vec<_>>()
        );
        assert_eq!(
            split(text),
            vec![
                "See [the Fourier. transform][wiki] for details.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn inline_markdown_collapsed_reference_link_interior_punct() {
        let link = "[the Fourier. transform][]";
        let text = "See [the Fourier. transform][] for details. Next sentence.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == link),
            "collapsed reference must be one token, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec![
                "See [the Fourier. transform][] for details.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn markdown_link_reference_definition_is_not_an_inline_token() {
        let text = "[wiki]: https://example.org/fourier";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            !placeholders.iter().any(|p| p.contains("[wiki]:")),
            "LRD must not be swallowed as [text][ref], got {placeholders:?}"
        );
    }

    #[test]
    fn inline_code_preserved() {
        assert_eq!(
            split("Use `std.io.Read` for input. Then process."),
            vec!["Use `std.io.Read` for input.", "Then process."]
        );
    }

    #[test]
    fn autolink_preserved() {
        assert_eq!(
            split("Visit <https://example.com/a.b> today. Then read more."),
            vec!["Visit <https://example.com/a.b> today.", "Then read more."]
        );
    }

    #[test]
    fn atomic_inline_spans_cover_wrap_tokens() {
        let text = "See [the example site](https://ex.com) and `some long code` plus $E = m$ and [[https://example.com][the example site]] and <https://ex.com/a>.";
        let spans = atomic_inline_spans(text);
        let tokens: Vec<&str> = spans.iter().map(|&(s, e)| &text[s..e]).collect();
        assert!(
            tokens
                .iter()
                .any(|t| *t == "[the example site](https://ex.com)"),
            "markdown link: {tokens:?}"
        );
        assert!(
            tokens.iter().any(|t| *t == "`some long code`"),
            "inline code: {tokens:?}"
        );
        assert!(tokens.iter().any(|t| *t == "$E = m$"), "math: {tokens:?}");
        assert!(
            tokens
                .iter()
                .any(|t| *t == "[[https://example.com][the example site]]"),
            "org link: {tokens:?}"
        );
        assert!(
            tokens.iter().any(|t| *t == "<https://ex.com/a>"),
            "autolink: {tokens:?}"
        );
    }

    #[test]
    fn org_bold_with_internal_period_not_split() {
        // Splitting would leave a line starting with `*Bold...` (false headline).
        assert_eq!(
            split("End of first. *Bold spans period. Continues* after."),
            vec!["End of first.", "*Bold spans period. Continues* after."]
        );
    }

    #[test]
    fn org_verbatim_inner_equals_pairs_to_the_real_closer() {
        // `pandoc -f org` makes two Code inlines, class verbatim, contents
        // `x = 1 -- note.` and `s = "x"`. A `=[^=]+=` regex instead closes
        // on the inner `=` and leaves the period after `note.` unprotected.
        let text = r#"so =x = 1 -- note.= reflows while =s = "x"= does not."#;
        let (protected, placeholders) = protect_inline_tokens(text);
        assert_eq!(
            placeholders,
            vec![
                r#"=x = 1 -- note.="#.to_string(),
                r#"=s = "x"="#.to_string(),
            ],
            "pairing must not close on the inner `=`; got {placeholders:?} from {protected:?}"
        );
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn org_verbatim_inner_equals_alone_stays_one_sentence() {
        let text = "so =x = 1 -- note.= reflows here.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert_eq!(placeholders, vec!["=x = 1 -- note.=".to_string()]);
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn org_verbatim_second_span_alone_does_not_need_inner_equals() {
        let text = r#"so =x -- note.= reflows while =s = "x"= does not."#;
        let (_, placeholders) = protect_inline_tokens(text);
        assert_eq!(
            placeholders,
            vec!["=x -- note.=".to_string(), r#"=s = "x"="#.to_string(),]
        );
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn org_code_span_with_dot_pl_stays_atomic() {
        let text = "~latexindent.pl~ covers LaTeX only. Snapper handles Org.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert_eq!(placeholders, vec!["~latexindent.pl~".to_string()]);
        assert_eq!(
            split(text),
            vec![
                "~latexindent.pl~ covers LaTeX only.".to_string(),
                "Snapper handles Org.".to_string(),
            ]
        );
    }

    #[test]
    fn markdown_code_span_with_dot_pl_stays_atomic() {
        let text = "`latexindent.pl` covers LaTeX only. Snapper handles Org.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert_eq!(placeholders, vec!["`latexindent.pl`".to_string()]);
        assert_eq!(
            split(text),
            vec![
                "`latexindent.pl` covers LaTeX only.".to_string(),
                "Snapper handles Org.".to_string(),
            ]
        );
    }

    #[test]
    fn org_code_inner_tilde_pairs_to_the_real_closer() {
        let text = r#"so ~x ~ 1 -- note.~ reflows while ~s ~ "x"~ does not."#;
        let (_, placeholders) = protect_inline_tokens(text);
        assert_eq!(
            placeholders,
            vec![
                r#"~x ~ 1 -- note.~"#.to_string(),
                r#"~s ~ "x"~"#.to_string(),
            ]
        );
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn markdown_double_backticks_can_hold_a_backtick() {
        let text = r#"see ``x ` 1 -- note.`` and ``s ` "x"`` too."#;
        let (_, placeholders) = protect_inline_tokens(text);
        assert_eq!(
            placeholders,
            vec![
                r#"``x ` 1 -- note.``"#.to_string(),
                r#"``s ` "x"``"#.to_string(),
            ]
        );
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn org_italic_with_internal_period_not_split() {
        assert_eq!(
            split("Lead-in. /Italic has a period. Still italic/ trail."),
            vec!["Lead-in.", "/Italic has a period. Still italic/ trail."]
        );
    }

    #[test]
    fn angle_bracket_tail_after_period_preserved() {
        // UAX #29 can drop a lone `>` after `.` without merge_tail_punctuation.
        assert_eq!(
            split("snapshot field is Box[T], not Vec[T]"),
            vec!["snapshot field is Box[T], not Vec[T]"]
        );
        assert_eq!(split("see <a.>"), vec!["see <a.>"]);
    }

    #[test]
    fn double_quoted_span_with_internal_period_not_split() {
        assert_eq!(
            split(r#"He said "Hello world. How are you?" Then he left."#),
            vec![r#"He said "Hello world. How are you?""#, "Then he left."]
        );
    }

    #[test]
    fn curly_double_quoted_span_with_internal_period_not_split() {
        assert_eq!(
            split("He said \u{201C}Hello world. How are you?\u{201D} Then he left."),
            vec![
                "He said \u{201C}Hello world. How are you?\u{201D}",
                "Then he left."
            ]
        );
    }

    #[test]
    fn quoted_title_with_abbrev_stays_one_sentence() {
        assert_eq!(
            split(r#"See the note "Fig. 3 is wrong." in the appendix."#),
            vec![r#"See the note "Fig. 3 is wrong." in the appendix."#]
        );
    }

    #[test]
    fn plaintext_format_keeps_dialogue_quote_together() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "He said \"Hello world. How are you?\" Then he left.\n";
        let cfg = FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("world.\nHow"),
            "must not break inside ASCII double quotes, got:\n{out}"
        );
        assert!(
            out.contains("you?\"\nThen") || out.contains("you?\" Then"),
            "may break after closing quote; got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn paren_span_with_internal_period_capital_not_split() {
        assert_eq!(
            split("See (Fig. 3 is wrong. Really.) Next."),
            vec!["See (Fig. 3 is wrong. Really.)", "Next."]
        );
    }

    #[test]
    fn bracket_span_with_internal_period_not_split() {
        assert_eq!(
            split("See [note. One] more."),
            vec!["See [note. One] more."]
        );
    }

    #[test]
    fn latex_style_quotes_with_internal_period_not_split() {
        assert_eq!(
            split("He said ``Hello world. How?'' Then."),
            vec!["He said ``Hello world. How?''", "Then."]
        );
    }

    #[test]
    fn escaped_ascii_quote_does_not_toggle_early() {
        // Backslash-escaped quotes are common in code-ish plaintext; do not
        // treat `\"` as ending the outer dialogue span.
        let out = split(r#"She said "He said \"no.\" Then left." Done."#);
        assert_eq!(out.len(), 2, "got {out:?}");
        assert!(
            out[0].contains(r#"\"no.\""#) || out[0].contains("no."),
            "{out:?}"
        );
        assert_eq!(out[1], "Done.");
    }

    #[test]
    fn single_quoted_dialogue_with_internal_period_not_split() {
        assert_eq!(
            split("He said 'Hello world. How are you?' Then he left."),
            vec!["He said 'Hello world. How are you?'", "Then he left."]
        );
    }

    #[test]
    fn apostrophe_contractions_still_split_sentences() {
        assert_eq!(
            split("Don't split here. Next sentence."),
            vec!["Don't split here.", "Next sentence."]
        );
        assert_eq!(
            split("It's fine. She said 'Go. Now.' Done."),
            vec!["It's fine.", "She said 'Go. Now.'", "Done."]
        );
    }

    #[test]
    fn curly_single_quoted_dialogue_not_split() {
        assert_eq!(
            split("He said \u{2018}Hello world. How?\u{2019} Then."),
            vec!["He said \u{2018}Hello world. How?\u{2019}", "Then."]
        );
    }

    #[test]
    fn newlines_invariant_holds_on_dialogue_output() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let samples = [
            "He said \"Hello world. How are you?\" Then he left.\n",
            "He said 'Hello world. How are you?' Then he left.\n",
            "See (Fig. 3 is wrong. Really.) Next.\n",
            "See [note. One] more. Trailing.\n",
            "He said ``Hello world. How?'' Then.\n",
            "Don't stop. It's ok. Done.\n",
            // Brackets inside inline code are opaque (protect_inline_tokens);
            // outer `[…].` closes before the period, so a following sentence
            // break is allowed.
            "[`[`].A\"\"]\"}\"''\n",
        ];
        let cfg = FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        }
        .without_safety_backstops();
        for input in samples {
            let out = format_text(input, &cfg).unwrap();
            assert!(
                newlines_respect_delimiter_spans(&out),
                "newline inside delimiter span for input {input:?}, out:\n{out}"
            );
            assert_eq!(
                format_text(&out, &cfg).unwrap(),
                out,
                "idempotence {input:?}"
            );
        }
    }

    #[test]
    fn quoted_exclamation_no_false_split() {
        assert_eq!(
            split(r#"He said "wow!" and left. She agreed."#),
            vec![r#"He said "wow!" and left."#, "She agreed."]
        );
    }

    #[test]
    fn paren_exclamation_no_false_split() {
        assert_eq!(
            split("He replied (with emphasis!) loudly. She agreed."),
            vec!["He replied (with emphasis!) loudly.", "She agreed."]
        );
    }

    #[test]
    fn paren_question_no_false_split() {
        assert_eq!(
            split("The answer (really?) surprised them. Next sentence."),
            vec!["The answer (really?) surprised them.", "Next sentence."]
        );
    }

    #[test]
    fn url_trailing_period_not_swallowed() {
        assert_eq!(
            split("Visit https://example.com/path. Then read more."),
            vec!["Visit https://example.com/path.", "Then read more."]
        );
    }

    #[test]
    fn url_with_query_trailing_period() {
        assert_eq!(
            split("See https://example.com/path?q=1&r=2. Next sentence."),
            vec!["See https://example.com/path?q=1&r=2.", "Next sentence."]
        );
    }

    #[test]
    fn org_file_token_trailing_punct_not_swallowed() {
        // GitHub #169: `file:\S+` used to eat the period so
        // `See file:/tmp/foo. Next` stayed one sentence.
        let (_, placeholders) = protect_inline_tokens("See file:/tmp/foo. Next");
        assert!(
            placeholders.iter().any(|p| p == "file:/tmp/foo"),
            "file: token must stop before sentence punct, got {placeholders:?}"
        );
        assert!(
            !placeholders.iter().any(|p| p.contains("file:/tmp/foo.")),
            "file: token must not swallow trailing period, got {placeholders:?}"
        );
        assert_eq!(
            split("See file:/tmp/foo. Next"),
            vec!["See file:/tmp/foo.".to_string(), "Next".to_string()]
        );
        assert_eq!(
            split("See file:/tmp/foo. Next sentence."),
            vec![
                "See file:/tmp/foo.".to_string(),
                "Next sentence.".to_string()
            ]
        );
        assert_eq!(
            split("See file:/tmp/foo! Next sentence."),
            vec![
                "See file:/tmp/foo!".to_string(),
                "Next sentence.".to_string()
            ]
        );
        assert_eq!(
            split("See file:/tmp/foo? Next sentence."),
            vec![
                "See file:/tmp/foo?".to_string(),
                "Next sentence.".to_string()
            ]
        );
        // Mid-path dots stay inside the token (same as URL path segments).
        assert_eq!(
            split("See file:/tmp/foo.org. Next sentence."),
            vec![
                "See file:/tmp/foo.org.".to_string(),
                "Next sentence.".to_string()
            ]
        );
    }

    #[test]
    fn ellipsis_splits() {
        assert_eq!(
            split("Sentence one... Sentence two."),
            vec!["Sentence one...", "Sentence two."]
        );
    }

    #[test]
    fn quoted_period_end_of_sentence() {
        // "done." followed by uppercase Start is a real sentence boundary
        assert_eq!(
            split(r#"End of quote: "done." Start again."#),
            vec![r#"End of quote: "done.""#, "Start again."]
        );
    }

    #[test]
    fn markdown_strong_with_internal_period_not_split() {
        // CommonMark `**`: a period next to the closer is still inside the span.
        let text = "This is **the end. Still bold** after.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == "**the end. Still bold**"),
            "strong span must be one token, got {placeholders:?}"
        );
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn markdown_strong_may_split_after_closer() {
        assert_eq!(
            split("It is **complex**. Equity is hard."),
            vec![
                "It is **complex**.".to_string(),
                "Equity is hard.".to_string()
            ]
        );
    }

    #[test]
    fn markdown_period_inside_closers_is_a_sentence_end() {
        assert_eq!(
            split("**Bold sentence.** Next one."),
            vec!["**Bold sentence.**".to_string(), "Next one.".to_string()]
        );
        assert_eq!(
            split("*Italic sentence.* Next one."),
            vec!["*Italic sentence.*".to_string(), "Next one.".to_string()]
        );
        assert_eq!(
            split("`Code sentence.` Next one."),
            vec!["`Code sentence.`".to_string(), "Next one.".to_string()]
        );
        assert_eq!(
            split("[Link sentence.](http://x.com) Next one."),
            vec![
                "[Link sentence.](http://x.com)".to_string(),
                "Next one.".to_string()
            ]
        );
        assert_eq!(
            split("Visit [Example Inc.](https://example.com) now. Then read more."),
            vec![
                "Visit [Example Inc.](https://example.com) now.".to_string(),
                "Then read more.".to_string()
            ]
        );
        assert_eq!(
            split("**Bold sentence.** \"Quoted next.\""),
            vec![
                "**Bold sentence.**".to_string(),
                "\"Quoted next.\"".to_string()
            ]
        );
        // PR #52 CI seed: period before backticks, then quote-backtick.
        // Next token is not a capital letter, so this is not a new sentence.
        assert_eq!(split("`.`` \"`"), vec!["`.`` \"`".to_string()]);
    }

    #[test]
    fn rst_role_closer_period_is_a_sentence_end() {
        assert_eq!(
            split("The task is in :file:`README.md`. Use this section for questions."),
            vec![
                "The task is in :file:`README.md`.".to_string(),
                "Use this section for questions.".to_string()
            ]
        );
        assert_eq!(
            split("The task is in ``README.md``. Use this section for questions."),
            vec![
                "The task is in ``README.md``.".to_string(),
                "Use this section for questions.".to_string()
            ]
        );
        assert_eq!(
            split("The task is in `README.md`:file:. Use this section for questions."),
            vec![
                "The task is in `README.md`:file:.".to_string(),
                "Use this section for questions.".to_string()
            ]
        );
        assert_eq!(
            split("See `RFC 2119`:rfc:keyword:. Next sentence."),
            vec![
                "See `RFC 2119`:rfc:keyword:.".to_string(),
                "Next sentence.".to_string()
            ]
        );
        assert_eq!(
            split("See `README.md`:file:. now continue."),
            vec!["See `README.md`:file:. now continue.".to_string()]
        );
    }

    #[test]
    fn markdown_period_inside_closers_survives_format_roundtrip() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text("**Bold sentence.** Next one.\n", &cfg).unwrap();
        assert_eq!(out, "**Bold sentence.**\nNext one.\n");
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn org_markdown_style_bold_period_splits_without_headline() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Org,
            ..Default::default()
        };
        let out = format_text("**CLI backend.** Pandoc 2.x on =PATH=.\n", &cfg).unwrap();
        assert_eq!(out, "**CLI backend.**\nPandoc 2.x on =PATH=.\n");
        assert!(
            !out.lines().any(|l| {
                let stars = l.chars().take_while(|c| *c == '*').count();
                stars > 0 && l[stars..].starts_with(' ')
            }),
            "split must not invent an org headline, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn markdown_em_with_internal_period_not_split() {
        let text = "This is *the end. Still em* after.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == "*the end. Still em*"),
            "em span must be one token, got {placeholders:?}"
        );
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn markdown_strike_with_internal_period_not_split() {
        let text = "This is ~~the end. Still strike~~ after.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders
                .iter()
                .any(|p| p == "~~the end. Still strike~~"),
            "strike span must be one token, got {placeholders:?}"
        );
        assert_eq!(split(text), vec![text.to_string()]);
    }

    #[test]
    fn markdown_strong_inner_star_does_not_close_early() {
        // Org `*bold*` closes on the first inner `*`. CommonMark flanking
        // keeps `**a * b. C**` as one strong span, so the period stays inside.
        let text = "Wrap **a * b. C** after. Next.";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == "**a * b. C**"),
            "must not close strong on the inner star, got {placeholders:?}"
        );
        assert_eq!(
            split(text),
            vec!["Wrap **a * b. C** after.".to_string(), "Next.".to_string()]
        );
    }

    #[test]
    fn markdown_emphasis_format_text_does_not_break_inside_span() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let cfg = FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        };
        let out = format_text("This is **the end. Still bold** after.\n", &cfg).unwrap();
        assert!(
            !out.contains("end.\nStill"),
            "must not split inside **...**, got:\n{out}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);

        let out = format_text("It is **complex**. Equity is hard.\n", &cfg).unwrap();
        assert!(
            out.contains("**complex**.") && out.contains("Equity is hard."),
            "may split after the closer, got:\n{out}"
        );
        assert!(
            !out.contains("**complex.\n"),
            "must not split before the closer, got:\n{out}"
        );
    }

    #[test]
    fn org_equals_does_not_pair_through_markdown_code_span() {
        // #77 keep-break puts a newline after `?`. That newline is Org PRE,
        // so `=` would otherwise close on the `=` inside `` `=!a` ``.
        let text = "?=\"`=!a`";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == "`=!a`"),
            "backtick span must stay one token, got {placeholders:?}"
        );
        assert_eq!(split(text), vec!["?".to_string(), "=\"`=!a`".to_string()]);

        let text = "?\n=\"`=!a`";
        let (_, placeholders) = protect_inline_tokens(text);
        assert!(
            placeholders.iter().any(|p| p == "`=!a`"),
            "keep-break newline must not let Org `=` steal the inner equals, got {placeholders:?}"
        );
        assert_eq!(split(text), vec!["?".to_string(), "=\"`=!a`".to_string()]);
    }

    #[test]
    fn plaintext_bang_inside_code_span_stays_atomic_after_keep_break() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "?=\"`=!a`\n";
        let cfg = FormatConfig {
            format: Format::Plaintext,
            max_width: 0,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            !out.contains("=!\n"),
            "must not split inside `=!a`, got:\n{out:?}"
        );
        assert_eq!(format_text(&out, &cfg).unwrap(), out);
    }

    #[test]
    fn keeps_break_before_lowercase_proper_noun() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        let input = "First sentence.\niCloud starts the second sentence.\n";
        for format in [
            Format::Markdown,
            Format::Plaintext,
            Format::Org,
            Format::Latex,
            Format::Rst,
        ] {
            let cfg = FormatConfig {
                format,
                max_width: 0,
                ..Default::default()
            };
            let out = format_text(input, &cfg).unwrap();
            assert_eq!(
                out, input,
                "{format:?} must keep the break before iCloud, got:\n{out}"
            );
            assert_eq!(format_text(&out, &cfg).unwrap(), out);
        }
    }

    #[test]
    fn splits_same_line_lowercase_proper_noun() {
        use crate::format::Format;
        use crate::{FormatConfig, format_text};

        assert_eq!(
            split("First sentence. iCloud starts the second sentence."),
            vec![
                "First sentence.".to_string(),
                "iCloud starts the second sentence.".to_string()
            ]
        );
        assert_eq!(
            split("Stop! iCloud starts now."),
            vec!["Stop!".to_string(), "iCloud starts now.".to_string()]
        );
        assert_eq!(
            split("Ready? iPhone is here."),
            vec!["Ready?".to_string(), "iPhone is here.".to_string()]
        );
        // UAX SB8: all-lowercase continuation stays one sentence.
        assert_eq!(
            split("First sentence. icloud starts the second sentence."),
            vec!["First sentence. icloud starts the second sentence.".to_string()]
        );
        assert_eq!(
            split("Use e.g. iCloud for storage."),
            vec!["Use e.g. iCloud for storage.".to_string()]
        );
        assert_eq!(
            split("`First sentence. iCloud stays.`"),
            vec!["`First sentence. iCloud stays.`".to_string()]
        );
        assert_eq!(
            split("This is **the end. iCloud still** after."),
            vec!["This is **the end. iCloud still** after.".to_string()]
        );

        let input = "First sentence. iCloud starts the second sentence.";
        let expected = "First sentence.\niCloud starts the second sentence.";
        for format in [
            Format::Markdown,
            Format::Plaintext,
            Format::Org,
            Format::Latex,
            Format::Rst,
        ] {
            let cfg = FormatConfig {
                format,
                max_width: 0,
                ..Default::default()
            };
            let out = format_text(input, &cfg).unwrap();
            assert_eq!(
                out, expected,
                "{format:?} must split the same-line iCloud fixture, got:\n{out}"
            );
            assert_eq!(format_text(&out, &cfg).unwrap(), out);
        }
    }
}
