//! Property tests for `format_text` / `format_bytes` per [`Format`].
//!
//! With both safety backstops off: idempotency, oracle match, and
//! invalid UTF-8 is a hard error. These are the always-on stand-in for
//! a libfuzzer target (see `fuzz/fuzz_targets/format_text.rs`).

use proptest::prelude::*;
use snapper_fmt::format::Format;
use snapper_fmt::oracle;
use snapper_fmt::{FormatConfig, InvalidUtf8Error, format_bytes, format_text};

fn cfg(format: Format) -> FormatConfig {
    FormatConfig {
        format,
        fixpoint_backstop: false,
        render_backstop: false,
        ..Default::default()
    }
}

fn format_of(n: u8) -> Format {
    match n % 5 {
        0 => Format::Plaintext,
        1 => Format::Markdown,
        2 => Format::Org,
        3 => Format::Latex,
        _ => Format::Rst,
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        ..ProptestConfig::default()
    })]

    #[test]
    fn format_text_idempotent_and_oracle(
        s in "[A-Za-z#*][A-Za-z0-9#* ,;:'\"]{0,80}\\. [A-Za-z][A-Za-z0-9 ,;:'\"]{0,40}\\.",
        fmt_tag in 0u8..5,
    ) {
        let format = format_of(fmt_tag);
        let c = cfg(format);
        let out = format_text(&s, &c).expect("format_text on valid UTF-8");
        let twice = format_text(&out, &c).expect("second pass");
        prop_assert_eq!(&out, &twice, "not idempotent for {:?}", format);
        prop_assert!(
            oracle::matches(format, &s, &out),
            "oracle mismatch format={:?}\n in={:?}\n out={:?}",
            format,
            s,
            out
        );
    }

    #[test]
    #[test]
    fn format_text_paragraph_break_oracle(
        a in "[A-Za-z][A-Za-z0-9 ,;:'\"]{0,40}\\.",
        b in "[A-Za-z][A-Za-z0-9 ,;:'\"]{0,40}\\.",
        fmt_tag in 0u8..5,
    ) {
        let s = format!("{a}\n\n{b}");
        let format = format_of(fmt_tag);
        let c = cfg(format);
        let out = format_text(&s, &c).expect("format_text");
        let twice = format_text(&out, &c).expect("second pass");
        prop_assert_eq!(&out, &twice);
        prop_assert!(
            oracle::matches(format, &s, &out),
            "oracle mismatch format={:?}\n in={:?}\n out={:?}",
            format, s, out
        );
    }

    #[test]
    fn invalid_utf8_bytes_error(mut bytes in prop::collection::vec(any::<u8>(), 0..64)) {
        if std::str::from_utf8(&bytes).is_ok() {
            bytes.push(0xff);
        }
        let err = format_bytes(&bytes, &cfg(Format::Plaintext)).unwrap_err();
        prop_assert!(err.downcast_ref::<InvalidUtf8Error>().is_some());
    }
}
