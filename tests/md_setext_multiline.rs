//! GitHub #208 / snapper-4wxk: CommonMark 0.31.2 §4.3 ex. 50–51.
//! pulldown `parse_setext_heading` promotes the whole open paragraph.
//! Pairing only the last title line with the underline left earlier
//! title lines as Prose, so they sentence-split.

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

/// Ticket fixture: two-line setext title plus two-sentence body.
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
fn multiline_setext_title_is_structure_body_still_splits() {
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
            .any(|r| matches!(r, Region::Structure(s) if s.starts_with('='))),
        "underline must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("second title")
        )),
        "title lines must not be Prose: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Body after setext.") && p.contains("Second body.")
        )),
        "body must stay Prose, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.starts_with(
            "Foo is the first title line. Still title.\nBar is the second title line.\n=======\n"
        ),
        "both title lines plus underline must stay Structure, got:\n{out}"
    );
    assert!(
        !out.contains("Still title.\nStill") && !out.contains("title.\nFoo"),
        "must not reflow the setext title, got:\n{out}"
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
fn multiline_setext_three_title_lines_are_structure() {
    let input = concat!(
        "Foo is the first title line. Still title.\n",
        "Bar is the second title line.\n",
        "Baz is the third title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    for line in [
        "Foo is the first title line. Still title.\n",
        "Bar is the second title line.\n",
        "Baz is the third title line.\n",
        "=======\n",
    ] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == line)),
            "expected Structure {line:?}, got: {regions:?}"
        );
    }
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("title line"))),
        "no title line may be Prose: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}
