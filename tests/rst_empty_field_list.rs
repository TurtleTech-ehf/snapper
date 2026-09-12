//! GitHub #330 / snapper-pn97: Docutils `Body.patterns` field_marker is
//! `:(?![: ])...:( +|$)`. Lone `:name:` at EOL is Structure; following
//! prose still splits. `:name: value` with a trailing space hangs as
//! today. `:role:`text`` stays interpreted text, not a field.

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

/// Ticket fixture (Format::Rst / GitHub #330).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        ":name:\n",
        "After field. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        ":name:\n",
        "After field.\n",
        "Next sentence.\n",
    )
}

#[test]
fn empty_field_marker_is_structure() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == ":name:")),
        "lone :name: at EOL must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains(":name:") && p.contains("After field.")
        )),
        "empty :name: must not join the following prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After field.") && p.contains("Next sentence.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_empty_field_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn valued_field_with_trailing_space_hangs() {
    let input = ":name: value \n";
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":name: value"))),
        ":name: value with trailing space must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        ":name: value with trailing space must hang as today, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn valued_field_without_trailing_space_hangs() {
    let input = ":Author: Someone\n";
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, input, "valued field must hang as today, got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn interpreted_text_role_is_not_a_field() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        ":class:`CloudDatabase` exceeds a rate. Next sentence.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(":class:`CloudDatabase`")
        )),
        "role must not be a field-list Structure, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            ":class:`CloudDatabase` exceeds a rate.\n",
            "Next sentence.\n",
        ),
        "role line must stay prose and split, got:\n{out}"
    );
}
