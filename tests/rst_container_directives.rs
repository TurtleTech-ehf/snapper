//! RST container directive bodies hang and reflow.
//! Docutils admonitions / figure / topic / sidebar / container nested-parse
//! their body. Openers and option fields stay Structure; code-block / raw /
//! include / csv-table stay opaque.

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

/// Ticket fixture (Format::Rst).
fn ticket_fixture() -> &'static str {
    concat!(
        ".. note::\n",
        "\n",
        "   This is a long note sentence that must reflow. Second sentence.\n",
        "\n",
        "After the note. Next.\n",
    )
}

#[test]
fn note_opener_is_structure_body_hangs_and_splits() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(".. note::"))),
        "note opener must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long note sentence that must reflow.")
                    && s.contains("Second sentence.")
        )),
        "note body must be hung Prose, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
        "note body hang spaces must be Structure, got {regions:?}"
    );
    let out = format_text(ticket_fixture(), &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. note::\n",
            "\n",
            "   This is a long note sentence that must reflow.\n",
            "   Second sentence.\n",
            "\n",
            "After the note.\n",
            "Next.\n",
        ),
        "note body must hang and split; following prose must reflow, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn option_field_stays_structure() {
    let input = concat!(".. note::\n", "   :class: test\n", "\n", "   Body.\n",);
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":class: test"))),
        "option field must stay Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("Body."))),
        "body after the option must be Prose, got {regions:?}"
    );
}

#[test]
fn leftover_container_names_reflow_like_note() {
    for name in [
        "warning",
        "caution",
        "danger",
        "tip",
        "important",
        "hint",
        "error",
        "attention",
        "admonition",
        "figure",
        "topic",
        "sidebar",
        "container",
    ] {
        let input = format!(
            ".. {name}::\n\n   This is a long note sentence that must reflow. Second sentence.\n\nAfter the note. Next.\n"
        );
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert_eq!(
            out,
            format!(
                ".. {name}::\n\n   This is a long note sentence that must reflow.\n   Second sentence.\n\nAfter the note.\nNext.\n"
            ),
            "{name} is the same container class as note, got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
}

#[test]
fn opaque_directives_stay_identity() {
    for input in [
        ".. raw:: html\n\n   <p>Hello. World.</p>\n",
        ".. include:: other.rst\n   :start-line: 1\n",
        ".. csv-table:: Title\n\n   \"a\", \"b. c\"\n",
        ".. code-block:: python\n\n   print(\"Hello. World.\")\n",
    ] {
        let out = format_text(input, &rst_cfg()).unwrap();
        assert_eq!(
            out, input,
            "opaque directive body must stay identity, got:\n{out}"
        );
        if input.contains("Hello. World.") {
            assert!(
                !out.contains("Hello.\n"),
                "opaque body must not split, got:\n{out}"
            );
        }
    }
}
