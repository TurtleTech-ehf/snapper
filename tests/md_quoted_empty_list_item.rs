//! GitHub #339 / snapper-u24m: CommonMark 0.31.2 sec 5.1 + 5.2.
//! `> -` is a blockquote whose only block is an empty list item. An
//! unquoted next line cannot lazily continue an empty item.

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

/// Ticket fixture (Format::Markdown / GitHub #339).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "> -\n",
        "After empty item. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "\n",
        "> -\n",
        "After empty item.\n",
        "Next sentence.\n",
    )
}

fn wrapped(marker: &str) -> String {
    format!(
        "Intro sentence here. Another intro sentence.\n\n{marker}\nAfter empty item. Next sentence.\n"
    )
}

fn expected_wrapped(marker: &str) -> String {
    format!(
        "Intro sentence here.\nAnother intro sentence.\n\n{marker}\nAfter empty item.\nNext sentence.\n"
    )
}

#[test]
fn quoted_empty_dash_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "> -")),
        "quoted empty - must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After empty item.") && p.contains('>')
        )),
        "unquoted After must not join the quote, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After empty item.") && p.contains("Next sentence.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_quoted_empty_dash_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert!(
        !out.contains("> - After") && !out.contains("> After empty item."),
        "must not swallow After into the quote, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn quoted_empty_star_and_plus_are_list_markers() {
    for marker in ["> *", "> +"] {
        let input = wrapped(marker);
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.trim() == marker)),
            "{marker} must be Structure, got {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert_eq!(out, expected_wrapped(marker), "{marker} got:\n{out}");
        assert!(
            !out.contains(&format!("{marker} After")) && !out.contains("> After empty item."),
            "{marker} must not swallow After into the quote, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}

#[test]
fn quoted_empty_ordered_does_not_swallow_after() {
    let input = wrapped("> 1.");
    let regions = MarkdownParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "> 1.")),
        "> 1. must be Structure, got {regions:?}"
    );
    let out = format_text(&input, &md_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("> 1."), "> 1. got:\n{out}");
    assert!(
        !out.contains("> 1. After") && !out.contains("> After empty item."),
        "> 1. must not swallow After into the quote, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn quoted_ordered_with_payload_still_period_splits() {
    let input = "> 1. After empty item. Next sentence.\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, "> 1. After empty item.\n> Next sentence.\n",
        "quoted 1. with payload must period-split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn quoted_nonempty_list_still_hangs() {
    let input = "> - Hello world. Next sentence.\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, "> - Hello world.\n> Next sentence.\n",
        "quoted nonempty list must keep quote hang, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn quoted_setext_dash_after_quote_title_unchanged() {
    let input =
        "> Intro sentence here. Another intro sentence.\n> -\nAfter empty item. Next sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Intro sentence here.")
        )),
        "quoted setext title must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("> Intro sentence here. Another intro sentence.\n> -"),
        "quoted setext title+underline must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains("> Intro sentence here.\n> Another intro sentence.\n> -"),
        "must not split the quoted setext title, got:\n{out}"
    );
    assert!(
        out.contains("After empty item.\nNext sentence."),
        "body after quoted setext must still split, got:\n{out}"
    );
    assert!(
        !out.contains("> After empty item."),
        "After after quoted setext must stay unquoted, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
