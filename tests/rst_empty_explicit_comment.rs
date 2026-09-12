//! GitHub #343 / snapper-wr17: Docutils `Body.explicit` / comment is a
//! lone `..` at EOL. That is Structure (empty comment), not a `.`
//! section underline, so the previous paragraph stays Prose and still
//! splits. `.. a comment.` already works. `...` stays an adornment.

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

/// Ticket fixture (Format::Rst / GitHub #343).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "..\n",
        "After markup. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "..\n",
        "After markup.\n",
        "Next sentence.\n",
    )
}

#[test]
fn empty_explicit_markup_is_structure() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "..")),
        "lone .. at EOL must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Intro sentence here.")
        )),
        "previous paragraph must not become a section title, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Intro sentence here.") && p.contains("Another intro sentence.")
        )),
        "previous paragraph must stay Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After markup.") && p.contains("Next sentence.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_empty_comment_and_splits_neighbors() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn text_comment_already_works() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        ".. a comment.\n",
        "After markup. Next sentence.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(".. a comment."))),
        ".. a comment. must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Intro sentence here.") && p.contains("Another intro sentence.")
        )),
        "previous paragraph must stay Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After markup.") && p.contains("Next sentence.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            ".. a comment.\n",
            "After markup.\n",
            "Next sentence.\n",
        ),
        "got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn three_dots_stay_a_section_underline() {
    let input = "Intro sentence here. Another intro sentence.\n...\n";
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Intro sentence here.")
        )),
        "title above ... must stay Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "...")),
        "... must stay a . adornment, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, input, "... must stay a section underline, got:\n{out}");
    assert!(
        !out.contains("Intro sentence here.\nAnother intro sentence.\n..."),
        "must not split a title above ..., got:\n{out}"
    );
}
