//! snapper-k7bb / GitHub #176: RST comments close on a blank.
//! A later indented quote is hung Prose that still splits.

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

/// Ticket fixture: comment opener, blank, indented quote.
fn ticket_fixture() -> &'static str {
    concat!(
        ".. Comment sentence. Still comment.\n",
        "\n",
        "    This is a block quote. Another sentence.\n",
    )
}

#[test]
fn comment_opener_is_structure_blank_closes_quote_is_hung_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Comment sentence.")
        )),
        "comment opener must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(r, Region::BlankLines(_))),
        "blank after comment must close as BlankLines, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a block quote.") && s.contains("Another sentence.")
        )),
        "later indent must be hung quote Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a block quote.")
        )),
        "later indent must not stay comment Structure, got {regions:?}"
    );
}

#[test]
fn comment_blank_quote_fixture_splits_and_hangs() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. Comment sentence. Still comment.\n",
            "\n",
            "    This is a block quote.\n",
            "    Another sentence.\n",
        ),
        "comment stays; blank closes; quote hangs and splits, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung quote after comment must be identity, got:\n{out}"
    );
}
