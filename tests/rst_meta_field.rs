//! snapper-10kg / GitHub #434: Docutils `meta` field body is leftover
//! Prose. Field marker stays Structure; same-line After. still splits.
//! Flush After. / Next. still split. Bibliographic `:Author:` and note
//! `:class:` stay whole-line Structure.

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

/// Ticket fixture (Format::Rst / GitHub #434).
fn ticket_fixture() -> &'static str {
    concat!(
        ".. meta::\n",
        "   :description: fig. 1 is here. After.\n",
        "After. Next.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        ".. meta::\n",
        "   :description: fig. 1 is here.\n",
        "                 After.\n",
        "After.\n",
        "Next.\n",
    )
}

#[test]
fn meta_opener_is_structure_field_marker_is_structure_body_is_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == ".. meta::")),
        "opener .. meta:: must stay Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "   :description: ")),
        "field marker :description: must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("fig. 1 is here.") && s.contains("After.")
        )),
        "meta field body must be leftover Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("fig. 1 is here")
        )),
        "meta field body must not stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_hangs_and_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert!(
        !out.contains(":description: fig. 1 is here. After."),
        "meta field body must not stay one line, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext.\n"),
        "flush After. / Next. must stay split, got:\n{out}"
    );
    assert!(
        !out.contains("\n                 After.\n                 After."),
        "flush After. must not inherit the meta field hang, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "split meta field body must be identity, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn bibliographic_author_stays_whole_line_structure() {
    let input = ":Author: Someone\n";
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":Author: Someone"))),
        ":Author: Someone must stay whole-line Structure, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "valued bibliographic field must hang as today, got:\n{out}"
    );
}

/// snapper-gaxz: flush `:Author:` after `.. meta::` + blank is a
/// top-level bibliographic field, not a hung meta leftover.
#[test]
fn flush_author_after_meta_stays_whole_line_structure() {
    let input = concat!(
        ".. meta::\n",
        "   :description: fig. 1 is here. After.\n",
        "\n",
        ":Author: Jane Doe. Also here.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(":Author: Jane Doe. Also here.")
        )),
        "flush :Author: after meta must stay whole-line Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("Jane Doe"))),
        "flush :Author: body must not become leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains(":Author: Jane Doe. Also here.\n"),
        "flush :Author: must stay one Structure line, got:\n{out}"
    );
    assert!(
        !out.contains("                 Also here."),
        "Also. must not hang as meta leftover, got:\n{out}"
    );
}

/// snapper-omso: after the blank that ends the meta block, a later
/// valued field list is bibliographic Structure. `Next.` does not hang.
#[test]
fn flush_author_after_meta_blank_does_not_hang_next() {
    let input = concat!(
        ".. meta::\n",
        "   :description: fig. 1 is here. After.\n",
        "\n",
        ":Author: Jane Doe wrote this. Next.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(":Author: Jane Doe wrote this. Next.")
        )),
        "flush :Author: after meta blank must stay whole-line Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Jane Doe") || s.contains("Next.")
        )),
        "flush :Author: after meta blank must not hang Next. as leftover, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains(":Author: Jane Doe wrote this. Next.\n"),
        "flush :Author: must stay one line; Next. must not split, got:\n{out}"
    );
}

/// Indented blank-separated meta fields stay leftover Prose.
#[test]
fn indented_keywords_after_interior_blank_still_hang() {
    let input = concat!(
        ".. meta::\n",
        "   :description: fig. 1 is here. After.\n",
        "\n",
        "   :keywords: more here. Next.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "   :keywords: ")),
        "indented :keywords: marker must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("more here.") && s.contains("Next.")
        )),
        "indented :keywords: after interior blank must stay leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    let marker = "   :keywords: ";
    let hang = " ".repeat(marker.len());
    assert!(
        !out.contains(":keywords: more here. Next."),
        "indented :keywords: body must still split, got:\n{out}"
    );
    assert!(
        out.contains(&format!("{marker}more here.\n{hang}Next.\n")),
        "indented :keywords: must hang Next. under the marker, got:\n{out}"
    );
}

#[test]
fn note_class_option_stays_whole_line_structure() {
    let input = ".. note::\n  :class: test\n\n  Body.\n";
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":class: test"))),
        "note :class: option must stay whole-line Structure, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("\n  :class: test"),
        "note :class: must keep two-space indent, got:\n{out}"
    );
}

#[test]
fn header_footer_replace_unchanged() {
    for name in ["header", "footer"] {
        let opener = format!(".. {name}:: ");
        let hang = " ".repeat(opener.len());
        let input = format!(".. {name}:: fig. 1 is here. After.\nAfter. Next.\n");
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert_eq!(
            out,
            format!("{opener}fig. 1 is here.\n{hang}After.\nAfter.\nNext.\n"),
            "{name} same-line leftover must stay, got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }

    let replace = concat!(
        "See |v|. Next.\n",
        "\n",
        ".. |v| replace:: fig. 1 is here. After.\n",
    );
    let replace_out = format_text(replace, &rst_cfg()).unwrap();
    assert_eq!(
        replace_out,
        concat!(
            "See |v|.\n",
            "Next.\n",
            "\n",
            ".. |v| replace:: fig. 1 is here.\n",
            "                 After.\n",
        ),
        "replace:: leftover walker must stay, got:\n{replace_out}"
    );
}
