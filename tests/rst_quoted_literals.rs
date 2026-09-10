use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #92 / snapper-onis: quoted literal blocks after `::` must not reflow.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        "Take it literally::\n",
        "\n",
        "> if literal_block:\n",
        ">     text = 'is left as-is'\n",
        ">     markup_processing = None\n",
    )
}

#[test]
fn quoted_literal_lines_are_structure_not_prose() {
    let regions = RstParser.parse(ticket_fixture());
    for needle in [
        "> if literal_block:",
        ">     text = 'is left as-is'",
        ">     markup_processing = None",
    ] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "quoted literal line {needle:?} must be Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains(needle))),
            "quoted literal line {needle:?} must not be Prose, got {regions:?}"
        );
    }
}

#[test]
fn quoted_literal_block_is_identity_under_format() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "quoted literal lines after :: must stay unjoined, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung quoted literal must be identity, got:\n{out}"
    );
}

/// Flush `>` without a preceding `::` is ordinary prose, not a literal.
#[test]
fn flush_gt_without_double_colon_still_reflows() {
    let input = "> if this were email it is still prose. Second sentence.\n";
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, "> if this were email it is still prose.\nSecond sentence.\n",
        "flush > without :: must still reflow as prose, got:\n{out}"
    );
}

/// A blank after the quoted body ends it; following prose still reflows.
#[test]
fn prose_after_quoted_literal_still_reflows() {
    let input = concat!(
        "Take it literally::\n",
        "\n",
        "> if literal_block:\n",
        ">     text = 'is left as-is'\n",
        "\n",
        "After. More after.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("> if literal_block:\n>     text = 'is left as-is'\n"),
        "quoted body must stay unjoined, got:\n{out}"
    );
    assert!(
        out.contains("After.\nMore after."),
        "prose after quoted literal must still reflow, got:\n{out}"
    );
}
