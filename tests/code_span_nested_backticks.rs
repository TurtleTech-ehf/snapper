//! A code span that holds backticks keeps every byte.
//! `` `git-force-push (seat rule `ljos-policyd`)` `` lost the space before
//! the inner backtick when pandoc's `markdown` writer wrote it back.
//! That was the default path in 0.11.9; in 0.11.10 it is still reached by
//! `--use-pandoc` on plaintext or with `pandoc_format = "markdown"`.
//! The native path never touches code, and a pandoc write that changes
//! code is refused.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

const SINGLE: &str =
    "The guard says `git-force-push (seat rule `ljos-policyd`)` in plain words. Next one.\n";
const DOUBLE: &str =
    "The guard says ``git-force-push (seat rule `ljos-policyd`)`` in plain words. Next one.\n";

fn native(format: Format) -> FormatConfig {
    FormatConfig {
        format,
        max_width: 0,
        ..Default::default()
    }
}

fn pandoc(format: Format) -> FormatConfig {
    FormatConfig {
        format,
        max_width: 0,
        use_pandoc: true,
        ..Default::default()
    }
}

fn split(input: &str) -> String {
    input.replacen(" words. ", " words.\n", 1)
}

#[test]
fn native_markdown_keeps_a_single_backtick_span_with_inner_backticks() {
    let out = format_text(SINGLE, &native(Format::Markdown)).unwrap();
    assert_eq!(out, split(SINGLE));
}

#[test]
fn native_markdown_keeps_a_double_backtick_span_with_inner_backticks() {
    let out = format_text(DOUBLE, &native(Format::Markdown)).unwrap();
    assert_eq!(out, split(DOUBLE));
}

#[test]
fn native_plaintext_keeps_both_spans() {
    for input in [SINGLE, DOUBLE] {
        let out = format_text(input, &native(Format::Plaintext)).unwrap();
        assert_eq!(out, split(input));
    }
}

/// Every pandoc write either keeps the span's bytes or leaves the input as it was.
#[test]
fn pandoc_write_never_drops_the_space_inside_a_code_span() {
    if !snapper_fmt::parser::pandoc::pandoc_available() {
        eprintln!("pandoc not on PATH; skipping");
        return;
    }
    let mut markdown_reader = pandoc(Format::Markdown);
    markdown_reader.pandoc_format = Some("markdown".into());
    let configs = [
        pandoc(Format::Markdown),
        pandoc(Format::Plaintext),
        markdown_reader,
    ];
    for cfg in &configs {
        let format = cfg.format;
        for input in [SINGLE, DOUBLE] {
            let out = format_text(input, cfg).unwrap();
            assert!(
                !out.contains("rule`ljos"),
                "{format:?}: pandoc write dropped the space in the code span:\n{out}"
            );
            assert!(
                out.contains("seat rule `ljos-policyd`"),
                "{format:?}: code span changed:\n{out}"
            );
        }
    }
}

#[test]
fn inline_code_oracle_sees_the_dropped_space() {
    let bad = SINGLE.replace("rule `ljos", "rule`ljos");
    assert!(!snapper_fmt::oracle::inline_code_kept(SINGLE, &bad));
    assert!(snapper_fmt::oracle::inline_code_kept(
        SINGLE,
        &split(SINGLE)
    ));
}
