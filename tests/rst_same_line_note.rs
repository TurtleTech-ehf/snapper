//! GitHub #349 / snapper-lo9y: Docutils note nested-parses same-line
//! text after `::`. The opener is Structure; the two sentences stay
//! Prose and still split. Indented note bodies already hang.
//! Isolate after #347 (unmatched simple-table top).

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

/// Ticket fixture (Format::Rst / GitHub #349).
fn ticket_fixture() -> &'static str {
    ".. note:: This is a long note sentence that must reflow. Second sentence.\n"
}

fn expected_ticket() -> &'static str {
    concat!(
        ".. note:: This is a long note sentence that must reflow.\n",
        "          Second sentence.\n",
    )
}

#[test]
fn same_line_note_opener_is_structure_body_is_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ".. note:: ")),
        "opener .. note:: must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long note sentence that must reflow.")
                    && s.contains("Second sentence.")
        )),
        "same-line note body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a long note sentence")
        )),
        "same-line note body must not stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_hangs_and_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn indented_note_body_still_hangs() {
    let input = concat!(
        ".. note::\n",
        "\n",
        "   This is a long note sentence that must reflow. Second sentence.\n",
        "\n",
        "After the note. Next.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
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
        "indented note body from #54 must stay, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn compact_note_flush_paragraph_still_stays_unindented() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        ".. note::\n",
        "   Body here. Second body.\n",
        "After markup. Next sentence.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            ".. note::\n",
            "   Body here.\n",
            "   Second body.\n",
            "After markup.\n",
            "Next sentence.\n",
        ),
        "#344 compact note must stay, got:\n{out}"
    );
    assert!(
        !out.contains("\n   After markup."),
        "After markup must stay flush, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn same_line_note_does_not_swallow_flush_paragraph() {
    let input = concat!(
        ".. note:: This is a long note sentence that must reflow. Second sentence.\n",
        "After markup. Next sentence.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. note:: This is a long note sentence that must reflow.\n",
            "          Second sentence.\n",
            "After markup.\n",
            "Next sentence.\n",
        ),
        "After markup must stay flush after same-line note, got:\n{out}"
    );
    assert!(
        !out.contains("\n          After markup."),
        "After markup must not inherit the note hang, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn leftover_specific_admonitions_same_line_hang() {
    for name in [
        "warning",
        "caution",
        "danger",
        "tip",
        "important",
        "hint",
        "error",
        "attention",
    ] {
        let opener = format!(".. {name}:: ");
        let hang = " ".repeat(opener.len());
        let input = format!(
            ".. {name}:: This is a long note sentence that must reflow. Second sentence.\n"
        );
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert_eq!(
            out,
            format!(
                "{opener}This is a long note sentence that must reflow.\n{hang}Second sentence.\n"
            ),
            "{name} same-line body must hang and split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
}

#[test]
fn figure_same_line_argument_stays_structure() {
    let input = ".. figure:: image.png\n\n   This is a long note sentence that must reflow. Second sentence.\n";
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(".. figure:: image.png"))),
        "figure URI argument must stay Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("image.png"))),
        "figure URI must not become Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains(".. figure:: image.png\n"),
        "figure opener must stay identity, got:\n{out}"
    );
    assert!(
        out.contains("   This is a long note sentence that must reflow.\n   Second sentence.\n"),
        "figure body must still hang, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn closed_simple_table_stays_identity() {
    let input = concat!(
        "=====  =====\n",
        "Name   Value\n",
        "=====  =====\n",
        "A      B\n",
        "=====  =====\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "#347 closed simple table must stay, got:\n{out}"
    );
}

#[test]
fn opaque_directives_stay_frozen() {
    for (label, input, needle) in [
        (
            "raw",
            ".. raw:: html\n\n   <p>This is a long note sentence that must reflow. Second sentence.</p>\n",
            "<p>This is a long note sentence that must reflow. Second sentence.</p>",
        ),
        (
            "code-block",
            ".. code-block:: python\n\n   print(\"This is a long note sentence that must reflow. Second sentence.\")\n",
            "print(\"This is a long note sentence that must reflow. Second sentence.\")",
        ),
    ] {
        let regions = RstParser.parse(input);
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains("This is a long note"))),
            "{label} body must not be Prose, got {regions:?}"
        );
        let out = format_text(input, &rst_cfg()).unwrap();
        assert!(
            out.contains(needle),
            "{label} body must stay opaque, got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
}
