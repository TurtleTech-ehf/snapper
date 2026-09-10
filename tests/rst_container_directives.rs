//! snapper-q88q: RST container directive bodies hang and reflow.
//! Docutils admonitions/figure/topic/sidebar nested-parse their body.
//! Option fields stay Structure; code-block/raw/include/csv-table stay opaque.

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
fn note_container_fixture() -> &'static str {
    concat!(
        ".. note::\n",
        "\n",
        "   This is a long note sentence that must reflow. Second sentence.\n",
        "\n",
        "After the note. Next.\n",
    )
}

#[test]
fn note_container_body_hangs_and_splits() {
    let input = note_container_fixture();
    let regions = RstParser.parse(input);
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
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a long note sentence")
        )),
        "note body must not freeze as Structure, got {regions:?}"
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
        "note body must hang and split; following prose must reflow, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
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
fn container_option_field_stays_structure() {
    let input = concat!(
        ".. note::\n",
        "   :class: test\n",
        "\n",
        "   This is a long note sentence that must reflow. Second sentence.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":class: test"))),
        "option field must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("   :class: test\n"),
        "option field must stay identity, got:\n{out}"
    );
    assert!(
        out.contains("   This is a long note sentence that must reflow.\n   Second sentence.\n"),
        "body after option must hang and split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
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
            "include",
            ".. include:: foo.rst\n   :start-after: This is a long note sentence that must reflow. Second sentence.\n",
            ":start-after: This is a long note sentence that must reflow. Second sentence.",
        ),
        (
            "csv-table",
            ".. csv-table:: Title\n   :header: \"a\", \"b\"\n\n   \"This is a long note sentence that must reflow. Second sentence.\", \"x\"\n",
            "\"This is a long note sentence that must reflow. Second sentence.\", \"x\"",
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
        assert_eq!(
            format_text(&out, &rst_cfg()).unwrap(),
            out,
            "{label} must be identity, got:\n{out}"
        );
    }
}
