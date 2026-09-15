//! Quoted indented code: a `>` line then 4-space inner stays Code.
//! pulldown / CommonMark 4.4 + 5.1. Following quote prose still splits.

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
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "> Quote opener. Still quote.\n",
        ">\n",
        ">     indented. not split\n",
        ">\n",
        "> After code. Still quoted.\n",
        "\n",
        "After the quote. Next.\n",
    )
}

#[test]
fn quoted_indented_code_is_code_not_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("indented. not split")
        )),
        "quoted 4-space inner must be Code, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("indented. not split")
        )),
        "quoted indented code must not be Prose, got {regions:?}"
    );
}

#[test]
fn quoted_indented_code_does_not_reflow_and_following_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains(">     indented. not split\n"),
        "quoted indented code must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains(">     indented.\n"),
        "must not sentence-split quoted indented code, got:\n{out}"
    );
    assert!(
        out.contains("> After code.\n> Still quoted."),
        "quote prose after code must still split, got:\n{out}"
    );
    assert!(
        out.contains("After the quote.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
