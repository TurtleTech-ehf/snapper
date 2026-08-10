use std::borrow::Cow;
use std::collections::HashMap;

use crate::config::CodeLang;
use crate::format::Format;
use crate::parser::{Region, RegionOrigin, SpannedRegion};
use crate::sentence::SentenceSplitter;
use crate::sentence::unicode::atomic_inline_spans;

/// Configuration for the reflow engine.
pub struct ReflowConfig<'a> {
    /// Maximum line width. 0 means unlimited.
    pub max_width: usize,
    /// Per-language code-block configuration (borrowed from `FormatConfig`).
    pub code: Option<&'a HashMap<String, CodeLang>>,
    /// When `true`, the per-language `formatter` runs after comment reflow.
    pub format_code: bool,
    /// Prefer soft breaks after independent-clause punctuation (`,`, `;`,
    /// `:`, em dash, `--`) when wrapping under `max_width`.
    pub clause_breaks: bool,
    /// Markup format of the document being reflowed. Wrap-created line
    /// starts that the format parser would read as a new block are
    /// escaped (Markdown) or the cut is skipped (Org and the rest).
    pub format: Format,
}

impl Default for ReflowConfig<'_> {
    fn default() -> Self {
        Self {
            max_width: 0,
            code: None,
            format_code: false,
            clause_breaks: false,
            format: Format::Plaintext,
        }
    }
}

/// Minimum region count before parallelizing reflow (large multi-MB org/md files).
#[cfg(feature = "cli")]
const PARALLEL_REGION_THRESHOLD: usize = 32;

/// Reflow a sequence of regions, applying sentence breaks to Prose regions.
///
/// With the `cli` feature, files that parse into many regions (typical large
/// Org/Markdown trees) reflow independent regions in parallel via rayon, then
/// concatenate in order.
pub fn reflow(
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    #[cfg(feature = "cli")]
    {
        if regions.len() >= PARALLEL_REGION_THRESHOLD {
            return reflow_parallel(regions, splitter, config);
        }
    }
    reflow_sequential(regions, splitter, config)
}

/// Why splice could not proceed. Callers fail closed (original document
/// or an error) instead of skipping the bad range.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SpliceError(pub String);

