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

/// True when `DelimState` ends outside all spans (balanced delimiters).
fn delimiters_balanced(text: &str) -> bool {
    use snapper_fmt::sentence::unicode::DelimState;
    let mut state = DelimState::default();
    state.feed(text);
    !state.is_inside()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

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
