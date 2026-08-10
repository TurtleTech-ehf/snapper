//! libfuzzer target for `format_bytes` across every [`Format`].
//!
//! Run: `cargo fuzz run format_text` from the repository root (requires
//! `cargo-fuzz`). The always-on `tests/safety_props.rs` suite covers the
//! same oracles under proptest.

#![no_main]

use libfuzzer_sys::fuzz_target;
use snapper_fmt::format::Format;
use snapper_fmt::oracle;
use snapper_fmt::{format_bytes, FormatConfig, InvalidUtf8Error};

fn cfg(format: Format) -> FormatConfig {
    FormatConfig {
        format,
        fixpoint_backstop: false,
        render_backstop: false,
        ..Default::default()
    }
}

fn format_of(b: u8) -> Format {
    match b % 5 {
        0 => Format::Plaintext,
        1 => Format::Markdown,
        2 => Format::Org,
        3 => Format::Latex,
        _ => Format::Rst,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let format = format_of(data[0]);
    let src = &data[1..];
    let c = cfg(format);
    match format_bytes(src, &c) {
        Err(e) => {
            assert!(
                e.downcast_ref::<InvalidUtf8Error>().is_some(),
                "only InvalidUtf8Error is allowed, got {e}"
            );
        }
        Ok(out) => {
            let twice = format_bytes(&out, &c).expect("second pass on valid UTF-8");
            assert_eq!(out, twice, "not idempotent");
            let s = std::str::from_utf8(src).unwrap();
            let o = std::str::from_utf8(&out).unwrap();
            assert!(
                oracle::matches(format, s, o),
                "oracle mismatch\n in={s:?}\n out={o:?}"
            );
        }
    }
});
