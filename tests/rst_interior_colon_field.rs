//! GitHub #341 / snapper-1n4x: Docutils `Body.patterns` field_marker is
//! `:(?![: ])([^:\\]|\\.|:(?!([ `]|$)))*(?<! ):( +|$)`. Interior colons
//! are allowed (`:py:mod: name`). Distinct from snapper-pn97 / GH #330
//! (empty `:name:` at EOL). `:role:`text`` stays interpreted text.

use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Rst / GitHub #341).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        ":py:mod: some.module.Name is here. Second sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        ":py:mod: some.module.Name is here.\n",
        "         Second sentence.\n",
    )
}

#[test]
fn interior_colon_field_marker_is_structure() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ":py:mod: ")),
        ":py:mod: must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("some.module.Name is here.") && s.contains("Second sentence.")
        )),
        "field body must stay Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(":py:mod:") && s.contains("Intro sentence")
        )),
        "field must not stay one Prose region with the intro, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_hangs_field_and_splits_second() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn interpreted_text_role_is_not_a_field() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        ":role:`text` stays interpreted. Next sentence.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(":role:`text`")
        )),
        "role must not be a field-list Structure, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            ":role:`text` stays interpreted.\n",
            "Next sentence.\n",
        ),
        "role line must stay prose and split, got:\n{out}"
    );
}

#[test]
fn empty_name_field_still_structure() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        ":name:\n",
        "After field. Next sentence.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == ":name:")),
        "empty :name: is snapper-pn97, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            ":name:\n",
            "After field.\n",
            "Next sentence.\n",
        ),
        "empty :name: must stay Structure and following prose split, got:\n{out}"
    );
}
