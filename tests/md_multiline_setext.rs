//! GitHub #208 / snapper-4wxk: CommonMark 4.3 ex. 50–51. pulldown
//! `parse_setext_heading` promotes the whole open paragraph. Pairing only
//! the last title line with the underline left earlier lines as Prose.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Markdown).
fn ticket_fixture() -> &'static str {
    concat!(
        "Foo is the first title line. Still title.\n",
        "Bar is the second title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    )
}

#[test]
fn multiline_setext_title_lines_and_underline_are_structure() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "Foo is the first title line. Still title.\n"
        )),
        "first title line must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "Bar is the second title line.\n"
        )),
        "second title line must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "=======")),
        "underline must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Still title")
                    || p.contains("Foo is the first")
                    || p.contains("Bar is the second")
        )),
        "title lines must not be Prose: {regions:?}"
    );
}

#[test]
fn multiline_setext_body_still_splits() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Body after setext") && p.contains("Second body")
        )),
        "body must stay Prose, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.starts_with(
            "Foo is the first title line. Still title.\nBar is the second title line.\n=======\n"
        ),
        "both title lines plus underline must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split the setext title, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
    assert!(
        !out.contains("Body after setext. Second body."),
        "fused body must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn multiline_setext_survives_safety_backstops() {
    let input = ticket_fixture();
    let cfg = FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    };
    let out = format_text(input, &cfg).unwrap();
    assert!(
        out.contains(
            "Foo is the first title line. Still title.\nBar is the second title line.\n======="
        ),
        "CLI backstops must not reflow the title, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "CLI backstops must not revert the body split, got:\n{out}"
    );
}
