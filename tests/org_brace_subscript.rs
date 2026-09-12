//! GitHub #338 / snapper-upgw: org-match-substring-regexp brace
//! subscript/superscript (`H_{2. 0}` / `x^{n. 1}`) stay one wrap token.
//! Interior punct is not a wrap boundary. `Next.` still splits.
//! Distinct from latex-fragment (snapper-e6tw).

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn org_wrap10() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 10,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    "See H_{2. 0} today. Next.\n"
}

#[test]
fn ticket_brace_subscript_stays_prose() {
    let token = "H_{2. 0}";
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(token) && s.contains("Next.")
        )),
        "brace subscript must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_brace_span_and_splits_next() {
    let input = ticket_fixture();
    let token = "H_{2. 0}";
    let out = format_text(input, &org_wrap10()).unwrap();
    assert!(
        out.lines().any(|l| l.contains(token)),
        "brace form must stay one wrap token, got:\n{out}"
    );
    assert!(
        !out.contains("2.\n0") && !out.contains("H_{2.\n"),
        "must not wrap on the interior period, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("today. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_wrap10()).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Org,
        max_width: 10,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn ticket_brace_superscript_stays_one_wrap_token() {
    let input = "See x^{n. 1} today. Next.\n";
    let out = format_text(input, &org_wrap10()).unwrap();
    assert!(
        out.lines().any(|l| l.contains("x^{n. 1}")),
        "brace superscript must stay one wrap token, got:\n{out}"
    );
    assert!(
        !out.contains("n.\n1") && !out.contains("x^{n.\n"),
        "must not wrap on the interior period, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_wrap10()).unwrap(), out);
}

#[test]
fn ticket_paren_form_stays_one_wrap_token() {
    let input = "See H_(2. 0) today. Next.\n";
    let out = format_text(input, &org_wrap10()).unwrap();
    assert!(
        out.lines().any(|l| l.contains("H_(2. 0)")),
        "paren form must stay one wrap token, got:\n{out}"
    );
    assert!(
        !out.contains("H_(2.\n") && !out.contains("2.\n0)"),
        "must not wrap-split inside the paren form, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_wrap10()).unwrap(), out);
}

#[test]
fn no_space_brace_subscript_unchanged() {
    let input = "See H_{2.0} today. Next.\n";
    let out = format_text(input, &org_wrap10()).unwrap();
    assert!(
        out.lines().any(|l| l.contains("H_{2.0}")),
        "no-space H_{{2.0}} must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_wrap10()).unwrap(), out);
}
