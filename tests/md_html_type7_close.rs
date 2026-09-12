//! GitHub #356 / snapper-615s: CommonMark 0.31.2 sec 4.6 HTML type 7.
//! A closed `<span>…</span>` must not include the next paragraph.
//! `After html.` / `Next.` stay Prose and still split. Type-6 close
//! (`<div>…</div>`) and quoted HTML stay intact.

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

/// Ticket fixture (Format::Markdown / GitHub #356).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "<span class=\"note\">\n",
        "First. Second.\n",
        "</span>\n",
        "After html. Next.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "<span class=\"note\">\n",
        "First. Second.\n",
        "</span>\n",
        "After html.\n",
        "Next.\n",
    )
}

#[test]
fn closed_span_is_structure_through_close() {
    let regions = MarkdownParser.parse(ticket_fixture());
    let span = regions.iter().find_map(|r| match r {
        Region::Structure(s) if s.contains("<span") => Some(s.as_str()),
        _ => None,
    });
    let span = span.expect(&format!("span block must be Structure, got {regions:?}"));
    assert!(span.contains("<span class=\"note\">"), "{span}");
    assert!(span.contains("First. Second."), "{span}");
    assert!(span.contains("</span>"), "{span}");
    assert!(
        !span.contains("After html"),
        "closed type-7 must end at </span>, got {span}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("First.") || p.contains("<span")
        )),
        "span body must not be Prose: {regions:?}"
    );
}

#[test]
fn after_html_stays_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After html.") && p.contains("Next.")
        )),
        "After html. / Next. must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_span_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn type6_closed_div_still_ends_at_close() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        "<div>\n",
        "First. Second.\n",
        "</div>\n",
        "After html. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    let div = regions.iter().find_map(|r| match r {
        Region::Structure(s) if s.contains("<div>") => Some(s.as_str()),
        _ => None,
    });
    let div = div.expect(&format!("div block must be Structure, got {regions:?}"));
    assert!(div.contains("</div>"), "{div}");
    assert!(
        !div.contains("After html"),
        "type-6 close intact, got {div}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            "<div>\n",
            "First. Second.\n",
            "</div>\n",
            "After html.\n",
            "Next.\n",
        ),
        "type-6 close must stay intact, got:\n{out}"
    );
}

#[test]
fn quoted_html_unquoted_next_stays_intact() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "> <div>\n",
        "After tag. Next sentence.\n",
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Intro sentence here.\n",
            "Another intro sentence.\n",
            "\n",
            "> <div>\n",
            "After tag.\n",
            "Next sentence.\n",
        ),
        "quoted HTML must stay intact, got:\n{out}"
    );
}

#[test]
fn unclosed_type7_still_does_not_interrupt() {
    let input = "Intro. More.\n<span class=\"x\">\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Intro") && p.contains("<span")
        )),
        "unclosed type-7 must stay in the paragraph, got: {regions:?}"
    );
}
