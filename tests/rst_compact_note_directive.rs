//! GitHub #344 / snapper-ut5c: compact `.. note::` plus indented body
//! plus a flush paragraph. Docutils ends the directive at the first
//! less-indented line, so After markup stays column-0 Prose and still
//! splits. wr17 / #343 empty `..` is already on main.

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

/// Ticket fixture (Format::Rst / GitHub #344).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        ".. note::\n",
        "   Body here. Second body.\n",
        "After markup. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        ".. note::\n",
        "   Body here.\n",
        "   Second body.\n",
        "After markup.\n",
        "Next sentence.\n",
    )
}

#[test]
fn compact_note_opener_is_structure_body_hangs() {
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
            Region::Structure(s) if s == "   "
        )),
        "note body hang spaces must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Body here.") && s.contains("Second body.")
        )),
        "note body must hang as Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Body here.")
        )),
        "note body must not freeze as Structure, got {regions:?}"
    );
}

#[test]
fn compact_note_does_not_swallow_flush_paragraph() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Body here.") && s.contains("After markup.")
        )),
        "After markup must not join the note body, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("After markup.") && s.contains("Next sentence.")
        )),
        "After markup must stay unindented Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("After markup.")
        )),
        "After markup must not become Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_note_hang_and_splits_neighbors() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert!(
        !out.contains("\n   After markup."),
        "After markup must stay flush, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn blank_separated_note_still_hangs() {
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
        "blank-separated note from #54 must stay, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn leftover_container_names_do_not_swallow_flush_prose() {
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
        "epigraph",
        "highlights",
        "pull-quote",
        "compound",
    ] {
        let arg = if matches!(name, "figure" | "admonition" | "sidebar" | "topic") {
            " Title"
        } else {
            ""
        };
        let input = format!(
            "Intro sentence here. Another intro sentence.\n.. {name}::{arg}\n   Body here. Second body.\nAfter markup. Next sentence.\n"
        );
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert!(
            out.contains("   Body here.\n   Second body.\n"),
            "{name} body must hang and split, got:\n{out}"
        );
        assert!(
            out.contains("After markup.\nNext sentence.\n"),
            "After markup after {name} must stay flush and split, got:\n{out}"
        );
        assert!(
            !out.contains("\n   After markup."),
            "After markup after {name} must not inherit the hang, got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
}

#[test]
fn empty_explicit_comment_still_splits() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        "..\n",
        "After markup. Next sentence.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            "..\n",
            "After markup.\n",
            "Next sentence.\n",
        ),
        "wr17 #343 empty .. must stay, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}
