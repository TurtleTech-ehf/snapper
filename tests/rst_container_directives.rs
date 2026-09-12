//! RST container directive bodies hang and reflow.
//! Docutils admonitions/figure/topic/sidebar nested-parse their body.
//! Leftover body.py parsed-literal (GitHub #386) and epigraph / highlights /
//! pull-quote / compound (GitHub #351) hang and split like note. Option fields
//! stay Structure; code-block/raw/include/csv-table stay opaque.

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
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
        "note body hang spaces must be Structure, got {regions:?}"
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


/// Ticket fixture (Format::Rst / GitHub #386): leftover body.py
/// parsed-literal hangs and splits; flush After. / Next. stay unindented.
fn leftover_parsed_literal_fixture() -> &'static str {
    concat!(
        ".. parsed-literal::\n",
        "\n",
        "   This is a long note sentence that must reflow. Second sentence.\n",
        "After. Next.\n",
    )
}

#[test]
fn leftover_parsed_literal_fixture_hangs_and_splits() {
    let input = leftover_parsed_literal_fixture();
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(".. parsed-literal::"))),
        "parsed-literal opener must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long note sentence that must reflow.")
                    && s.contains("Second sentence.")
        )),
        "parsed-literal body must be hung Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a long note sentence")
        )),
        "parsed-literal body must not freeze as Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
        "parsed-literal body hang spaces must be Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("After.") && s.contains("Next."))),
        "After. / Next. must stay unindented Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("Second sentence.") && s.contains("After.")
        )),
        "After. must not join the parsed-literal body, got {regions:?}"
    );

    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. parsed-literal::\n",
            "\n",
            "   This is a long note sentence that must reflow.\n",
            "   Second sentence.\n",
            "After.\n",
            "Next.\n",
        ),
        "parsed-literal body must hang and split; After. / Next. stay flush, got:\n{out}"
    );
    assert!(
        !out.contains("\n   After."),
        "After. must not inherit the parsed-literal hang, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);

/// Ticket fixture (Format::Rst / GitHub #351): leftover body.py
/// containers hang and split; flush After. / Next. stay unindented.
fn leftover_epigraph_fixture() -> &'static str {
    concat!(
        ".. epigraph::\n",
        "\n",
        "   This is a long note sentence that must reflow. Second sentence.\n",
        "After. Next.\n",
    )
}

#[test]
fn leftover_epigraph_fixture_hangs_and_splits() {
    let input = leftover_epigraph_fixture();
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(".. epigraph::"))),
        "epigraph opener must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long note sentence that must reflow.")
                    && s.contains("Second sentence.")
        )),
        "epigraph body must be hung Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a long note sentence")
        )),
        "epigraph body must not freeze as Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "   ")),
        "epigraph body hang spaces must be Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("After.") && s.contains("Next."))),
        "After. / Next. must stay unindented Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("Second sentence.") && s.contains("After.")
        )),
        "After. must not join the epigraph body, got {regions:?}"
    );

    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. epigraph::\n",
            "\n",
            "   This is a long note sentence that must reflow.\n",
            "   Second sentence.\n",
            "After.\n",
            "Next.\n",
        ),
        "epigraph body must hang and split; After. / Next. stay flush, got:\n{out}"
    );
    assert!(
        !out.contains("\n   After."),
        "After. must not inherit the epigraph hang, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

/// GitHub #351 leftover walker: body.py names hang and split like
/// note. After. / Next. stay unindented. One walker for the leftover
/// class; code-block / raw stay opaque in `opaque_directives_stay_frozen`.
#[test]
fn leftover_body_py_container_names_reflow_like_note() {
    for name in ["epigraph", "highlights", "pull-quote", "compound", "parsed-literal"] {
        let input = format!(
            ".. {name}::\n\n   This is a long note sentence that must reflow. Second sentence.\nAfter. Next.\n"
        );
        let regions = RstParser.parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("This is a long note sentence that must reflow.")
                        && s.contains("Second sentence.")
            )),
            "{name} body must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("This is a long note sentence")
            )),
            "{name} body must not freeze as Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(s)
                    if s.contains("Second sentence.") && s.contains("After.")
            )),
            "After. must not join the {name} body, got {regions:?}"
        );
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert_eq!(
            out,
            format!(
                ".. {name}::\n\n   This is a long note sentence that must reflow.\n   Second sentence.\nAfter.\nNext.\n"
            ),
            "{name} hangs and splits like note; After. / Next. stay flush, got:\n{out}"
        );
        assert!(
            !out.contains("\n   After."),
            "After. after {name} must stay flush, got:\n{out}"
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
