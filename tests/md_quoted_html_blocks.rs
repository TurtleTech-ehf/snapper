//! GitHub #340 / snapper-eyx7: CommonMark 0.31.2 sec 4.6 + 5.1.
//! Types 1–7 may start after `>`. Lazy continuation does not apply to
//! an HTML block, so an unquoted next line closes the quote.
//! `> <div>` is Structure; After tag. / Next sentence. stay unquoted
//! Prose and still split.

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

/// Ticket fixture (Format::Markdown / GitHub #340).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "> <div>\n",
        "After tag. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "\n",
        "> <div>\n",
        "After tag.\n",
        "Next sentence.\n",
    )
}

#[test]
fn quoted_div_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains('>') && s.contains("<div>")
        )),
        "> <div> must be Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("<div"))),
        "quoted <div> must not be Prose: {regions:?}"
    );
}

#[test]
fn after_tag_stays_unquoted_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After tag.") && p.contains("Next sentence.")
        )),
        "After tag. / Next sentence. must stay Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After tag.") && p.contains('>')
        )),
        "following prose must stay unquoted, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_div_and_splits_unquoted_next() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn quoted_pre_comment_span_following_prose_unquoted() {
    for opener in ["<pre>", "<!--", "<span>"] {
        let input = format!(
            "Intro sentence here. Another intro sentence.\n\n> {opener}\nAfter tag. Next sentence.\n"
        );
        let regions = MarkdownParser.parse(&input);
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains(opener))),
            "> {opener} must not be Prose, got {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert!(
            out.contains(&format!("> {opener}\nAfter tag.\nNext sentence.\n")),
            "> {opener} stays; After tag. / Next sentence. stay unquoted and split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}

#[test]
fn both_quoted_div_does_not_glue() {
    let input = "> <div>\n> After tag. Next sentence.\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, input,
        "both-quoted unclosed type-6 must not glue or split, got:\n{out}"
    );
}

#[test]
fn top_level_type6_interrupt_unchanged() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        "<div>\n",
        "After tag. Next sentence.\n",
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            "<div>\n",
            "After tag. Next sentence.\n",
        ),
        "top-level type 1-7 interrupt unchanged, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
