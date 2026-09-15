//! `starts_html_comment` must not treat 4-space `<!--` as type 2.
//! CommonMark 4.6: type 2 is at most three spaces. The 4-space comment
//! stays setext title text.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    concat!(
        "Foo is the first title line. Still title.\n",
        "    <!-- toc -->\n",
        "=======\n",
        "\n",
        "After the heading. Next.\n",
    )
}

#[test]
fn four_space_comment_stays_setext_title() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Foo is the first title line.")
        )),
        "setext title must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("<!-- toc -->")
        )),
        "4-space comment must stay in the heading, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("<!-- toc")
        )),
        "4-space <!-- must not interrupt the setext title, got {regions:?}"
    );
}

#[test]
fn four_space_comment_setext_does_not_split() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Foo is the first title line. Still title.\n    <!-- toc -->\n======="),
        "whole setext heading must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains("first title line.\n"),
        "must not sentence-split the setext title, got:\n{out}"
    );
    assert!(
        out.contains("After the heading.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