/// Reflow using parser-recorded byte ranges: non-prose is copied from
/// `source`, and only prose (plus rewritten comment spans inside code)
/// is rewritten. Missing origins or invalid spans are errors.
pub fn reflow_spanned(
    source: &str,
    spanned: &[SpannedRegion],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> Result<String, SpliceError> {
    if spanned.is_empty() {
        return Ok(String::new());
    }
    if let Some((i, _)) = spanned.iter().enumerate().find(|(_, s)| s.origin.is_none()) {
        return Err(SpliceError(format!("region {i} has no source origin")));
    }
    splice(source, spanned, splitter, config)
}

fn splice(
    source: &str,
    spanned: &[SpannedRegion],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> Result<String, SpliceError> {
    let regions: Vec<Region> = spanned.iter().map(|s| s.region.clone()).collect();
    let mut rewrites: Vec<(usize, usize, String)> = Vec::new();
    for (idx, sr) in spanned.iter().enumerate() {
        let origin = sr
            .origin
            .as_ref()
            .ok_or_else(|| SpliceError(format!("region {idx} has no source origin")))?;
        match (&sr.region, origin) {
            (Region::Prose(text), RegionOrigin::Whole(span)) => {
                let replacement = reflow_prose(text, idx, &regions, splitter, config);
                rewrites.push((span.start, span.end, replacement));
            }
            (
                Region::Code { lang, body, .. },
                RegionOrigin::Code {
                    body: body_span, ..
                },
            ) => {
                let code_cfg = lang
                    .as_deref()
                    .and_then(|l| config.code.and_then(|m| m.get(l)));
                if let Some(cfg) = code_cfg {
                    let reflowed = crate::code_block::reflow_code_body(
                        lang.as_deref().unwrap_or(""),
                        body,
                        cfg,
                        splitter,
                        config.format_code,
                    );
                    if reflowed != *body {
                        rewrites.push((body_span.start, body_span.end, reflowed));
                    }
                }
            }
            _ => {}
        }
    }
    rewrites.sort_by_key(|(start, _, _)| *start);
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for (start, end, repl) in rewrites {
        if start < cursor || end > source.len() || start > end || source.get(start..end).is_none() {
            return Err(SpliceError(format!(
                "invalid splice span {start}..{end} (cursor={cursor}, len={})",
                source.len()
            )));
        }
        out.push_str(&source[cursor..start]);
        out.push_str(&repl);
        cursor = end;
    }
    out.push_str(&source[cursor..]);
    Ok(out)
}

fn reflow_sequential(
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    let mut output = String::new();
    for (idx, region) in regions.iter().enumerate() {
        output.push_str(&reflow_one(region, idx, regions, splitter, config));
    }
    output
}

#[cfg(feature = "cli")]
fn reflow_parallel(
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    use rayon::prelude::*;
    // Indexed parallel map preserves order on collect.
    let parts: Vec<String> = regions
        .par_iter()
        .enumerate()
        .map(|(idx, region)| reflow_one(region, idx, regions, splitter, config))
        .collect();
    let mut output = String::new();
    for p in parts {
        output.push_str(&p);
    }
    output
}

fn reflow_one(
    region: &Region,
    idx: usize,
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    let mut output = String::new();
    match region {
        Region::Structure(s) => output.push_str(s),
        Region::BlankLines(s) => output.push_str(s),
        Region::Code {
            lang,
            header,
            body,
            footer,
        } => {
            output.push_str(header);
            let code_cfg = lang
                .as_deref()
                .and_then(|l| config.code.and_then(|m| m.get(l)));
            let reflowed = if let Some(cfg) = code_cfg {
                crate::code_block::reflow_code_body(
                    lang.as_deref().unwrap_or(""),
                    body,
                    cfg,
                    splitter,
                    config.format_code,
                )
            } else {
                body.clone()
            };
            output.push_str(&reflowed);
            output.push_str(footer);
        }
        Region::Prose(text) => {
            output.push_str(&reflow_prose(text, idx, regions, splitter, config));
        }
    }
    output
}

fn reflow_prose(
    text: &str,
    idx: usize,
    regions: &[Region],
    splitter: &dyn SentenceSplitter,
    config: &ReflowConfig,
) -> String {
    let mut output = String::new();
    let hang = match idx.checked_sub(1).and_then(|i| regions.get(i)) {
        Some(Region::Structure(s)) => hanging_prefix(s),
        _ => String::new(),
    };
    let hanging = hang.chars().count();
    let sentences = splitter.split(text);
    let nsent = sentences.len();
    for (i, sentence) in sentences.iter().enumerate() {
        if config.max_width > 0 {
            let layout = WrapLayout {
                initial_column: if i == 0 { hanging } else { 0 },
                first_indent: if i == 0 { "" } else { hang.as_str() },
                subsequent_indent: hang.as_str(),
            };
            let wrapped = wrap_prose(
                sentence,
                config.max_width,
                config.clause_breaks,
                config.format,
                layout,
            );
            output.push_str(&wrapped);
        } else {
            if hanging > 0 && i > 0 {
                output.push_str(&hang);
            }
            output.push_str(sentence);
        }
        if i + 1 < nsent {
            output.push('\n');
        }
    }
    if !sentences.is_empty() {
        // Splitter trims; keep a mid-line TeX ` % comment` space (not newlines).
        if text.ends_with([' ', '\t']) && !output.ends_with(char::is_whitespace) {
            let trail: String = text
                .chars()
                .rev()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            output.push_str(&trail);
        }
        // No forced paragraph break before inline islands (math/code) or
        // tight punctuation structures — those continue the same line.
        let suppress = match regions.get(idx + 1) {
            Some(Region::Structure(s)) if suppress_prose_trailing_newline(s) => true,
            Some(Region::Structure(s))
                if s.trim_start().starts_with('%') && text.ends_with([' ', '\t']) =>
            {
                true
            }
            _ => false,
        };
        if !suppress {
            output.push('\n');
        }
    }
    output
}

/// True when `word` ends with independent-clause punctuation (sembr rule 5),
/// ignoring trailing closing quotes and brackets. Words come from whitespace
/// splitting, so a match always marks a lossless break site: the punctuation
/// is followed by real whitespace in the source.
fn ends_with_clause_punct(word: &str) -> bool {
    let core = word.trim_end_matches(['"', '\'', ')', ']', '}']);
    core.ends_with(',')
        || core.ends_with(';')
        || core.ends_with(':')
        || core.ends_with('\u{2014}') // em dash —
        || core.ends_with("--")
}

/// Wrap `sentence` under `max_width`, preferring breaks after clause
/// punctuation (sembr rule 5). A sentence that already fits stays on one
/// line. Breaks only ever land at whitespace outside atomic tokens, so
/// links, inline code, `$math$`, and tokens like `1,000`, `10:30`, URLs,
/// and `--flags` are never split apart.
pub fn wrap_with_clause_breaks(sentence: &str, max_width: usize) -> String {
    wrap_prose(
        sentence,
        max_width,
        true,
        Format::Plaintext,
        WrapLayout::default(),
    )
}

#[derive(Default)]
struct WrapLayout<'a> {
    /// Columns already occupied on the first emitted line (list marker).
    initial_column: usize,
    /// Prefix for the first emitted line (subsequent sentences of a hang).
    first_indent: &'a str,
    /// Prefix for wrap-created lines. Interrupt is tested on the content
    /// after this indent; column-0 escape is the no-hang fallback.
    subsequent_indent: &'a str,
}

fn wrap_prose(
    sentence: &str,
    max_width: usize,
    clause_breaks: bool,
    format: Format,
    layout: WrapLayout<'_>,
) -> String {
    if max_width == 0 {
        return sentence.to_string();
    }
    wrap_atomic_words(sentence, max_width, clause_breaks, format, layout).join("\n")
}

/// Whitespace words with links, images, inline code, autolinks, math, and
/// Org `[[...]]` kept whole (internal spaces do not split). Adjacent
/// non-space text glues to a token so `foo[a](b)bar` is one word.
fn split_atomic_words(text: &str) -> Vec<&str> {
    let spans = atomic_inline_spans(text);
    let mut words = Vec::new();
    let mut buf_start: Option<usize> = None;
    let mut buf_end = 0usize;
    let mut pos = 0usize;
    let mut span_i = 0usize;

    while pos < text.len() {
        if span_i < spans.len() && pos >= spans[span_i].1 {
            span_i += 1;
            continue;
        }
        if span_i < spans.len() && pos == spans[span_i].0 {
            let end = spans[span_i].1;
            if buf_start.is_none() {
                buf_start = Some(pos);
            }
            buf_end = end;
            pos = end;
            span_i += 1;
            continue;
        }
        let rest_end = if span_i < spans.len() {
            spans[span_i].0
        } else {
            text.len()
        };
        if pos < rest_end {
            let gap = &text[pos..rest_end];
            for (i, ch) in gap.char_indices() {
                if ch.is_whitespace() {
                    if let Some(start) = buf_start.take() {
                        words.push(&text[start..buf_end]);
                    }
                } else {
                    let abs = pos + i;
                    if buf_start.is_none() {
                        buf_start = Some(abs);
                    }
                    buf_end = abs + ch.len_utf8();
                }
            }
        }
        pos = rest_end;
    }
    if let Some(start) = buf_start {
        words.push(&text[start..buf_end]);
    }
    words
}

fn is_ordered_list_marker(word: &str) -> bool {
    let bytes = word.as_bytes();
    if bytes.len() < 2 {
        return false;
    }
    let delim = *bytes.last().unwrap();
    if delim != b'.' && delim != b')' {
        return false;
    }
    bytes[..bytes.len() - 1].iter().all(|b| b.is_ascii_digit())
}

fn ordered_list_start(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return false;
    }
    matches!(bytes.get(i), Some(b'.') | Some(b')')) && matches!(bytes.get(i + 1), Some(b' ') | None)
}

fn thematic_or_setext_token(text: &str) -> bool {
    let first = text.split_whitespace().next().unwrap_or("");
    if first.len() < 3 {
        return false;
    }
    let b = first.as_bytes()[0];
    matches!(b, b'-' | b'=' | b'*' | b'_') && first.bytes().all(|c| c == b)
}

fn atx_heading_start(text: &str) -> bool {
    let n = text.bytes().take_while(|&b| b == b'#').count();
    (1..=6).contains(&n) && (text.len() == n || text.as_bytes()[n] == b' ')
}

