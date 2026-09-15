//! Quoted GFM tables without a leading pipe stay Structure.
//! GFM 4.10 / pulldown ENABLE_TABLES after `>`.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "> Name | Note\n",
        "> --- | ---\n",
        "> Foo | Bar. Baz\n",
        "\n",
        "After the table. Next.\n",
    )
}

#[test]
fn quoted_pipe_optional_table_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Name | Note")
        )),
        "quoted header must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Foo | Bar. Baz")
        )),
        "quoted body row must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Name | Note") || p.contains("Bar. Baz")
        )),
        "quoted pipe-optional table must not be Prose, got {regions:?}"
    );
}

#[test]
fn quoted_pipe_optional_table_does_not_reflow() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("> Name | Note\n> --- | ---\n> Foo | Bar. Baz\n"),
        "quoted table must stay identity, got:\n{out}"
    );
    assert!(
        !out.contains("Bar.\n"),
        "must not sentence-split a quoted table cell, got:\n{out}"
    );
    assert!(
        out.contains("After the table.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
