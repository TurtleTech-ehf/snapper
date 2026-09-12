//! GitHub #335 / snapper-rhqk: Emacs 30.2 org-link-angle-re allows
//! spaces, so `<file:fig. 1.png>` stays one inline token. Interior
//! punctuation is not a sentence or wrap boundary. Bracket
//! `[[file:fig. 1.png]]` stays atomic. `Next.` still splits.

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
    "See <file:fig. 1.png> today. Next.\n"
}

#[test]
fn ticket_angle_link_use_stays_prose() {
    let link = "<file:fig. 1.png>";
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(link) && s.contains("Next.")
        )),
        "angle link must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_angle_link_span_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("<file:fig. 1.png>"),
        "angle link must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("<file:fig.\n") && !out.contains("fig.\n1.png>"),
        "must not split inside the angle link, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("today. Next."),
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
fn bracket_file_link_stays_one_span() {
    let input = "See [[file:fig. 1.png]] today. Next.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("[[file:fig. 1.png]]"),
        "bracket link must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("fig.\n1.png"),
        "must not split inside the bracket link, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn angle_link_stays_atomic_under_wrap() {
    let input = ticket_fixture();
    let cfg = FormatConfig {
        format: Format::Org,
        max_width: 16,
        ..Default::default()
    }
    .without_safety_backstops();
    let out = format_text(input, &cfg).unwrap();
    assert!(
        out.contains("<file:fig. 1.png>"),
        "angle link must stay one token under wrap, got:\n{out}"
    );
    assert!(
        !out.contains("fig.\n") && !out.contains("fig. \n"),
        "must not wrap inside the angle link, got:\n{out}"
    );
    assert!(
        out.contains("Next."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}