fn md_list_start(text: &str) -> bool {
    text.starts_with("- ")
        || text.starts_with("* ")
        || text.starts_with("+ ")
        || ordered_list_start(text)
}

fn md_link_ref_def(text: &str) -> bool {
    text.starts_with('[') && text.contains("]:")
}

fn md_html_opener(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('<') else {
        return false;
    };
    rest.starts_with('!')
        || rest.starts_with('?')
        || rest.starts_with('/')
        || rest.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// True when `line` at column 0 is a new block in `format`'s grammar.
/// Already-escaped Markdown (`\-`, `\[ref]:`) does not match. A leading
/// `\` is not an escape in LaTeX (`\begin`, `\section`).
fn line_opens_block(format: Format, line: &str) -> bool {
    match format {
        Format::Plaintext => false,
        Format::Markdown => md_opens_block(line),
        Format::Org => org_opens_block(line),
        Format::Latex => latex_opens_block(line),
        Format::Rst => rst_opens_block(line),
    }
}

fn md_opens_block(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with("```") || t.starts_with("~~~") {
        return true;
    }
    if thematic_or_setext_token(t) {
        return true;
    }
    if t.starts_with('>') {
        return true;
    }
    if atx_heading_start(t) {
        return true;
    }
    if md_list_start(t) {
        return true;
    }
    if md_link_ref_def(t) {
        return true;
    }
    if md_html_opener(t) {
        return true;
    }
    false
}

fn org_opens_block(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with('|') {
        return true;
    }
    if t.starts_with('#') {
        return true;
    }
    let stars = t.bytes().take_while(|&b| b == b'*').count();
    if stars > 0 {
        return t.len() == stars || matches!(t.as_bytes()[stars], b' ' | b'\t');
    }
    if t.starts_with("- ") || t.starts_with("+ ") {
        return true;
    }
    ordered_list_start(t)
}

fn latex_opens_block(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with('%') {
        return true;
    }
    if t.starts_with("\\begin{") || t.starts_with("\\end{") || t.starts_with("\\[") {
        return true;
    }
    const CMDS: &[&str] = &[
        "\\part",
        "\\chapter",
        "\\section",
        "\\subsection",
        "\\subsubsection",
        "\\paragraph",
        "\\subparagraph",
    ];
    for cmd in CMDS {
        if let Some(after) = t.strip_prefix(cmd) {
            if after.is_empty()
                || after.starts_with('{')
                || after.starts_with('*')
                || after.starts_with(' ')
            {
                return true;
            }
        }
    }
    false
}

fn rst_opens_block(line: &str) -> bool {
    let t = line.trim_start();
    if t == ".." || t.starts_with(".. ") || t.starts_with("..\t") {
        return true;
    }
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    if t.starts_with(':') && t[1..].contains(':') {
        return true;
    }
    ordered_list_start(t)
}

/// Markdown backslash-escape for the first word of a wrap-created line.
/// Already-escaped words are left alone so a second pass does not
/// accumulate backslashes. Gated on Markdown only.
fn escape_md_first_word(word: &str) -> String {
    if word.starts_with('\\') {
        return word.to_string();
    }
    if is_ordered_list_marker(word) {
        let (digits, delim) = word.split_at(word.len() - 1);
        return format!("{digits}\\{delim}");
    }
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!("\\{first}{}", chars.as_str())
}

fn displayed_first_word<'a>(
    word: &'a str,
    may_escape: bool,
    rest: &str,
    format: Format,
) -> Cow<'a, str> {
    if may_escape && format == Format::Markdown && line_opens_block(format, rest) {
        Cow::Owned(escape_md_first_word(word))
    } else {
        Cow::Borrowed(word)
    }
}

/// Greedy wrap over atomic words. Forced breaks prefer the last clause
/// punctuation on the line. A wrap that would start a new block is
/// escaped in Markdown or skipped (loop) in other formats. The first
/// line of a list item is not escaped. Interrupt is tested on content
/// after hanging indent.
fn wrap_atomic_words(
    text: &str,
    max_width: usize,
    prefer_clause: bool,
    format: Format,
    layout: WrapLayout<'_>,
) -> Vec<String> {
    let words = split_atomic_words(text);
    if words.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let first = start == 0;
        let indent = if first {
            layout.first_indent
        } else {
            layout.subsequent_indent
        };
        let prefix_width = if first {
            layout.initial_column + layout.first_indent.chars().count()
        } else {
            layout.subsequent_indent.chars().count()
        };
        // First line of the first sentence (marker already emitted) is not
        // wrap-created. Later sentences and wrap-created lines may escape.
        let may_escape = format == Format::Markdown && !(first && layout.first_indent.is_empty());

        let mut end = start;
        let mut line_len = prefix_width;
        while end < words.len() {
            let rest = words[end..].join(" ");
            let displayed =
                displayed_first_word(words[end], may_escape && end == start, &rest, format);
            let wlen = displayed.chars().count();
            let next_len = if end == start {
                line_len + wlen
            } else {
                line_len + 1 + wlen
            };
            if next_len > max_width && end > start {
                break;
            }
            line_len = next_len;
            end += 1;
            if end == start + 1 && line_len > max_width {
                break;
            }
        }
        let mut break_at = end;
        if prefer_clause && end < words.len() {
            for j in (start..end).rev() {
                if ends_with_clause_punct(words[j]) {
                    break_at = j + 1;
                    break;
                }
            }
        }
        // Skip-cut loops until the next line would not open a block.
        if format != Format::Markdown {
            while break_at < words.len() && break_at > start {
                let candidate = words[break_at..].join(" ");
                if !line_opens_block(format, &candidate) {
                    break;
                }
                break_at += 1;
            }
        }

        let mut line = indent.to_string();
        let rest = words[start..break_at].join(" ");
        for (i, word) in words[start..break_at].iter().enumerate() {
            if i > 0 {
                line.push(' ');
            }
            if i == 0 {
                line.push_str(&displayed_first_word(word, may_escape, &rest, format));
            } else {
                line.push_str(word);
            }
        }
        lines.push(line);
        start = break_at;
    }
    lines
}

/// True when `s` is a list or quote marker that continuation lines hang from.
pub(crate) fn is_hanging_marker(s: &str) -> bool {
    !hanging_prefix(s).is_empty()
}

