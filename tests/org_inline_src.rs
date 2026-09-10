//! GitHub #214 / snapper-pdtw: org-element inline src (`src_lang{...}`)
//! and babel calls (`call_name(...)`) stay one inline token. Interior
//! punctuation is not a sentence boundary. The use stays Prose.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    "Use src_python{print(1. 2)} today. Next sentence.\n"
}

#[test]
fn ticket_inline_src_use_stays_prose() {
    let src = "src_python{print(1. 2)}";
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(src) && s.contains("Next sentence.")
        )),
        "inline src must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_src_span_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("src_python{print(1. 2)}"),
        "inline src must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("src_python{print(1.\n") && !out.contains("print(1.\n2)}"),
        "must not split inside the inline src, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("today. Next sentence."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Org,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn ticket_inline_call_stays_one_span() {
    let input = "Use call_name(1. 2) today. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("call_name(1. 2)"),
        "inline babel call must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("call_name(1.\n") && !out.contains("1.\n2)"),
        "must not split inside the inline call, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn inline_src_headers_stay_one_span() {
    let input = "Use src_python[:exports code]{print(1. 2)} today. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("src_python[:exports code]{print(1. 2)}"),
        "inline src with headers must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn inline_src_after_word_underscore_is_not_an_object() {
    // org-element--object-lex matches subscript first, so
    // `foo_src_python{...}` is not an inline-src-block. `_src_python`
    // after whitespace still is. `asrc_python` is not.
    let input = "See foo_src_python{print(1. 2)} today. Next sentence.\n";
    let src = "src_python{print(1. 2)}";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(src) && s.contains("Next sentence.")
        )),
        "foo_src_ must stay Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("today.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

    let wrap_cfg = FormatConfig {
        format: Format::Org,
        max_width: 20,
        ..Default::default()
    }
    .without_safety_backstops();
    let wrapped = format_text(input, &wrap_cfg).unwrap();
    assert!(
        !wrapped.lines().any(|l| l.contains(src)),
        "foo_src_ leftover braces must not stay one wrap token, got:\n{wrapped}"
    );
    assert!(
        wrapped.contains("1.\n2"),
        "interior period in leftover braces may wrap, got:\n{wrapped}"
    );

    let leading = format_text(
        "See _src_python{print(1. 2)} today. Next sentence.\n",
        &org_cfg(),
    )
    .unwrap();
    assert!(
        leading.contains("See _src_python{print(1. 2)} today.\nNext sentence."),
        "_src_ after whitespace is still an object, got:\n{leading}"
    );

    let unmatched = format_text(
        "asrc_python{print(1. 2)} today. Next sentence.\n",
        &org_cfg(),
    )
    .unwrap();
    assert!(
        unmatched.contains("today.\nNext sentence."),
        "asrc_ is not an object; Next sentence. still splits, got:\n{unmatched}"
    );
}
