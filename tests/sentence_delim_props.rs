//! Property / matrix regression tests for delimiter-aware sentence splitting.
//!
//! Invariants (must hold for every generated case):
//! 1. `format_text` is idempotent on plaintext.
//! 2. Formatted output never places a `\n` while [`DelimState`] is still
//!    inside a quote/paren/bracket/brace span (shared with production code).
//! 3. Hand-built “must not fracture” fixtures never gain a line break that
//!    sits strictly between a known opener and its closer.

use proptest::prelude::*;
use snapper_fmt::format::Format;
use snapper_fmt::sentence::unicode::newlines_respect_delimiter_spans;
use snapper_fmt::{FormatConfig, format_text};

fn plaintext_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Plaintext,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn format_plain(input: &str) -> String {
    let mut s = input.to_string();
    if !s.ends_with('\n') {
        s.push('\n');
    }
    format_text(&s, &plaintext_cfg()).unwrap()
}

/// Deterministic matrix: combinations of wrappers × interiors × trails.
#[test]
fn matrix_wrappers_never_break_inside() {
    let interiors = [
        "Hello world. How are you?",
        "Fig. 3 is wrong. Really.",
        "note. One",
        "A. B. C. Done.",
        "Stop!",
        "Go. Now.",
    ];
    let wrappers: &[(&str, &str)] = &[
        ("\"", "\""),
        ("'", "'"),
        ("\u{201C}", "\u{201D}"),
        ("\u{2018}", "\u{2019}"),
        ("\u{00AB}", "\u{00BB}"),
        ("``", "''"),
        ("(", ")"),
        ("[", "]"),
        ("{", "}"),
    ];
    let prefixes = ["", "He said ", "See ", "Read "];
    let suffixes = ["", " Then he left.", " Next.", " Done."];

    for (open, close) in wrappers {
        for interior in &interiors {
            for prefix in &prefixes {
                for suffix in &suffixes {
                    let input = format!("{prefix}{open}{interior}{close}{suffix}");
                    let out = format_plain(&input);
                    assert!(
                        newlines_respect_delimiter_spans(&out),
                        "invariant failed\n input: {input:?}\n output:\n{out}"
                    );
                    let again = format_plain(&out);
                    assert_eq!(again, out, "idempotence\n input: {input:?}\n out:\n{out}");
                    // Must not place a newline between open and close markers.
                    if let (Some(a), Some(b)) = (out.find(open), out.rfind(close)) {
                        if a < b {
                            let between = out.get(a..b + close.len()).unwrap_or("");
                            assert!(
                                !between.contains('\n'),
                                "broke inside {open}...{close}\n input: {input:?}\n slice: {between:?}\n full:\n{out}"
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Apostrophe-heavy prose must still allow normal sentence breaks.
#[test]
fn contractions_still_break_between_sentences() {
    let out = format_plain("Don't stop here. It's fine. We're done.");
    assert!(out.contains("here.\n"), "got:\n{out}");
    assert!(
        out.contains("fine.\n") || out.ends_with("fine.\n"),
        "got:\n{out}"
    );
    assert!(newlines_respect_delimiter_spans(&out), "{out}");
}

/// Markup formats share `UnicodeSentenceSplitter` via `format_text` / reflow;
/// dialogue and paren spans must not fracture in extracted prose regions.
#[test]
fn multi_format_prose_respects_delimiter_spans() {
    let samples = [
        (
            Format::Org,
            "He said \"Hello world. How are you?\" Then left.\n",
        ),
        (Format::Org, "He said 'Go. Now.' Done.\n"),
        (
            Format::Markdown,
            "He said \"Hello world. How are you?\" Then left.\n",
        ),
        (Format::Markdown, "See (Fig. 3 is wrong. Really.) Next.\n"),
        (
            Format::Rst,
            "He said \"Hello world. How are you?\" Then left.\n",
        ),
        (Format::Rst, "See [note. One] more. Trailing.\n"),
        // LaTeX body only after \begin{document}; otherwise preamble is structure.
        (
            Format::Latex,
            "\\begin{document}\nHe said \"Hello world. How are you?\" Then left.\n\\end{document}\n",
        ),
    ];
    for (format, input) in samples {
        let cfg = FormatConfig {
            format,
            ..Default::default()
        }
        .without_safety_backstops();
        let out = format_text(input, &cfg).unwrap();
        assert!(
            newlines_respect_delimiter_spans(&out),
            "format={format:?} input={input:?} out:\n{out}"
        );
        let again = format_text(&out, &cfg).unwrap();
        assert_eq!(again, out, "idempotence format={format:?}");
        assert!(
            !out.contains("world.\nHow"),
            "fractured double-quote dialogue format={format:?} out:\n{out}"
        );
    }
}

/// Code-block *comments* also use the unicode splitter; configured rust
/// comments must keep quoted spans together.
#[test]
fn code_block_comment_respects_quotes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".snapperrc.toml"),
        "[code.rust]\nline_comment = \"//\"\n",
    )
    .unwrap();
    let input = "```rust\n// He said \"Hello. World.\" Outside.\nfn x() {}\n```\n";
    let cfg = FormatConfig {
        format: Format::Markdown,
        code: {
            let mut m = std::collections::HashMap::new();
            m.insert(
                "rust".into(),
                snapper_fmt::config::CodeLang {
                    line_comment: Some("//".into()),
                    block_comment: None,
                    ..Default::default()
                },
            );
            m
        },
        ..Default::default()
    }
    .without_safety_backstops();
    let out = format_text(input, &cfg).unwrap();
    assert!(
        !out.contains("Hello.\n// World") && !out.contains("Hello.\nWorld"),
        "comment quote fractured:\n{out}"
    );
    assert!(newlines_respect_delimiter_spans(&out), "{out}");
}

/// True when `DelimState` ends outside all spans (balanced delimiters).
///
/// Matches production: protect inline code/links first so brackets inside
/// `` `[` `` do not count as real nesting.
fn delimiters_balanced(text: &str) -> bool {
    use snapper_fmt::sentence::unicode::{DelimState, protect_inline_tokens};
    let (protected, _) = protect_inline_tokens(text);
    let mut state = DelimState::default();
    state.feed(&protected);
    !state.is_inside()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    // CI seed: nested backticks inside a `[...](...)` match leaked
    // `\x00PHn\x00` when restore ran inner-first.

    /// Random printable ASCII must stay idempotent. When delimiters are
    /// balanced, output must also never break mid-span (unbalanced input is
    /// allowed to leave spans open across a trailing line).
    #[test]
    fn proptest_plaintext_idempotent_and_span_safe(
        s in proptest::collection::vec(
            prop_oneof![
                proptest::char::range(' ', '~'),
                Just('"'),
                Just('\''),
                Just('('),
                Just(')'),
                Just('['),
                Just(']'),
                Just('{'),
                Just('}'),
                Just('.'),
                Just('!'),
                Just('?'),
                Just('`'),
            ],
            0..120,
        )
    ) {
        let input: String = s.into_iter().collect();
        // Skip inputs that are only whitespace — pipeline trims to empty.
        prop_assume!(!input.trim().is_empty());
        let out = format_plain(&input);
        let again = format_plain(&out);
        prop_assert_eq!(&again, &out, "idempotence failed");
        if delimiters_balanced(&input) {
            prop_assert!(
                newlines_respect_delimiter_spans(&out),
                "span newline on balanced input\n in={input:?}\n out={out:?}"
            );
        }
    }

    /// Explicit balanced dialogue/paren shapes (generator enforces matching closers).
    #[test]
    fn proptest_balanced_wrappers(
        prefix in "([A-Za-z ]{0,20})",
        interior in "([A-Za-z0-9 .!?]{1,40})",
        suffix in "([A-Za-z .]{0,20})",
        which in 0usize..6
    ) {
        let (open, close) = match which {
            0 => ("\"", "\""),
            1 => ("'", "'"),
            2 => ("(", ")"),
            3 => ("[", "]"),
            4 => ("{", "}"),
            _ => ("``", "''"),
        };
        let input = format!("{prefix}{open}{interior}{close}{suffix}");
        prop_assume!(!input.trim().is_empty());
        let out = format_plain(&input);
        prop_assert!(
            newlines_respect_delimiter_spans(&out),
            "in={input:?} out={out:?}"
        );
        prop_assert_eq!(format_plain(&out), out);
    }
}
