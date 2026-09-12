//! snapper-10kg / GitHub #434: leftover Docutils `meta` field
//! `:description: fig. 1 is here. After.` same-line body is leftover
//! Prose. Field marker stays Structure; After. still splits and hangs.
//! Flush After. / Next. still split. `.. meta::` stays Structure.
//! Top-level `:Author:` and note `:option:` stay whole-line Structure.
//! Flush `:Author:` after `.. meta::` plus a blank stays Structure.
//! Indented `:keywords:` after an interior blank still hangs.

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
fn meta_opener_and_field_marker_are_structure_body_is_prose() {
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
            Region::Prose(s) if s.contains("fig. 1 is here.") && s.contains("After.")
        )),
        "same-line meta field body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("fig. 1 is here")
        )),
        "same-line meta field body must not stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_hangs_and_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert!(
        !out.contains(":description: fig. 1 is here. After."),
        "same-line meta field body must not stay one line, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext.\n"),
        "flush After. / Next. must stay split, got:\n{out}"
    );
    assert!(
        !out.contains("\n                 After.\n                 Next."),
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
fn author_field_stays_whole_line_structure() {
    let input = ":Author: Someone\n";
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":Author: Someone"))),
        "top-level :Author: must stay whole-line Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("Someone"))),
        "top-level :Author: body must not become Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, input, "valued field must hang as today, got:\n{out}");
}

#[test]
fn flush_author_after_meta_blank_stays_structure() {
    // Flush bibliographic field after meta + blank (snapper-gaxz).
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
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Jane Doe") || s.contains("Also here")
        )),
        "flush :Author: body after meta must not become leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains(":Author: Jane Doe. Also here.\n"),
        "flush :Author: must stay one line, got:\n{out}"
    );
    assert!(
        !out.contains("                 Also here."),
        "Also. must not hang as meta leftover, got:\n{out}"
    );
}

#[test]
fn flush_author_after_meta_blank_does_not_split_next() {
    // Later field list after the blank that ends meta (snapper-omso).
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
        "flush :Author: body after meta blank must not become leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. meta::\n",
            "   :description: fig. 1 is here.\n",
            "                 After.\n",
            "\n",
            ":Author: Jane Doe wrote this. Next.\n",
        ),
        "flush :Author: after meta blank must stay whole-line; Next. must not split, got:\n{out}"
    );
}

#[test]
fn indented_keywords_after_meta_blank_still_hangs() {
    // Interior blank does not end indented meta fields.
    let input = concat!(
        ".. meta::\n",
        "   :description: fig. 1 is here. After.\n",
        "\n",
        "   :keywords: more. Also here.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "   :keywords: ")),
        "indented :keywords: marker after meta blank must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("more.") && s.contains("Also here.")
        )),
        "indented :keywords: body after meta blank must hang as leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. meta::\n",
            "   :description: fig. 1 is here.\n",
            "                 After.\n",
            "\n",
            "   :keywords: more.\n",
            "              Also here.\n",
        ),
        "indented :keywords: after interior blank must hang and split, got:\n{out}"
    );
}

#[test]
fn less_indented_field_after_meta_stays_structure() {
    let input = concat!(
        ".. meta::\n",
        "   :description: fig. 1 is here. After.\n",
        "\n",
        "  :Author: Jane Doe. Also here.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(":Author: Jane Doe. Also here.")
        )),
        "less-indented :Author: after meta must stay whole-line Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Jane Doe")
        )),
        "less-indented :Author: body must not become leftover Prose, got {regions:?}"
    );
}

#[test]
fn author_after_meta_section_stays_structure() {
    let input = concat!(
        ".. meta::\n",
        "   :description: fig. 1 is here. After.\n",
        "\n",
        "Title\n",
        "=====\n",
        "\n",
        ":Author: Jane Doe wrote this. Next.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(":Author: Jane Doe wrote this. Next.")
        )),
        "flush :Author: after meta then section must stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn note_option_field_stays_structure() {
    let input = concat!(".. note::\n", "   :class: test\n", "\n", "   Body.\n",);
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":class: test"))),
        "note option field must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("   :class: test\n"),
        "note option field must stay one line, got:\n{out}"
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
