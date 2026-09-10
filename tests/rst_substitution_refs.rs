//! snapper-t0th: RST substitution references are not leftover table rows.
//! Docutils Inliner substitution_ref is inline; line_block is only `|`
//! plus space or EOL; grid_table_top is `+---+`. Leftover `|` stays Prose.

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
        "|version| is the current release. Second sentence.\n",
        "\n",
        "The current release is\n",
        "|version|. Next sentence.\n",
    )
}

#[test]
fn substitution_ref_lines_are_prose_not_structure() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("|version|")
        )),
        "|version| must not be leftover Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("|version| is the current release.")
                    && s.contains("Second sentence.")
        )),
        "lead |version| line must be Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("The current release is")
                    && s.contains("|version|.")
                    && s.contains("Next sentence.")
        )),
        "mid-paragraph |version| must stay Prose, got {regions:?}"
    );
}

#[test]
fn substitution_ref_fixture_splits_as_prose() {
    let out = format_text(ticket_fixture(), &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "|version| is the current release.\n",
            "Second sentence.\n",
            "\n",
            "The current release is |version|.\n",
            "Next sentence.\n",
        ),
        "both |version| sentences must split as Prose, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "split substitution-ref prose must be identity, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, ticket_fixture(), &out),
        "oracle mismatch\n in={:?}\n out={out:?}",
        ticket_fixture()
    );
}

#[test]
fn line_block_and_grid_table_still_hold() {
    let line_block = "| This is a line. Another sentence.\n";
    let out = format_text(line_block, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!("| This is a line.\n", "  Another sentence.\n"),
        "line-block hang must still split, got:\n{out}"
    );
    let grid = concat!("+---+---+\n", "| a | b |\n", "+---+---+\n");
    assert_eq!(
        format_text(grid, &rst_cfg()).unwrap(),
        grid,
        "grid table must stay identity"
    );
}
