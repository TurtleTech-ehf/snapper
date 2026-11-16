//! Source-byte spans for splice reflow.
//!
//! Native parsers record half-open `[start, end)` ranges into the original
//! input. Structure, blank, and code regions are those slices; only prose
//! (and a configured code-comment body) is rewritten. Output is assembled
//! by copying the gaps between rewrite ranges.

use crate::parser::Region;

/// Half-open byte range `[start, end)` into the parser input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Slice `input` if the range is in bounds and on a char boundary.
    pub fn slice(self, input: &str) -> Option<&str> {
        input.get(self.start..self.end)
    }
}

/// Origin recorded by a parser for one [`Region`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionOrigin {
    /// One contiguous source range (prose, structure, blank).
    Whole(ByteSpan),
    /// Fenced/env code block with independently recorded parts.
    Code {
        header: ByteSpan,
        body: ByteSpan,
        footer: ByteSpan,
    },
}

impl RegionOrigin {
    pub fn whole(self) -> ByteSpan {
        match self {
            RegionOrigin::Whole(s) => s,
            RegionOrigin::Code { header, footer, .. } => {
                ByteSpan::new(header.start, footer.end.max(header.end))
            }
        }
    }

    pub fn code(self) -> Option<CodeSpans> {
        match self {
            RegionOrigin::Code {
                header,
                body,
                footer,
            } => Some(CodeSpans {
                header,
                body,
                footer,
            }),
            RegionOrigin::Whole(_) => None,
        }
    }
}

/// Header/body/footer spans of a [`Region::Code`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeSpans {
    pub header: ByteSpan,
    pub body: ByteSpan,
    pub footer: ByteSpan,
}

/// A classified region plus the parser-recorded source origin, if any.
///
/// Pandoc reconstructs regions from an AST and leaves `origin` unset.
/// Native line parsers always set it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedRegion {
    pub region: Region,
    pub origin: Option<RegionOrigin>,
    /// When true, splice keeps the source physical line.
    /// The region stays [`Region::Prose`]; sentence breaks do not invent lines.
    pub line_preserving: bool,
}

impl SpannedRegion {
    pub fn unspanned(region: Region) -> Self {
        Self {
            region,
            origin: None,
            line_preserving: false,
        }
    }

    pub fn prose(text: String, span: ByteSpan) -> Self {
        Self {
            region: Region::Prose(text),
            origin: Some(RegionOrigin::Whole(span)),
            line_preserving: false,
        }
    }

    pub fn structure(input: &str, span: ByteSpan) -> Self {
        Self {
            region: Region::Structure(input[span.start..span.end].to_string()),
            origin: Some(RegionOrigin::Whole(span)),
            line_preserving: false,
        }
    }

    pub fn blank(input: &str, span: ByteSpan) -> Self {
        Self {
            region: Region::BlankLines(input[span.start..span.end].to_string()),
            origin: Some(RegionOrigin::Whole(span)),
            line_preserving: false,
        }
    }

    pub fn code(
        input: &str,
        lang: Option<String>,
        header: ByteSpan,
        body: ByteSpan,
        footer: ByteSpan,
    ) -> Self {
        Self {
            region: Region::Code {
                lang,
                header: input[header.start..header.end].to_string(),
                body: input[body.start..body.end].to_string(),
                footer: input[footer.start..footer.end].to_string(),
            },
            origin: Some(RegionOrigin::Code {
                header,
                body,
                footer,
            }),
            line_preserving: false,
        }
    }
}

/// One physical line of `input`, with byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line<'a> {
    /// Byte offset of the first character of the line.
    pub start: usize,
    /// Byte offset past the terminator (`\n` or `\r\n`), or EOF.
    pub end: usize,
    /// Line text without the terminator.
    pub text: &'a str,
}

impl Line<'_> {
    pub fn span(self) -> ByteSpan {
        ByteSpan::new(self.start, self.end)
    }

    /// Range of `text` only (no terminator).
    pub fn content_span(self) -> ByteSpan {
        ByteSpan::new(self.start, self.start + self.text.len())
    }

    /// Range of the terminator, empty at EOF with no trailing newline.
    pub fn terminator_span(self) -> ByteSpan {
        ByteSpan::new(self.start + self.text.len(), self.end)
    }
}

/// Split `input` into lines the way [`str::lines`] does, but keep offsets.
///
/// Terminators are `\n` or `\r\n`. A final line without a terminator is
/// included. `""` yields no lines.
pub fn iter_lines(input: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let bytes = input.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            let content_end = if i > start && bytes[i - 1] == b'\r' {
                i - 1
            } else {
                i
            };
            lines.push(Line {
                start,
                end: i + 1,
                text: &input[start..content_end],
            });
            start = i + 1;
        }
        i += 1;
    }
    if start < input.len() {
        lines.push(Line {
            start,
            end: input.len(),
            text: &input[start..],
        });
    }
    lines
}

