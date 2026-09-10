//! snapper-t0th / snapper-15i9: RST substitution references.
//! Docutils Inliner substitution_ref is inline; line_block is only `|`
//! plus space or EOL; grid_table_top is `+---+`. Leftover `|` stays Prose.
//! Interior punctuation inside `|fig. 1|` is not a sentence boundary.

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

/// snapper-15i9 / GitHub #233 ticket fixture (Format::Rst).
fn interior_punct_fixture() -> &'static str {
    "See |fig. 1| in the caption. Next sentence.\n"
}

#[test]
fn substitution_ref_interior_punct_stays_one_token() {
    let regions = RstParser.parse(interior_punct_fixture());
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("|fig. 1|") || s.contains("|version|")
        )),
        "|fig. 1| / |version| must stay Prose not Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("|fig. 1|")
                    && s.contains("in the caption.")
                    && s.contains("Next sentence.")
        )),
        "ticket fixture must stay one Prose region, got {regions:?}"
    );
}

#[test]
fn substitution_ref_interior_punct_fixture_splits_after_caption() {
    let input = interior_punct_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("|fig. 1|"),
        "|fig. 1| must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("|fig.\n") && !out.contains("|fig.\n1|"),
        "must not split inside the substitution ref, got:\n{out}"
    );
    assert_eq!(
        out,
        concat!("See |fig. 1| in the caption.\n", "Next sentence.\n"),
        "Next sentence. must still split; |fig. 1| stays one token, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "split substitution-ref prose must be identity, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );

    let guarded = FormatConfig {
        format: Format::Rst,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
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
