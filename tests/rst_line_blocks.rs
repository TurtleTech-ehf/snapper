use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #174 / snapper-4a31: RST line blocks must not be stolen by
/// the grid-table arm. Docutils line_block (`| `) is before grid_table_top.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        "| This is a line. Another sentence.\n",
        "| Next line. More text.\n",
    )
}

#[test]
fn line_block_marker_is_structure_body_is_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "| ")),
        "| plus space must be Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("This is a line"))),
        "line-block body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("This is a line") || s.contains("Another sentence")
        )),
        "line-block sentences must not be full-line Structure, got {regions:?}"
    );
}

#[test]
fn line_block_prose_splits_and_hangs() {
    let out = format_text(ticket_fixture(), &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "| This is a line.\n",
            "  Another sentence.\n",
            "| Next line.\n",
            "  More text.\n",
        ),
        "line-block body must split and hang at | width, got:\n{out}"
    );
    let twice = format_text(&out, &rst_cfg()).unwrap();
    assert_eq!(
        out, twice,
        "hung line block must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, ticket_fixture(), &out),
        "oracle mismatch\n in={:?}\n out={out:?}",
        ticket_fixture()
    );
}

#[test]
fn grid_table_stays_full_line_structure() {
    let input = concat!("+---+---+\n", "| a | b |\n", "+---+---+\n",);
    let regions = RstParser.parse(input);
    for needle in ["+---+---+", "| a | b |"] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "grid line {needle:?} must be Structure, got {regions:?}"
        );
    }
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains('a') || s.contains('+'))),
        "grid table must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, input, "grid table must stay identity, got:\n{out}");
}
