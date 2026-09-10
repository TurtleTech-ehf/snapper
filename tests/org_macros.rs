//! GitHub #212 / snapper-aunh: org-element macros (`{{{name}}}` /
//! `{{{name(args)}}}`) stay one inline token. Interior punctuation is
//! not a sentence boundary. The use stays Prose.

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
    "See {{{cite(Smith. 2020)}}} for the source. Next sentence.\n"
}

#[test]
fn ticket_macro_use_stays_prose() {
    let mac = "{{{cite(Smith. 2020)}}}";
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(mac) && s.contains("Next sentence.")
        )),
        "macro must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_macro_span_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("{{{cite(Smith. 2020)}}}"),
        "macro must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("{{{cite(Smith.\n") && !out.contains("Smith.\n2020}}}"),
        "must not split inside the macro, got:\n{out}"
    );
    assert!(
        out.contains("for the source.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("for the source. Next sentence."),
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
fn no_arg_macro_stays_one_span() {
    let input = "See {{{title}}} for the source. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("{{{title}}}"),
        "no-arg macro must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("for the source.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
