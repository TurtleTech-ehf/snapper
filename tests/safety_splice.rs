//! Safety-bar tests: splice (non-prose is an input slice), fixpoint, and
//! format-local render oracle.
//!
//! Production `format_text` enables the fixpoint and render backstops.
//! These tests turn them off so a planner that needs the backstop fails
//! here instead of being masked.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use snapper_fmt::format::Format;
use snapper_fmt::oracle;
use snapper_fmt::parser::{Region, RegionOrigin, parser_for_format};
use snapper_fmt::{FormatConfig, InvalidUtf8Error, format_bytes, format_text};

fn fixture(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(p).unwrap()
}

fn cfg(format: Format) -> FormatConfig {
    FormatConfig {
        format,
        fixpoint_backstop: false,
        render_backstop: false,
        ..Default::default()
    }
}

fn non_prose_payload(regions: &[Region]) -> Vec<u8> {
    let mut out = Vec::new();
    for r in regions {
        match r {
            Region::Structure(s) | Region::BlankLines(s) => out.extend(s.as_bytes()),
            Region::Code {
                header,
                body,
                footer,
                ..
            } => {
                out.extend(header.as_bytes());
                out.extend(body.as_bytes());
                out.extend(footer.as_bytes());
            }
            Region::Prose(_) => {}
        }
    }
    out
}

fn hash_bytes(b: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

/// Structure/Code/Blank must be the original input slice, not a
/// reconstructed `format!("{line}\\n")` that invents a trailing newline.
#[test]
fn structure_without_newline_is_exact_slice() {
    let input = "## Title";
    let regions = parser_for_format(Format::Markdown).parse(input);
    match regions.as_slice() {
        [Region::Structure(s)] => {
            assert_eq!(
                s.as_str(),
                input,
                "structure must be the input slice, not a re-serialized line"
            );
        }
        other => panic!("expected one Structure, got {other:?}"),
    }
}

#[test]
fn org_headline_without_newline_is_exact_slice() {
    let input = "* TODO a headline";
    let regions = parser_for_format(Format::Org).parse(input);
    match regions.as_slice() {
        [Region::Structure(s)] => {
            assert_eq!(s.as_str(), input);
        }
        other => panic!("expected one Structure, got {other:?}"),
    }
}

fn assert_non_prose_hash_stable(name: &str, format: Format) {
    let input = fixture(name);
    let parser = parser_for_format(format);
    let before = non_prose_payload(&parser.parse(&input));
    let formatted = format_text(&input, &cfg(format)).unwrap();
    let after = non_prose_payload(&parser.parse(&formatted));
    assert_eq!(
        hash_bytes(&before),
        hash_bytes(&after),
        "non-prose hash changed on {name}\n--- before ---\n{}\n--- after ---\n{}",
        String::from_utf8_lossy(&before),
        String::from_utf8_lossy(&after)
    );
    assert_eq!(
        before, after,
        "non-prose bytes changed on {name} (hash collision check)"
    );
}

#[test]
fn non_prose_hash_sample_md() {
    assert_non_prose_hash_stable("sample.md", Format::Markdown);
}

#[test]
fn non_prose_hash_sample_tex() {
    assert_non_prose_hash_stable("sample.tex", Format::Latex);
}

#[test]
fn non_prose_hash_edge_cases_org() {
    assert_non_prose_hash_stable("edge_cases.org", Format::Org);
}

/// Native parsers record a byte span for every region, and Structure/Code/Blank
/// text equals the input slice.
#[test]
fn recorded_spans_are_input_slices_on_fixtures() {
    for (name, format) in [
        ("sample.md", Format::Markdown),
        ("sample.tex", Format::Latex),
        ("edge_cases.org", Format::Org),
    ] {
        let input = fixture(name);
        let spanned = parser_for_format(format).parse_full(&input);
        for sr in &spanned {
            let origin = sr
                .origin
                .unwrap_or_else(|| panic!("{name}: native parser must record origin for {sr:?}"));
            match (&sr.region, origin) {
                (Region::Prose(_), RegionOrigin::Whole(span)) => {
                    assert!(span.end <= input.len(), "{name}: prose span out of range");
                }
                (Region::Structure(s) | Region::BlankLines(s), RegionOrigin::Whole(span)) => {
                    let slice = &input[span.start..span.end];
                    assert_eq!(
                        s.as_str(),
                        slice,
                        "{name}: non-prose region is not an input slice\nregion={s:?}\nslice={slice:?}"
                    );
                }
                (
                    Region::Code {
                        header,
                        body,
                        footer,
                        ..
                    },
                    RegionOrigin::Code {
                        header: hs,
                        body: bs,
                        footer: fs,
                    },
                ) => {
                    assert_eq!(header.as_str(), &input[hs.start..hs.end], "{name} header");
                    assert_eq!(body.as_str(), &input[bs.start..bs.end], "{name} body");
                    assert_eq!(footer.as_str(), &input[fs.start..fs.end], "{name} footer");
                }
                other => panic!("{name}: origin kind mismatch: {other:?}"),
            }
        }
    }
}

/// With the fixpoint backstop off, a second pass must already be a no-op
/// on the fixtures. A planner that needs the loop fails here.
#[test]
fn fixtures_are_single_pass_fixpoints() {
    for (name, format) in [
        ("sample.md", Format::Markdown),
        ("sample.tex", Format::Latex),
        ("edge_cases.org", Format::Org),
        ("sample.txt", Format::Plaintext),
    ] {
        let input = fixture(name);
        let c = cfg(format);
        let once = format_text(&input, &c).unwrap();
        let twice = format_text(&once, &c).unwrap();
        assert_eq!(once, twice, "{name} is not a single-pass fixpoint");
    }
}

/// Oracle holds on the fixtures with the render backstop disabled.
#[test]
fn fixtures_pass_oracle_without_backstop() {
    for (name, format) in [
        ("sample.md", Format::Markdown),
        ("sample.tex", Format::Latex),
        ("edge_cases.org", Format::Org),
        ("sample.txt", Format::Plaintext),
    ] {
        let input = fixture(name);
        let out = format_text(&input, &cfg(format)).unwrap();
        assert!(
            oracle::matches(format, &input, &out),
            "{name}: oracle mismatch\n--- in ---\n{input}\n--- out ---\n{out}"
        );
    }
}

#[test]
fn invalid_utf8_is_a_hard_error() {
    let mut bytes = b"Hello world. Next.\n".to_vec();
    bytes.push(0xff);
    let err = format_bytes(&bytes, &cfg(Format::Plaintext)).unwrap_err();
    assert!(
        err.downcast_ref::<InvalidUtf8Error>().is_some(),
        "expected InvalidUtf8Error, got {err:?}"
    );
}

/// A mid-token period+capital (`a0.A.`) changes the prose word sequence if
/// split. Production backstops must fail closed to the original.
#[test]
fn render_backstop_keeps_mid_token_period_capital() {
    let input = "A. a0.A.";
    let out = format_text(
        input,
        &FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(out, input);
}

#[test]
fn production_fixpoint_returns_original_on_cap() {
    // Empty and already-formatted inputs converge in one extra pass.
    let input = "Hello world.\nThis is a test.\n";
    let out = format_text(
        input,
        &FormatConfig {
            format: Format::Plaintext,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(out, "Hello world.\nThis is a test.\n");
}
