//! CommonMark 0.31.2 sec 4.6 HTML type 6 ends at a blank line.
//! Text after `</div>` and before a blank stays in the block.
//! A blank line lets the next paragraph split. Type-1 `<pre>`
//! still ends at `</pre>`.

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

/// Ticket fixture (Format::Markdown / GitHub #332).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "<div>\n",
        "First. Second.\n",
        "</div>\n",
        "After html. Next.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "<div>\n",
        "First. Second.\n",
        "</div>\n",
        "After html. Next.\n",
    )
}

#[test]
fn closed_div_is_structure_through_close() {
    let regions = MarkdownParser.parse(ticket_fixture());
    let div = regions.iter().find_map(|r| match r {
        Region::Structure(s) if s.contains("<div>") => Some(s.as_str()),
        _ => None,
    });
    let div = div.expect(&format!("div block must be Structure, got {regions:?}"));
    assert!(div.contains("<div>"), "{div}");
    assert!(div.contains("First. Second."), "{div}");
    assert!(div.contains("</div>"), "{div}");
    assert!(
        div.contains("After html. Next."),
        "text before a blank stays in the type-6 block, got {div}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("First.") || p.contains("<div")
        )),
        "div body must not be Prose: {regions:?}"
    );
}

#[test]
fn blank_line_ends_type6_and_the_next_paragraph_splits() {
    let input = concat!(
        "<div>\n",
        "First. Second.\n",
        "</div>\n",
        "\n",
        "After html. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After html.") && p.contains("Next.")
        )),
        "a blank line ends the block, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("After html.\nNext.\n"),
        "paragraph after the blank still splits, got:\n{out}"
    );
}

#[test]
fn ticket_fixture_keeps_div_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn pre_type1_still_ends_at_close() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        "<pre>\n",
        "foo. bar\n",
        "</pre>\n",
        "After pre. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    match regions.iter().find(|r| matches!(r, Region::Code { .. })) {
        Some(Region::Code {
            header,
            body,
            footer,
            ..
        }) => {
            assert!(header.contains("<pre>"), "{header:?}");
            assert!(body.contains("foo. bar"), "{body:?}");
            assert!(footer.contains("</pre>"), "{footer:?}");
        }
        other => panic!("pre block must stay Code, got {other:?} / {regions:?}"),
    }
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After pre.") && p.contains("Next.")
        )),
        "type-1 close must not swallow following prose: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    let expected = concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "<pre>\n",
        "foo. bar\n",
        "</pre>\n",
        "After pre.\n",
        "Next.\n",
    );
    assert_eq!(out, expected, "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