/// Closers peeled after `.!?` (quotes, brackets, markup). A Markdown
/// `](url)` dest is not a closer: strip it first so the period inside
/// `[the docs.](http://x.com)` is visible.
const SENTENCE_CLOSERS: [char; 14] = [
    '"', '\'', ')', ']', '}', '*', '_', '`', '~', '/', '=', '+', '\u{201d}', '\u{2019}',
];

/// Prose accumulated across physical lines.
///
/// `](` indexes are recorded as text is appended. Sentence-end checks then
/// read that list instead of scanning the whole paragraph on every join.
#[derive(Debug, Default)]
pub struct ProseBuf {
    text: String,
    /// Ascending byte indexes of `](` in `text`.
    links: Vec<usize>,
}

impl ProseBuf {
    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn push_str(&mut self, added: &str) {
        self.note(added);
        self.text.push_str(added);
    }

    pub fn push(&mut self, ch: char) {
        let mut buf = [0u8; 4];
        self.push_str(ch.encode_utf8(&mut buf));
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.links.clear();
    }

    /// Take the text and forget the link indexes.
    pub fn take(&mut self) -> String {
        self.links.clear();
        std::mem::take(&mut self.text)
    }

    /// Sentence end using the indexes recorded in [`Self::push_str`].
    pub fn ends_with_sentence(&self) -> bool {
        ends_sentence_punct_tracked(self.as_str(), Some(&self.links))
    }

    fn note(&mut self, added: &str) {
        let base = self.text.len();
        let bytes = added.as_bytes();
        if base > 0 && self.text.as_bytes()[base - 1] == b']' && bytes.first() == Some(&b'(') {
            self.links.push(base - 1);
        }
        let mut i = 0;
        while i + 1 < bytes.len() {
            if bytes[i] == b']' && bytes[i + 1] == b'(' {
                self.links.push(base + i);
                i += 2;
                continue;
            }
            i += 1;
        }
    }
}

impl std::ops::Deref for ProseBuf {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text
    }
}

/// True when `s` ends a sentence: `.!?` plus optional quotes, brackets,
/// markup closers, or a trailing Markdown `](url)` destination.
pub fn ends_sentence_punct(s: &str) -> bool {
    ends_sentence_punct_tracked(s, None)
}

/// `links` is every `](` index in `s` when the caller has been tracking
/// appends. `None` scans once; that path is for a single string, not for
/// a paragraph that is rechecked on every line.
fn ends_sentence_punct_tracked(s: &str, links: Option<&[usize]>) -> bool {
    let mut core = s.trim_end();
    loop {
        let next = strip_trailing_md_link_dest(core, links).trim_end_matches(SENTENCE_CLOSERS);
        if next.len() == core.len() {
            break;
        }
        core = next;
    }
    core.ends_with('.') || core.ends_with('!') || core.ends_with('?')
}

/// If `s` ends with CommonMark `](dest)` or `](dest "title")`, keep the `]`
/// so [`SENTENCE_CLOSERS`] can peel it. Trailing closers after dest `)`
/// (`"` / `*` / …) stay part of the dest tail so `trim_end_matches` cannot
/// eat that `)` first.
fn strip_trailing_md_link_dest<'a>(s: &'a str, links: Option<&[usize]>) -> &'a str {
    let Some(open) = rightmost_link_open(s, links) else {
        return s;
    };
    if md_inline_dest_consumes(&s[open + 2..]) {
        &s[..=open]
    } else {
        s
    }
}

/// Rightmost `](` that still lies inside `s`.
///
/// Tracked indexes are byte offsets into the buffer `s` was trimmed from,
/// so a peeled suffix simply drops indexes past `s.len()`. Without an
/// index, a consuming destination has to reach the end of `s`, so a
/// paragraph that does not end in `)` is not scanned.
fn rightmost_link_open(s: &str, links: Option<&[usize]>) -> Option<usize> {
    let bytes = s.as_bytes();
    if let Some(links) = links {
        return links
            .iter()
            .rev()
            .copied()
            .find(|&i| i + 1 < bytes.len() && bytes[i] == b']' && bytes[i + 1] == b'(');
    }
    if !suffix_could_close_link(s) {
        return None;
    }
    s.rfind("](")
}

/// True when a trailing `](dest)` could still reach the end of `s`.
fn suffix_could_close_link(s: &str) -> bool {
    for ch in s.chars().rev() {
        if ch == ')' {
            return true;
        }
        if ch == ' ' || ch == '\t' || SENTENCE_CLOSERS.contains(&ch) {
            continue;
        }
        return false;
    }
    false
}

