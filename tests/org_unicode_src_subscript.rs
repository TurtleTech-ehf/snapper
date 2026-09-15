//! org-element `[:alnum:]` is Unicode. `café_src_python{...}` is a
//! subscript; leftover braces are prose. Space `_src_` and `foo-src_`
//! stay objects. `H_{2. 0}` is unchanged.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn wrap_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 20,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn unicode_letter_underscore_src_is_subscript() {
    let input = "See café_src_python{print(1. 2)} today. Next sentence.\n";
    let src = "src_python{print(1. 2)}";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(src) && s.contains("Next sentence.")
        )),
        "café_src leftover must stay Prose, got {regions:?}"
    );
    let wrapped = format_text(input, &wrap_cfg()).unwrap();
    assert!(
        !wrapped.lines().any(|l| l.contains(src)),
        "café_src leftover braces must not stay one token, got:\n{wrapped}"
    );
    assert!(
        wrapped.contains("1.\n2"),
        "interior period may wrap after Unicode-letter subscript, got:\n{wrapped}"
    );
    assert!(
        wrapped.contains("today.\nNext sentence."),
        "following sentence must still split, got:\n{wrapped}"
    );
    assert_eq!(format_text(&wrapped, &wrap_cfg()).unwrap(), wrapped);
}

#[test]
fn space_and_hyphen_src_stay_objects() {
    let src = "src_python{print(1. 2)}";
    for input in [
        "See _src_python{print(1. 2)} today. Next sentence.\n",
        "See foo-src_python{print(1. 2)} today. Next sentence.\n",
    ] {
        let out = format_text(input, &org_cfg()).unwrap();
        assert!(
            out.contains(src),
            "space _src_ / foo-src_ stay objects, got:\n{out}"
        );
        assert!(
            !out.contains("print(1.\n2)}") && !out.contains("src_python{print(1.\n"),
            "must not split inside src_ after space or hyphen, got:\n{out}"
        );
        assert!(
            out.contains("today.\nNext sentence."),
            "following sentence must still split, got:\n{out}"
        );
    }
}

#[test]
fn brace_subscript_unchanged() {
    let brace = format_text("See H_{2. 0} today. Next.\n", &wrap_cfg()).unwrap();
    assert!(
        brace.lines().any(|l| l.contains("H_{2. 0}")),
        "brace subscript H_{{2. 0}} must stay one token, got:\n{brace}"
    );
    assert!(
        !brace.contains("2.\n0") && !brace.contains("H_{2.\n"),
        "must not wrap inside H_{{2. 0}}, got:\n{brace}"
    );
}