/// Prefix emitted on continuation lines after a list or quote marker.
/// Lists hang with spaces of marker width; Markdown quotes repeat the
/// quote prefix (`> `, `> > `, including leading indent). Empty when `s`
/// is not a marker (headings, fences, inline islands).
fn hanging_prefix(s: &str) -> String {
    if is_quote_marker(s) {
        return s.to_string();
    }
    let width = hanging_indent_width(s);
    if width > 0 {
        " ".repeat(width)
    } else {
        String::new()
    }
}

/// True when `s` is a Markdown quote marker: optional indent plus one or
/// more `> ` runs, and nothing else.
fn is_quote_marker(s: &str) -> bool {
    if s.is_empty() || s.contains('\n') || !s.ends_with(' ') {
        return false;
    }
    let trimmed = s.trim_start();
    let bytes = trimmed.as_bytes();
    if bytes.len() < 2 || bytes.len() % 2 != 0 {
        return false;
    }
    bytes.chunks_exact(2).all(|c| c == b"> ")
}

/// Column width of a list marker that continuation lines hang at with
/// spaces. Zero for quotes and non-markers.
fn hanging_indent_width(s: &str) -> usize {
    if s.is_empty() || s.contains('\n') || !s.ends_with(' ') {
        return 0;
    }
    let trimmed = s.trim_start();
    if trimmed.len() < 2 {
        return 0;
    }
    // Parser markers are `core` plus one trailing space; leading indent is
    // part of the hang so nested `   - ` continues at column 5.
    let core = &trimmed[..trimmed.len() - 1];
    let is_bullet = matches!(core, "-" | "*" | "+");
    let is_ordered = (core.ends_with('.') || core.ends_with(')'))
        && core.len() > 1
        && core[..core.len() - 1].bytes().all(|b| b.is_ascii_digit());
    if is_bullet || is_ordered {
        s.chars().count()
    } else {
        0
    }
}

