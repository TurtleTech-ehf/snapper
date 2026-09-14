//! List item then hang+4 indented code with no blank line stays Code.
//! Following item prose still splits. pulldown / CommonMark 5.2.

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
        "- Item one is a sentence. Second sentence.\n",
        "      indented. not split\n",
        "- Item two is a sentence. Second sentence.\n",
    )
}

#[test]
fn hang_plus_four_without_blank_is_code() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("indented. not split")
        )),
        "hang+4 without a blank must be Code, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("indented. not split")
        )),
        "hang+4 line must not be Prose, got {regions:?}"
    );
}

#[test]
fn hang_plus_four_stays_and_following_item_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("      indented. not split\n"),
        "hang+4 code must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains("      indented.\n"),
        "must not sentence-split hang+4 code, got:\n{out}"
    );
    assert!(
        out.contains("- Item one is a sentence.\n  Second sentence."),
        "first item must still split, got:\n{out}"
    );
    assert!(
        out.contains("- Item two is a sentence.\n  Second sentence."),
        "following item must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