fn only_sentence_closers(s: &str) -> bool {
    s.chars().all(|c| SENTENCE_CLOSERS.contains(&c))
}

/// `true` when `s` is dest (optional title), closing `)`, then only closers.
fn md_inline_dest_consumes(s: &str) -> bool {
    let Some(n) = md_inline_dest_len(s) else {
        return false;
    };
    let rest = s[n..].trim_start_matches([' ', '\t']);
    dest_close_then_closers(rest) || md_title_then_close(rest)
}

fn dest_close_then_closers(s: &str) -> bool {
    s.strip_prefix(')').is_some_and(only_sentence_closers)
}

/// CommonMark dest length at the start of `s`. Empty dest is `]()` or `<>`.
fn md_inline_dest_len(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b')') {
        return Some(0);
    }
    if bytes.first() == Some(&b'<') {
        let mut i = 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' | b'\r' | b'<' => return None,
                b'>' => return Some(i + 1),
                b'\\' if i + 1 < bytes.len() => i += 2,
                _ => i += 1,
            }
        }
        return None;
    }
    let mut i = 0;
    let mut nest = 0i32;
    while i < bytes.len() {
        match bytes[i] {
            0x00..=0x20 => break,
            b'(' => nest += 1,
            b')' => {
                if nest == 0 {
                    break;
                }
                nest -= 1;
            }
            b'\\' if i + 1 < bytes.len() => i += 1,
            _ => {}
        }
        i += 1;
    }
    if nest != 0 || i == 0 { None } else { Some(i) }
}

fn md_title_then_close(s: &str) -> bool {
    let bytes = s.as_bytes();
    let close = match bytes.first() {
        Some(b'"') => b'"',
        Some(b'\'') => b'\'',
        Some(b'(') => b')',
        _ => return false,
    };
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b if b == close => {
                let after = s[i + 1..].trim_start_matches([' ', '\t']);
                return dest_close_then_closers(after);
            }
            b'\n' | b'\r' => return false,
            _ => i += 1,
        }
    }
    false
}

/// Insert the join between accumulated prose lines.
///
/// A source newline after sentence punctuation is a break. Replacing it
/// with a space lets UAX SB8 fuse the next token when it starts lowercase
/// (`iCloud`). Mid-sentence wraps still join with a space.
pub fn join_prose_gap(prose: &mut ProseBuf) {
    if prose.is_empty() {
        return;
    }
    if prose.ends_with_sentence() {
        prose.push('\n');
    } else {
        prose.push(' ');
    }
}

/// Join a region that already owns a `String`.
///
/// The paragraph buffer records `](` as it grows. A region string is
/// checked from its end, which is enough for one continuation.
pub fn join_prose_gap_string(prose: &mut String) {
    if prose.is_empty() {
        return;
    }
    if ends_sentence_punct(prose) {
        prose.push('\n');
    } else {
        prose.push(' ');
    }
}

/// Append a physical line to the running prose buffer and extend its span.
///
/// `include_terminator` is true for ordinary paragraphs (the original
/// newline is inside the rewrite range) and false for list-item text
/// (the terminator is a separate Structure slice).
pub fn push_prose_line(
    prose: &mut ProseBuf,
    prose_span: &mut Option<ByteSpan>,
    line: &Line<'_>,
    join_space: bool,
    include_terminator: bool,
) {
    if !prose.is_empty() && join_space {
        join_prose_gap(prose);
    }
    prose.push_str(line.text.trim());
    let end = if include_terminator {
        line.end
    } else {
        line.start + line.text.len()
    };
    match prose_span {
        None => *prose_span = Some(ByteSpan::new(line.start, end)),
        Some(s) => s.end = end,
    }
}

