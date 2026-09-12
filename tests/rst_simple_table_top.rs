//! GitHub #347 / snapper-asnu: Docutils `simple_table_top` is
//! `=+( +=+)+ *$`. A top with no matching closer is Structure; following
//! flush prose stays Prose and still splits. A closed simple table stays
//! identity.

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

/// Ticket fixture (Format::Rst / GitHub #347).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "=====  =====\n",
        "After table. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "=====  =====\n",
        "After table.\n",
        "Next sentence.\n",
    )
}

fn closed_simple_table() -> &'static str {
    concat!(
        "=====  =====\n",
        "Name   Value\n",
        "=====  =====\n",
        "A      B\n",
        "=====  =====\n",
    )
}

#[test]
fn unclosed_simple_table_top_is_structure() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "=====  =====")),
        "unclosed simple_table_top must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("=====")
        )),
        "unclosed simple_table_top must not fall through to Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("Another intro sentence.") && s.contains("After table.")
        )),
        "top must not join intro prose to following prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Intro sentence here.") && p.contains("Another intro sentence.")
        )),
        "previous paragraph must stay Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After table.") && p.contains("Next sentence.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_top_and_splits_neighbors() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn closed_simple_table_stays_identity() {
    let input = closed_simple_table();
    let regions = RstParser.parse(input);
    for needle in ["=====  =====", "Name   Value", "A      B"] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "closed table line {needle:?} must be Structure, got {regions:?}"
        );
    }
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("Name") || s.contains("A      B") || s.contains("=====")
        )),
        "closed simple table must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "closed simple table must stay identity, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}
