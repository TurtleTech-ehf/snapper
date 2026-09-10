use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{format_text, FormatConfig};

/// GitHub #99 / snapper-8n65: Docutils isolate_simple_table does not stop
/// on interior blanks. Fails on origin/main (later rows become quote Prose;
/// closer joins as stray prose) then passes.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        "=====  =====\n",
        "Name   Value\n",
        "=====  =====\n",
        "A      first\n",
        "\n",
        "       more\n",
        "=====  =====\n",
    )
}

#[test]
fn interior_blank_table_rows_are_structure_not_prose() {
    let regions = RstParser.parse(ticket_fixture());
    for needle in ["Name   Value", "A      first", "more"] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "table line {needle:?} must be Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains(needle))),
            "table line {needle:?} must not be Prose, got {regions:?}"
        );
    }
    assert!(
        regions.iter().any(|r| matches!(r, Region::BlankLines(_))),
        "interior blank may stay BlankLines, got {regions:?}"
    );
}

#[test]
fn interior_blank_simple_table_is_identity_under_format() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "simple table with interior blank must stay unjoined, got:\n{out}"
    );
    assert!(
        !out.contains("more ====="),
        "closer must not join onto the continuation row, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung interior-blank simple table must be identity, got:\n{out}"
    );
}