/// Flush accumulated prose into the region list.
pub fn flush_prose_spanned(
    prose: &mut ProseBuf,
    prose_span: &mut Option<ByteSpan>,
    regions: &mut Vec<SpannedRegion>,
) {
    if prose.is_empty() {
        return;
    }
    let span = prose_span.take().unwrap_or(ByteSpan::new(0, 0));
    regions.push(SpannedRegion::prose(prose.take(), span));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iter_lines_matches_str_lines_and_keeps_newlines() {
        let input = "a\nb\n\nc";
        let lines = iter_lines(input);
        let plain: Vec<&str> = input.lines().collect();
        assert_eq!(lines.iter().map(|l| l.text).collect::<Vec<_>>(), plain);
        assert_eq!(lines[0].span().slice(input), Some("a\n"));
        assert_eq!(lines[1].span().slice(input), Some("b\n"));
        assert_eq!(lines[2].span().slice(input), Some("\n"));
        assert_eq!(lines[3].span().slice(input), Some("c"));
        assert!(lines[3].terminator_span().is_empty());
    }

    #[test]
    fn iter_lines_empty() {
        assert!(iter_lines("").is_empty());
    }

    #[test]
    fn iter_lines_crlf() {
        let input = "a\r\nb\r\n";
        let lines = iter_lines(input);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "a");
        assert_eq!(lines[0].span().slice(input), Some("a\r\n"));
        assert_eq!(lines[1].text, "b");
    }

    #[test]
    fn join_prose_gap_keeps_break_after_sentence() {
        let mut prose = ProseBuf::default();
        prose.push_str("First sentence.");
        join_prose_gap(&mut prose);
        prose.push_str("iCloud starts the second sentence.");
        assert_eq!(
            prose.as_str(),
            "First sentence.\niCloud starts the second sentence."
        );

        let mut wrap = ProseBuf::default();
        wrap.push_str("The experiment ran for several");
        join_prose_gap(&mut wrap);
        wrap.push_str("weeks using the usual protocol.");
        assert_eq!(
            wrap.as_str(),
            "The experiment ran for several weeks using the usual protocol."
        );
    }

    #[test]
    fn ends_sentence_punct_sees_period_inside_md_link() {
        assert!(ends_sentence_punct("See [the docs.](http://x.com)"));
        assert!(ends_sentence_punct("See [the docs!](http://x.com)"));
        assert!(ends_sentence_punct("See [the docs?](http://x.com)"));
        assert!(ends_sentence_punct(
            r#"See [the docs.](http://x.com "title")"#
        ));
        assert!(ends_sentence_punct("See [the docs.](<http://x.com>)"));
        assert!(ends_sentence_punct("See [the docs.](<http://x.com/a)b>)"));
        assert!(ends_sentence_punct("See [the docs.](http://x.com)\""));
        assert!(ends_sentence_punct("See [the docs.](http://x.com)**"));
        assert!(ends_sentence_punct(
            "See [the docs.](http://x.com/foo(bar))"
        ));
        assert!(!ends_sentence_punct("See [the docs](http://x.com)"));
        assert!(!ends_sentence_punct(
            "See [the docs](http://x.com) for more"
        ));
        assert!(!ends_sentence_punct(
            "Visit [Example Inc.](http://x.com) now"
        ));
    }

    #[test]
    fn join_prose_gap_keeps_break_after_md_link_sentence() {
        let mut prose = ProseBuf::default();
        prose.push_str("See [the docs.](http://x.com)");
        join_prose_gap(&mut prose);
        prose.push_str("iCloud");
        assert_eq!(prose.as_str(), "See [the docs.](http://x.com)\niCloud");
    }

    #[test]
    fn tracked_link_indexes_match_a_full_scan() {
        let mut prose = ProseBuf::default();
        prose.push_str("See [the docs.](http://x.com)");
        assert!(prose.ends_with_sentence());
        assert!(ends_sentence_punct(prose.as_str()));
        prose.push(' ');
        prose.push_str("and then more words");
        assert_eq!(
            prose.ends_with_sentence(),
            ends_sentence_punct(prose.as_str())
        );
        assert!(!prose.ends_with_sentence());
        // The earlier `](` stays indexed. A later sentence end must win
        // without rescanning the prefix for it.
        let mut long = ProseBuf::default();
        long.push_str("Intro [kept.](http://example.com/a)");
        for i in 0..200 {
            long.push(' ');
            long.push_str("word");
            long.push_str(&i.to_string());
        }
        assert!(!long.ends_with_sentence());
        assert_eq!(
            long.ends_with_sentence(),
            ends_sentence_punct(long.as_str())
        );
        long.push(' ');
        long.push_str("Done.");
        assert!(long.ends_with_sentence());
        assert_eq!(
            long.ends_with_sentence(),
            ends_sentence_punct(long.as_str())
        );
        // A `](` split across two appends is still recorded.
        let mut split = ProseBuf::default();
        split.push_str("See [the docs.]");
        split.push_str("(http://x.com)");
        assert!(split.ends_with_sentence());
    }

    #[test]
    fn five_megabyte_paragraph_formats_in_seconds() {
        let line = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu\n";
        let target = 5 * 1024 * 1024;
        let mut input = String::with_capacity(target);
        while input.len() + line.len() <= target {
            input.push_str(line);
        }
        let cfg = crate::FormatConfig {
            format: crate::format::Format::Plaintext,
            ..Default::default()
        };
        let started = std::time::Instant::now();
        let out = crate::format_text(&input, &cfg).expect("format 5MB");
        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs() < 15,
            "a 5MB paragraph took {elapsed:?}, expected seconds"
        );
        assert!(out.len() + 1 >= input.len(), "lost paragraph text");
        assert_eq!(out.chars().filter(|c| *c == '\n').count(), 1);
    }
}
