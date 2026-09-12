//! snapper-gqjv / GitHub #422: RST leftover body.py header / footer
//! same-line body is leftover Prose. Marker stays Structure; After.
//! hangs and splits. Flush After. / Next. still split. epigraph /
//! replace:: unchanged.

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

/// Ticket fixture (Format::Rst / GitHub #422).
fn header_fixture() -> &'static str {
    concat!(".. header:: fig. 1 is here. After.\n", "After. Next.\n",)
}

fn footer_fixture() -> &'static str {
    concat!(".. footer:: fig. 1 is here. After.\n", "After. Next.\n",)
}

fn expected_header() -> &'static str {
    concat!(
        ".. header:: fig. 1 is here.\n",
        "            After.\n",
        "After.\n",
        "Next.\n",
    )
}

fn expected_footer() -> &'static str {
    concat!(
        ".. footer:: fig. 1 is here.\n",
        "            After.\n",
        "After.\n",
        "Next.\n",
    )
}

#[test]
fn header_opener_is_structure_body_is_prose() {
    let regions = RstParser.parse(header_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ".. header:: ")),
        "opener .. header:: must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("fig. 1 is here.") && s.contains("After.")
        )),
        "same-line header body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("fig. 1 is here")
        )),
        "same-line header body must not stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn footer_opener_is_structure_body_is_prose() {
    let regions = RstParser.parse(footer_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ".. footer:: ")),
        "opener .. footer:: must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("fig. 1 is here.") && s.contains("After.")
        )),
        "same-line footer body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("fig. 1 is here")
        )),
        "same-line footer body must not stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn ticket_header_fixture_hangs_and_splits() {
    let input = header_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_header(), "got:\n{out}");
    assert!(
        !out.contains(".. header:: fig. 1 is here. After."),
        "same-line header body must not stay one line, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext.\n"),
        "flush After. / Next. must stay split, got:\n{out}"
    );
    assert!(
        !out.contains("\n            After.\n            Next."),
        "flush After. / Next. must not inherit the header hang, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "split header body must be identity, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn ticket_footer_fixture_hangs_and_splits() {
    let input = footer_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_footer(), "got:\n{out}");
    assert!(
        !out.contains(".. footer:: fig. 1 is here. After."),
        "same-line footer body must not stay one line, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext.\n"),
        "flush After. / Next. must stay split, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "split footer body must be identity, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn leftover_header_footer_names_same_line_hang() {
    for name in ["header", "footer"] {
        let opener = format!(".. {name}:: ");
        let hang = " ".repeat(opener.len());
        let input = format!(".. {name}:: fig. 1 is here. After.\nAfter. Next.\n");
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert_eq!(
            out,
            format!("{opener}fig. 1 is here.\n{hang}After.\nAfter.\nNext.\n"),
            "{name} same-line body must hang and split; flush After. / Next. stay flush, got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
}

#[test]
fn indented_header_body_still_hangs() {
    let input = concat!(
        ".. header::\n",
        "\n",
        "   fig. 1 is here. After.\n",
        "\n",
        "After. Next.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. header::\n",
            "\n",
            "   fig. 1 is here.\n",
            "   After.\n",
            "\n",
            "After.\n",
            "Next.\n",
        ),
        "indented header body must hang and split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn epigraph_leftover_walker_unchanged() {
    let input = concat!(
        ".. epigraph::\n",
        "\n",
        "   This is a long note sentence that must reflow. Second sentence.\n",
        "After. Next.\n",
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
        "epigraph leftover walker must stay, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn substitution_replace_leftover_walker_unchanged() {
    let input = concat!(
        "See |v|. Next.\n",
        "\n",
        ".. |v| replace:: fig. 1 is here. After.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "See |v|.\n",
            "Next.\n",
            "\n",
            ".. |v| replace:: fig. 1 is here.\n",
            "                 After.\n",
        ),
        "replace:: leftover walker must stay, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}
