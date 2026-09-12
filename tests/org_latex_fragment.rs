//! GitHub #336 / snapper-e6tw: org-element latex-fragment wrap must
//! not split on interior punctuation. `\( x = 1. \)` and `$a. b$` stay
//! atomic. Interior backslash (`\(\alpha. \beta\)`) and optional `[arg]`
//! (`\sqrt[2]{a. b}`) must too. The fragment stays one token; `Next.`
//! still splits. Same-line `\[ \]` Structure is unchanged.
//! Brace subscript `H_{2. 0}` is snapper-upgw, not this ticket.

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

fn wrap_cfg(width: usize) -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: width,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    "The root is \\(\\alpha. \\beta\\) today. Next.\n"
}

#[test]
fn ticket_fragment_stays_prose() {
    let frag = "\\(\\alpha. \\beta\\)";
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(frag) && s.contains("Next.")
        )),
        "latex fragment must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_wrap_keeps_fragment_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &wrap_cfg(12)).unwrap();
    assert!(
        out.contains("\\(\\alpha. \\beta\\)"),
        "fragment must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("\\(\\alpha.\n") && !out.contains("alpha.\n\\beta"),
        "must not wrap inside the fragment, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("today. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg(12)).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Org,
        max_width: 12,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn paren_and_dollar_without_interior_backslash_stay_atomic() {
    let cfg = org_cfg();
    let paren = "See \\( x = 1. \\) here. Next.\n";
    let out = format_text(paren, &cfg).unwrap();
    assert!(
        out.contains("\\( x = 1. \\)"),
        "\\( x = 1. \\) must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("here.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);

    let dollar = "See $a. b$ here. Next.\n";
    let out = format_text(dollar, &cfg).unwrap();
    assert!(
        out.contains("$a. b$"),
        "$a. b$ must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("here.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}

#[test]
fn sqrt_optional_arg_stays_one_span() {
    let input = "See \\sqrt[2]{a. b} today. Next.\n";
    let frag = "\\sqrt[2]{a. b}";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(frag) && s.contains("Next.")
        )),
        "\\sqrt[2]{{a. b}} must stay Prose, got {regions:?}"
    );
    let out = format_text(input, &wrap_cfg(12)).unwrap();
    assert!(
        out.contains(frag),
        "\\sqrt[2]{{a. b}} must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("\\sqrt[2]{a.\n") && !out.contains("a.\nb}"),
        "must not wrap inside \\sqrt[2]{{a. b}}, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg(12)).unwrap(), out);
}

#[test]
fn same_line_bracket_display_stays_structure() {
    let input = "The root is \\[ x = 1. \\] today. Next.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("x = 1."))),
        "same-line \\[ \\] must stay Structure, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("x = 1."))),
        "same-line \\[ body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("\\[ x = 1. \\] today.\nNext."),
        "same-line \\[ \\] Structure unchanged; Next. still splits, got:\n{out}"
    );
    assert!(
        !out.contains("\\[ x = 1.\n"),
        "must not break after the math period, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
