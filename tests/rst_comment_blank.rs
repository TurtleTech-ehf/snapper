//! snapper-k7bb / GitHub #176: RST comments close on a blank line.
//! A later indent is a hung quote that still splits, not comment Structure.

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
        ".. Comment sentence. Still comment.\n",
        "\n",
        "    This is a block quote. Another sentence.\n",
    )
}

#[test]
fn comment_opener_is_structure_blank_closes() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(".. Comment sentence. Still comment.")
        )),
        "comment opener must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(r, Region::BlankLines(_))),
        "blank must close the comment, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a block quote")
        )),
        "post-blank indent must not stay comment Structure, got {regions:?}"
    );
}

#[test]
fn later_indent_is_hung_quote_and_splits() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a block quote.") && s.contains("Another sentence.")
        )),
        "post-blank indent must be hung Prose, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "    ")),
        "quote hang spaces must be Structure, got {regions:?}"
    );
    let out = format_text(ticket_fixture(), &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. Comment sentence. Still comment.\n",
            "\n",
            "    This is a block quote.\n",
            "    Another sentence.\n",
        ),
        "blank must close the comment; quote must hang and split, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung quote must be identity, got:\n{out}"
    );
}