/// When the next region is an inline structure island (pandoc `Math`/`Code` as
/// Structure), do not end the preceding prose with a hard line break.
fn suppress_prose_trailing_newline(s: &str) -> bool {
    if s == "\n" || s.starts_with('}') || s.starts_with(']') || s.starts_with(')') {
        return true;
    }
    // Islands may carry a leading space for glue after reflow trims prose.
    let t = s.trim();
    // Inline math: single-line `$...$` (not display `$$...$$`).
    if t.starts_with('$') && !t.starts_with("$$") && !t.contains('\n') {
        return true;
    }
    // Inline code island: single-line `...` (optional trailing space already trimmed).
    let code = t.trim_end_matches(' ');
    if code.starts_with('`') && code.ends_with('`') && code.len() >= 2 && !code.contains('\n') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::unicode::UnicodeSentenceSplitter;

    fn reflow_text(input: &str) -> String {
        let regions = vec![Region::Prose(input.to_string())];
        let config = ReflowConfig::default();
        reflow(&regions, &UnicodeSentenceSplitter::new(), &config)
    }

    #[test]
    fn missing_origin_is_error() {
        let spanned = vec![crate::parser::SpannedRegion::unspanned(Region::Prose(
            "Hi.".into(),
        ))];
        let err = reflow_spanned(
            "Hi.",
            &spanned,
            &UnicodeSentenceSplitter::new(),
            &ReflowConfig::default(),
        )
        .unwrap_err();
        assert!(err.0.contains("origin"), "{err}");
    }

    #[test]
    fn invalid_span_is_error() {
        use crate::parser::{ByteSpan, RegionOrigin, SpannedRegion};
        let spanned = vec![SpannedRegion {
            region: Region::Prose("Hi.".into()),
            origin: Some(RegionOrigin::Whole(ByteSpan::new(0, 99))),
        }];
        let err = reflow_spanned(
            "Hi.",
            &spanned,
            &UnicodeSentenceSplitter::new(),
            &ReflowConfig::default(),
        )
        .unwrap_err();
        assert!(err.0.contains("invalid splice span"), "{err}");
    }

    #[test]
    fn simple_reflow() {
        let result = reflow_text("Hello world. This is a test. Another sentence.");
        assert_eq!(result, "Hello world.\nThis is a test.\nAnother sentence.\n");
    }

    #[test]
    fn idempotent() {
        let input = "Hello world.\nThis is a test.\nAnother sentence.";
        let first = reflow_text(input);
        let second = reflow_text(&first);
        assert_eq!(first, second, "reflow must be idempotent");
    }

    #[test]
    fn preserves_structure() {
        let regions = vec![
            Region::Structure("#+TITLE: Test\n".to_string()),
            Region::BlankLines("\n".to_string()),
            Region::Prose("First sentence. Second sentence.".to_string()),
        ];
        let config = ReflowConfig::default();
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        assert_eq!(
            result,
            "#+TITLE: Test\n\nFirst sentence.\nSecond sentence.\n"
        );
    }

    #[test]
    fn max_width_wrapping() {
        let regions = vec![Region::Prose(
            "This is a very long sentence that should be wrapped at a reasonable width for readability in narrow terminals.".to_string(),
        )];
        let config = ReflowConfig {
            max_width: 40,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        // Every line should be <= 40 chars
        for line in result.lines() {
            assert!(
                line.len() <= 40,
                "Line too long: {} chars: {:?}",
                line.len(),
                line
            );
        }
    }

    #[test]
    fn clause_breaks_prefer_commas_under_max_width() {
        // Issue #7 sample: max_width=80 with clause breaks should land soft
        // breaks after the independent-clause commas rather than packing
        // mid-phrase as greedy wrap without clause preference does.
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let wrapped = wrap_with_clause_breaks(sentence, 80);
        let expected = "\
It contains rules which govern how the Objectives are orchestrated,
along with rules which can automatically activate the Objectives in the plan,
without additional human intervention.";
        assert_eq!(
            wrapped, expected,
            "clause-first wrap:\n--- got ---\n{wrapped}\n--- expected ---\n{expected}"
        );
        for line in wrapped.lines() {
            assert!(
                line.chars().count() <= 80,
                "line exceeds max_width: {line:?}"
            );
        }
    }

    #[test]
    fn clause_breaks_off_packs_past_first_comma() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let regions = vec![Region::Prose(sentence.to_string())];
        let config = ReflowConfig {
            max_width: 80,
            clause_breaks: false,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        // Greedy wrap packs past the first comma; clause_breaks would break there.
        assert!(
            result.contains("orchestrated, along with\n"),
            "control path still packs past the first comma: {result:?}"
        );
        assert!(result.contains('\n'));
    }

    #[test]
    fn clause_breaks_via_reflow_config() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let regions = vec![Region::Prose(sentence.to_string())];
        let config = ReflowConfig {
            max_width: 80,
            clause_breaks: true,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        assert!(
            result.contains("orchestrated,\nalong with"),
            "reflow with clause_breaks must break after first comma: {result:?}"
        );
        assert!(
            result.contains("plan,\nwithout"),
            "reflow with clause_breaks must break after second comma: {result:?}"
        );
    }

    #[test]
    fn clause_breaks_handles_semicolon_colon_emdash() {
        let s = "First clause; second clause: third clause — fourth clause.";
        // Fits under the limit: no break is forced, the sentence stays whole.
        assert_eq!(wrap_with_clause_breaks(s, 80), s);
        // Forced under a tight limit: every break lands after clause punctuation.
        assert_eq!(
            wrap_with_clause_breaks(s, 20),
            "First clause;\nsecond clause:\nthird clause —\nfourth clause."
        );
    }

    #[test]
    fn clause_breaks_leave_fitting_sentences_alone() {
        let regions = vec![Region::Prose(
            "Hello, world. Short, sweet, and done.".to_string(),
        )];
        let config = ReflowConfig {
            max_width: 80,
            clause_breaks: true,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        assert_eq!(result, "Hello, world.\nShort, sweet, and done.\n");
    }

    #[test]
    fn clause_breaks_never_split_inside_tokens() {
        // Clause punctuation not followed by whitespace stays inside its
        // token; a break there would render as an inserted space.
        let s = "Totals reached 1,000,000 by 10:30 via https://example.com/a,b using --clause-breaks and rock—paper logic in a sentence long enough to need wrapping.";
        let wrapped = wrap_with_clause_breaks(s, 30);
        let rejoined: Vec<&str> = wrapped.split_whitespace().collect();
        let original: Vec<&str> = s.split_whitespace().collect();
        assert_eq!(rejoined, original, "wrapping must be lossless: {wrapped:?}");
        for token in [
            "1,000,000",
            "10:30",
            "https://example.com/a,b",
            "--clause-breaks",
            "rock—paper",
        ] {
            assert!(
                wrapped.lines().any(|l| l.contains(token)),
                "{token:?} must stay on a single line: {wrapped:?}"
            );
        }
    }

    #[test]
    fn clause_breaks_idempotent() {
        let sentence = "It contains rules which govern how the Objectives are orchestrated, along with rules which can automatically activate the Objectives in the plan, without additional human intervention.";
        let config = ReflowConfig {
            max_width: 80,
            clause_breaks: true,
            ..Default::default()
        };
        let splitter = UnicodeSentenceSplitter::new();
        let first = reflow(&[Region::Prose(sentence.to_string())], &splitter, &config);
        let second = reflow(
            &[Region::Prose(first.trim_end().to_string())],
            &splitter,
            &config,
        );
        assert_eq!(first, second, "clause-break reflow must be idempotent");
    }

    #[test]
    fn long_clause_still_word_wraps() {
        let long = "This is a deliberately long independent clause without internal punctuation that must still wrap under a tight max width constraint for the test.";
        let wrapped = wrap_with_clause_breaks(long, 40);
        for line in wrapped.lines() {
            assert!(
                line.chars().count() <= 40,
                "overlong clause must still wrap: {line:?}"
            );
        }
        assert!(wrapped.contains('\n'));
    }

    fn reflow_regions(regions: Vec<Region>) -> String {
        reflow(
            &regions,
            &UnicodeSentenceSplitter::new(),
            &ReflowConfig::default(),
        )
    }

    #[test]
    fn hanging_indent_width_markers_only() {
        assert_eq!(hanging_indent_width("- "), 2);
        assert_eq!(hanging_indent_width("* "), 2);
        assert_eq!(hanging_indent_width("+ "), 2);
        assert_eq!(hanging_indent_width("1. "), 3);
        assert_eq!(hanging_indent_width("10. "), 4);
        assert_eq!(hanging_indent_width("1) "), 3);
        assert_eq!(hanging_indent_width("   - "), 5);
        // Quotes are a prefix hang, not a space-hang bullet.
        assert_eq!(hanging_indent_width("> "), 0);
        assert_eq!(hanging_indent_width("> > "), 0);
        assert_eq!(hanging_prefix("> "), "> ");
        assert_eq!(hanging_prefix("> > "), "> > ");
        assert_eq!(hanging_prefix("  > "), "  > ");
        assert_eq!(hanging_prefix("- "), "  ");
        assert_eq!(hanging_prefix("1. "), "   ");
        assert_eq!(hanging_indent_width("\n"), 0);
        assert_eq!(hanging_indent_width("#+TITLE: Test\n"), 0);
        assert_eq!(hanging_indent_width("$x$"), 0);
        assert_eq!(hanging_indent_width("`code`"), 0);
        assert_eq!(hanging_indent_width("## heading\n"), 0);
    }

    #[test]
    fn list_hanging_indent_second_sentence() {
        let result = reflow_regions(vec![
            Region::Structure("- ".to_string()),
            Region::Prose("One. Two.".to_string()),
            Region::Structure("\n".to_string()),
        ]);
        assert_eq!(result, "- One.\n  Two.\n");
    }

    #[test]
    fn numbered_list_hanging_indent() {
        let result = reflow_regions(vec![
            Region::Structure("1. ".to_string()),
            Region::Prose("One. Two.".to_string()),
            Region::Structure("\n".to_string()),
        ]);
        assert_eq!(result, "1. One.\n   Two.\n");
    }

    #[test]
    fn quote_hanging_indent() {
        let result = reflow_regions(vec![
            Region::Structure("> ".to_string()),
            Region::Prose("One. Two.".to_string()),
            Region::Structure("\n".to_string()),
        ]);
        assert_eq!(result, "> One.\n> Two.\n");
    }

    #[test]
    fn nested_quote_repeats_full_prefix() {
        let result = reflow_regions(vec![
            Region::Structure("> ".to_string()),
            Region::Prose("Quoted one. Quoted two.".to_string()),
            Region::Structure("\n".to_string()),
            Region::Structure("> > ".to_string()),
            Region::Prose("Nested one. Nested two.".to_string()),
            Region::Structure("\n".to_string()),
        ]);
        assert_eq!(
            result,
            "> Quoted one.\n> Quoted two.\n> > Nested one.\n> > Nested two.\n"
        );
    }

    #[test]
    fn nested_list_items_do_not_flatten() {
        let result = reflow_regions(vec![
            Region::Structure("1. ".to_string()),
            Region::Prose("Parent one. Parent two.".to_string()),
            Region::Structure("\n".to_string()),
            Region::Structure("   - ".to_string()),
            Region::Prose("Child one. Child two.".to_string()),
            Region::Structure("\n".to_string()),
        ]);
        assert_eq!(
            result,
            "1. Parent one.\n   Parent two.\n   - Child one.\n     Child two.\n"
        );
        assert!(
            result.contains("\n   - Child one."),
            "nested marker must stay its own item: {result:?}"
        );
    }

    #[test]
    fn adjacent_list_items_are_not_merged() {
        let result = reflow_regions(vec![
            Region::Structure("- ".to_string()),
            Region::Prose("First item. More first.".to_string()),
            Region::Structure("\n".to_string()),
            Region::Structure("- ".to_string()),
            Region::Prose("Second item. More second.".to_string()),
            Region::Structure("\n".to_string()),
        ]);
        assert_eq!(
            result,
            "- First item.\n  More first.\n- Second item.\n  More second.\n"
        );
    }

    #[test]
    fn single_sentence_list_item_has_no_extra_indent() {
        let result = reflow_regions(vec![
            Region::Structure("- ".to_string()),
            Region::Prose("Only one sentence.".to_string()),
            Region::Structure("\n".to_string()),
        ]);
        assert_eq!(result, "- Only one sentence.\n");
    }

    #[test]
    fn wrap_lines_under_list_also_hang() {
        let regions = vec![
            Region::Structure("- ".to_string()),
            Region::Prose(
                "This is a deliberately long first sentence that must wrap. Short.".to_string(),
            ),
            Region::Structure("\n".to_string()),
        ];
        let config = ReflowConfig {
            max_width: 32,
            ..Default::default()
        };
        let result = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        let lines: Vec<&str> = result.lines().collect();
        assert!(
            lines[0].starts_with("- "),
            "first line keeps marker: {result:?}"
        );
        for line in &lines[1..] {
            assert!(
                line.starts_with("  "),
                "wrap/continuation must hang at marker width: {result:?}"
            );
            assert!(
                !line.starts_with("- "),
                "must not invent a new list item: {result:?}"
            );
        }
        for line in &lines {
            assert!(
                line.chars().count() <= 32,
                "line exceeds max_width: {line:?}"
            );
        }
    }

    fn wrap_sentence(sentence: &str, max_width: usize, clause_breaks: bool) -> String {
        let regions = vec![Region::Prose(sentence.to_string())];
        let config = ReflowConfig {
            max_width,
            clause_breaks,
            ..Default::default()
        };
        reflow(&regions, &UnicodeSentenceSplitter::new(), &config)
    }

    fn assert_atomic_token(wrapped: &str, token: &str) {
        assert!(
            wrapped.lines().any(|l| l.contains(token)),
            "{token:?} must stay on a single line:\n{wrapped}"
        );
        assert!(
            !wrapped.contains('\u{00a0}'),
            "wrap must not inject NBSP:\n{wrapped}"
        );
    }

    #[test]
    fn max_width_keeps_markdown_link_atomic() {
        let token = "[the example site](https://ex.com)";
        let sentence = "Please consult [the example site](https://ex.com) today.";
        for clause in [false, true] {
            let wrapped = wrap_sentence(sentence, 40, clause);
            assert_atomic_token(&wrapped, token);
        }
    }

    #[test]
    fn max_width_keeps_markdown_image_atomic() {
        let token = "![alt text here](https://img.example.com/a.png)";
        let sentence = "Look at ![alt text here](https://img.example.com/a.png) now please.";
        for clause in [false, true] {
            let wrapped = wrap_sentence(sentence, 36, clause);
            assert_atomic_token(&wrapped, token);
        }
    }

    #[test]
    fn max_width_keeps_inline_code_atomic() {
        let token = "`some long inline code`";
        let sentence = "Use `some long inline code` today.";
        for clause in [false, true] {
            let wrapped = wrap_sentence(sentence, 20, clause);
            assert_atomic_token(&wrapped, token);
        }
    }

    #[test]
    fn max_width_keeps_org_link_atomic() {
        let token = "[[https://example.com][the example site]]";
        let sentence = "See [[https://example.com][the example site]] now.";
        for clause in [false, true] {
            let wrapped = wrap_sentence(sentence, 30, clause);
            assert_atomic_token(&wrapped, token);
        }
    }

    #[test]
    fn max_width_keeps_math_atomic() {
        let token = "$E = m c^{2}$";
        let sentence = "The identity $E = m c^{2}$ holds in this frame.";
        for clause in [false, true] {
            let wrapped = wrap_sentence(sentence, 24, clause);
            assert_atomic_token(&wrapped, token);
        }
    }

    #[test]
    fn max_width_keeps_autolink_atomic() {
        let token = "<https://example.com/a/long-path>";
        let sentence = "Visit <https://example.com/a/long-path> today.";
        // Format::Markdown: wrap-created `<https://...>` must stay an
        // autolink, not a substring of `\<https://...` (HTML-opener escape).
        for clause in [false, true] {
            let regions = vec![Region::Prose(sentence.to_string())];
            let config = ReflowConfig {
                max_width: 24,
                clause_breaks: clause,
                format: Format::Markdown,
                ..Default::default()
            };
            let wrapped = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
            assert!(
                wrapped.lines().any(|l| l.trim() == token),
                "autolink must appear unchanged, not escaped:\n{wrapped}"
            );
            assert!(
                !wrapped.contains("\\<"),
                "must not inject \\ into the autolink:\n{wrapped}"
            );
            assert!(!wrapped.contains('\u{00a0}'));
        }
        let email = "<user@example.com>";
        let email_sentence = "Write <user@example.com> today please.";
        let regions = vec![Region::Prose(email_sentence.to_string())];
        let config = ReflowConfig {
            max_width: 20,
            format: Format::Markdown,
            ..Default::default()
        };
        let wrapped = reflow(&regions, &UnicodeSentenceSplitter::new(), &config);
        assert!(
            wrapped.lines().any(|l| l.trim() == email),
            "email autolink must appear unchanged:\n{wrapped}"
        );
        assert!(
            !wrapped.contains("\\<"),
            "email autolink escaped:\n{wrapped}"
        );
    }

    #[test]
    fn overlong_atomic_token_sits_alone() {
        let token =
            "[a deliberately long link description that exceeds width](https://example.com)";
        let sentence = format!("See {token} now.");
        for clause in [false, true] {
            let wrapped = wrap_sentence(&sentence, 20, clause);
            assert_atomic_token(&wrapped, token);
            let line = wrapped
                .lines()
                .find(|l| l.contains(token))
                .expect("token line");
            assert_eq!(line.trim(), token, "overlong token sits alone:\n{wrapped}");
        }
    }

    #[test]
    fn textwrap_path_never_splits_numeric_url_or_flag_tokens() {
        let s = "Totals reached 1,000,000 by 10:30 via https://example.com/a,b using --clause-breaks in a sentence long enough to wrap.";
        let wrapped = wrap_sentence(s, 30, false);
        let rejoined: Vec<&str> = wrapped.split_whitespace().collect();
        let original: Vec<&str> = s.split_whitespace().collect();
        assert_eq!(rejoined, original, "wrapping must be lossless: {wrapped:?}");
        for token in [
            "1,000,000",
            "10:30",
            "https://example.com/a,b",
            "--clause-breaks",
        ] {
            assert!(
                wrapped.lines().any(|l| l.contains(token)),
                "{token:?} must stay on a single line: {wrapped:?}"
            );
        }
        assert!(!wrapped.contains('\u{00a0}'));
    }

    #[test]
    fn wrap_created_dash_escaped_in_markdown() {
        // "The options are apples" is 22 chars; width 23 breaks before "-".
        let input = "The options are apples - oranges extra.";
        let config = crate::FormatConfig {
            format: crate::format::Format::Markdown,
            max_width: 23,
            ..Default::default()
        };
        let result = crate::format_text(input, &config).unwrap();
        assert!(
            !result.lines().any(|l| l.starts_with("- ")),
            "wrap must not invent a list:\n{result}"
        );
        assert!(
            result.lines().any(|l| l.starts_with("\\- ")),
            "wrap-created dash must be markdown-escaped:\n{result}"
        );
        assert!(!result.contains('\u{00a0}'));
    }

    #[test]
    fn wrap_created_hash_star_plus_gt_and_ordered_escaped_in_markdown() {
        let config = crate::FormatConfig {
            format: crate::format::Format::Markdown,
            max_width: 23,
            ..Default::default()
        };
        let cases = [
            ("The options are apples * oranges extra.", "\\* "),
            ("The options are apples + oranges extra.", "\\+ "),
            ("The options are apples > oranges extra.", "\\> "),
            ("The options are apples # oranges extra.", "\\# "),
            ("The options are apples 1. oranges extra.", "1\\. "),
        ];
        for (input, escaped_prefix) in cases {
            let result = crate::format_text(input, &config).unwrap();
            assert!(
                result.lines().any(|l| l.starts_with(escaped_prefix)),
                "expected a wrap-created line starting {escaped_prefix:?}:\n{result}"
            );
            assert!(
                !result.lines().any(|l| {
                    l.starts_with("* ")
                        || l.starts_with("+ ")
                        || l.starts_with("> ")
                        || l.starts_with("# ")
                        || l.starts_with("1. ")
                }),
                "wrap must not invent a block:\n{result}"
            );
        }
    }

    #[test]
    fn wrap_created_dash_skips_cut_in_org() {
        let input = "The options are apples - oranges extra.";
        let config = crate::FormatConfig {
            format: crate::format::Format::Org,
            max_width: 23,
            ..Default::default()
        };
        let result = crate::format_text(input, &config).unwrap();
        assert!(
            !result.lines().any(|l| l.starts_with("- ")),
            "wrap must not invent an Org list:\n{result}"
        );
        assert!(
            !result.contains('\\'),
            "Org skips the cut instead of backslash-escaping:\n{result}"
        );
        assert!(
            result.contains("apples -"),
            "dash stays on the previous line:\n{result}"
        );
    }

    #[test]
    fn list_item_first_line_is_not_escaped() {
        let input = "- item that is long enough to wrap onto a second line of words";
        let config = crate::FormatConfig {
            format: crate::format::Format::Markdown,
            max_width: 24,
            ..Default::default()
        };
        let result = crate::format_text(input, &config).unwrap();
        assert!(
            result.starts_with("- item"),
            "first line of a list item stays a list:\n{result}"
        );
        assert!(
            !result.starts_with("\\-"),
            "must not escape the real list marker:\n{result}"
        );
    }

    #[test]
    fn wrap_escape_is_idempotent() {
        let input = "The options are apples - oranges extra.";
        let config = crate::FormatConfig {
            format: crate::format::Format::Markdown,
            max_width: 23,
            ..Default::default()
        };
        let first = crate::format_text(input, &config).unwrap();
        let second = crate::format_text(&first, &config).unwrap();
        assert_eq!(first, second, "second pass must not change output");
        assert!(
            !first.contains("\\\\"),
            "second pass must not accumulate backslashes:\n{first}"
        );
        let third = crate::format_text(&second, &config).unwrap();
        assert_eq!(second, third);
    }

    fn wrap_fmt(input: &str, width: usize, format: crate::format::Format) -> String {
        // Drive wrap directly so the format parser cannot swallow the
        // interrupt token before `--max-width` sees it.
        let regions = vec![Region::Prose(input.to_string())];
        let config = ReflowConfig {
            max_width: width,
            format,
            ..Default::default()
        };
        reflow(&regions, &UnicodeSentenceSplitter::new(), &config)
    }

    fn assert_no_col0_block(result: &str, starts: &[&str]) {
        for line in result.lines() {
            for prefix in starts {
                assert!(
                    !line.starts_with(prefix),
                    "wrap must not invent a block starting {prefix:?}:\n{result}"
                );
            }
        }
    }

    // Review cases A–H: interrupt predicate is format grammar at column 0.

    #[test]
    fn wrap_created_fence_is_not_a_markdown_block() {
        // A. ``` / ~~~
        let tick = wrap_fmt(
            "The options are apples ``` extra words here.",
            23,
            crate::format::Format::Markdown,
        );
        assert_no_col0_block(&tick, &["```"]);
        let tilde = wrap_fmt(
            "The options are apples ~~~ extra words here.",
            23,
            crate::format::Format::Markdown,
        );
        assert_no_col0_block(&tilde, &["~~~"]);
    }

    #[test]
    fn wrap_created_thematic_break_is_not_a_markdown_block() {
        // B. --- / === / *** / ___
        for token in ["---", "===", "***", "___"] {
            let input = format!("The options are apples {token} extra words here.");
            let result = wrap_fmt(&input, 23, crate::format::Format::Markdown);
            assert_no_col0_block(&result, &[token]);
        }
    }

    #[test]
    fn wrap_created_link_ref_is_not_a_markdown_block() {
        // C. [ref]:
        let result = wrap_fmt(
            "The options are apples [ref]: https://ex.com extra.",
            23,
            crate::format::Format::Markdown,
        );
        assert_no_col0_block(&result, &["[ref]:", "[ref]: "]);
    }

    #[test]
    fn wrap_created_html_tag_is_not_a_markdown_block() {
        // D. HTML tags
        let result = wrap_fmt(
            "The options are apples <div> extra words here.",
            23,
            crate::format::Format::Markdown,
        );
        assert_no_col0_block(&result, &["<div>", "<div "]);
    }

    #[test]
    fn wrap_created_gt_without_space_is_not_a_blockquote() {
        // E. >foo
        let result = wrap_fmt(
            "The options are apples >foo extra words here.",
            23,
            crate::format::Format::Markdown,
        );
        assert_no_col0_block(&result, &[">foo", "> foo", ">"]);
        assert!(
            result.lines().any(|l| l.contains("foo")),
            "content must remain:\n{result}"
        );
    }

    #[test]
    fn wrap_created_latex_comment_and_commands_are_not_blocks() {
        // F. LaTeX % and \begin / \section. Leading `\` is not an MD escape.
        let pct = wrap_fmt(
            "The options are apples % extra words here.",
            23,
            crate::format::Format::Latex,
        );
        assert_no_col0_block(&pct, &["% ", "%"]);
        assert!(
            pct.contains("apples %"),
            "percent stays with previous line:\n{pct}"
        );

        let begin = wrap_fmt(
            "The options are apples \\begin{equation} extra words.",
            23,
            crate::format::Format::Latex,
        );
        assert_no_col0_block(&begin, &["\\begin", "\\begin{equation}"]);
        assert!(
            begin.contains("apples \\begin"),
            "\\begin is not an MD escape; skip-cut must keep it:\n{begin}"
        );

        let section = wrap_fmt(
            "The options are apples \\section{Foo} extra words.",
            23,
            crate::format::Format::Latex,
        );
        assert_no_col0_block(&section, &["\\section", "\\section{Foo}"]);
        assert!(
            section.contains("apples \\section"),
            "\\section is not an MD escape:\n{section}"
        );
    }

    #[test]
    fn wrap_created_rst_directive_is_not_a_block() {
        // G. RST ..
        let result = wrap_fmt(
            "The options are apples .. extra words here.",
            23,
            crate::format::Format::Rst,
        );
        assert_no_col0_block(&result, &[".. ", ".."]);
        assert!(
            result.contains("apples .."),
            "RST skip-cut keeps the directive marker:\n{result}"
        );
    }

    #[test]
    fn wrap_created_org_table_pipe_is_not_a_block() {
        // H. Org |
        let result = wrap_fmt(
            "The options are apples | extra words here.",
            23,
            crate::format::Format::Org,
        );
        assert_no_col0_block(&result, &["| ", "|"]);
        assert!(
            result.contains("apples |"),
            "Org skip-cut keeps the pipe:\n{result}"
        );
    }

    #[test]
    fn skip_cut_loops_until_next_line_is_not_a_block() {
        // Org: break_at += 1 only eats `-`, then `* oranges` is a headline.
        let result = wrap_fmt(
            "The options are apples - * oranges extra.",
            23,
            crate::format::Format::Org,
        );
        assert_no_col0_block(&result, &["- ", "* ", "*"]);
        assert!(
            result.contains("apples - *"),
            "both markers stay on the previous line:\n{result}"
        );
    }

    #[test]
    fn wrap_does_not_hyphenate_well_known_or_hyphenated_urls() {
        let known = wrap_sentence(
            "This is a well-known example in a sentence long enough to wrap here.",
            20,
            false,
        );
        assert!(
            known.lines().any(|l| l.contains("well-known")),
            "must not hyphen-split well-known:\n{known}"
        );
        let url = "https://example.com/well-known-path-name";
        let wrapped = wrap_sentence(
            &format!("See {url} extra words to force a wrap here."),
            24,
            false,
        );
        assert_atomic_token(&wrapped, url);
        assert!(!wrapped.contains('\u{00a0}'));
    }

    #[test]
    fn wrap_created_list_lines_hang_and_interrupt_after_indent() {
        // Prefix width on line 0; indent on wrap-created lines; col-0 escape
        // is fallback. A dash inside the item must not become a new list.
        let result = crate::format_text(
            "- The options are apples - oranges extra words.",
            &crate::FormatConfig {
                format: crate::format::Format::Markdown,
                max_width: 25,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            result.starts_with("- The options"),
            "list first line stays a list:\n{result}"
        );
        let mut lines = result.lines();
        let first = lines.next().expect("first line");
        assert!(first.starts_with("- "), "{first:?}");
        for line in result.lines().skip(1) {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            let looks_like_list = trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("+ ")
                || (trimmed.len() >= 3
                    && trimmed.as_bytes()[0].is_ascii_digit()
                    && (trimmed.contains(". ") || trimmed.contains(") ")));
            assert!(
                !(indent <= 3 && looks_like_list),
                "wrap-created line must not parse as a list:\n{result}"
            );
            if line.contains("oranges") {
                assert!(
                    line.starts_with(' ') || line.starts_with('\\'),
                    "hang or escape, not column-0 dash:\n{result}"
                );
            }
        }
    }
}
