use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #99 / snapper-8n65: RST simple tables must not end at an interior blank.
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
fn interior_blank_rows_are_structure_not_prose() {
    let regions = RstParser.parse(ticket_fixture());
    for needle in ["Name   Value", "A      first", "       more"] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "table line {needle:?} must be Structure, got {regions:?}"
        );
    }
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("Name")
                    || s.contains("first")
                    || s.contains("more")
                    || s.contains("=====")
        )),
        "table lines must not be Prose, got {regions:?}"
    );
}

#[test]
fn interior_blank_fixture_is_identity_under_format() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "interior-blank simple table must stay identity, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "interior-blank simple table must be identity, got:\n{out}"
    );
}
