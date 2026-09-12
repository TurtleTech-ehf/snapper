//! GitHub #326 / #329 / snapper-8k9o: CommonMark 0.31.2 sec 5.2 list
//! marker then 1–4 spaces or the empty rest of the line. Lone `-` / `*`
//! / `+` after a blank or at SOL is Structure; following prose still
//! splits. After a paragraph, lone `-` stays a setext underline.
//! Trailing-space markers stay.
//!
//! GitHub #337 / snapper-dfu5: tab-padded (`-\t`) and two-space-padded
//! (`-  `) empty markers are also empty items. One-space and three-or-
//! more spaces keep the #329 behavior. Setext `-` after a paragraph
//! stays a heading.

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

/// Ticket fixture (Format::Markdown / GitHub #326). Blank before the
/// marker so `-` is an empty item, not a setext underline.
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "-\n",
        "After empty item. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "\n",
        "-\n",
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
fn empty_dash_after_blank_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "-")),
        "lone - after a blank must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains('-') && p.contains("After empty item.")
        )),
        "empty dash must not join the following prose, got {regions:?}"
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
fn ticket_fixture_keeps_empty_dash_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn empty_star_after_blank_is_list_marker() {
    let input = wrapped("*");
    let regions = MarkdownParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "*")),
        "lone * after a blank must be Structure, got {regions:?}"
    );
    let out = format_text(&input, &md_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("*"), "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn empty_plus_after_blank_is_list_marker() {
    let input = wrapped("+");
    let regions = MarkdownParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "+")),
        "lone + after a blank must be Structure, got {regions:?}"
    );
    let out = format_text(&input, &md_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("+"), "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn empty_marker_at_sol_is_list_marker() {
    for marker in ["-", "*", "+"] {
        let input = format!("{marker}\nAfter empty item. Next sentence.\n");
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == marker)),
            "lone {marker} at SOL must be Structure, got {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        let expected = format!("{marker}\nAfter empty item.\nNext sentence.\n");
        assert_eq!(out, expected, "SOL {marker} got:\n{out}");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}

#[test]
fn lone_dash_after_paragraph_stays_setext() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        "-\n",
        "After empty item. Next sentence.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Intro sentence here.")
        )),
        "setext title must stay Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "-")),
        "setext underline must stay Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "-")),
        "setext - must not be an empty list marker, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Intro sentence here. Another intro sentence.\n-"),
        "setext title+underline must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains("Intro sentence here.\nAnother intro sentence.\n-"),
        "must not split the setext title, got:\n{out}"
    );
    assert!(
        out.contains("After empty item.\nNext sentence."),
        "body after setext must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn trailing_space_markers_unchanged() {
    for input in [
        "- item\n",
        "* Hello world.\n",
        "+ Hello world.\n",
        "1. Hello world.\n",
    ] {
        let out = format_text(input, &md_cfg()).unwrap();
        assert_eq!(out, input, "trailing-space marker must stay, got:\n{out}");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
    let regions = MarkdownParser.parse("- item\n");
    assert_eq!(regions[0], Region::Structure("- ".to_string()));
    assert_eq!(regions[1], Region::Prose("item".to_string()));
    let numbered = MarkdownParser.parse("1. Hello world.\n");
    assert_eq!(numbered[0], Region::Structure("1. ".to_string()));
    assert_eq!(numbered[1], Region::Prose("Hello world.".to_string()));
    let star = MarkdownParser.parse("* Hello world.\n");
    assert_eq!(star[0], Region::Structure("* ".to_string()));
    assert_eq!(star[1], Region::Prose("Hello world.".to_string()));
}

#[test]
fn ordinary_list_item_still_hangs() {
    let input = "* One. Two.";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, "* One.\n  Two.",
        "ordinary sembr hang must stay, got:\n{out}"
    );
}

/// Ticket fixture (Format::Markdown / GitHub #337). Two spaces after `-`.
fn two_space_ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "-  \n",
        "After empty item. Next sentence.\n",
    )
}

fn expected_two_space_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "\n",
        "-  \n",
        "After empty item.\n",
        "Next sentence.\n",
    )
}

#[test]
fn two_space_padded_empty_dash_is_structure() {
    let regions = MarkdownParser.parse(two_space_ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "-  ")),
        "two-space -  after a blank must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains('-') && p.contains("After empty item.")
        )),
        "two-space empty dash must not join the following prose, got {regions:?}"
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
fn two_space_ticket_fixture_keeps_empty_dash_and_splits_next() {
    let input = two_space_ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, expected_two_space_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn two_space_padded_star_and_plus_are_list_markers() {
    for marker in ["*  ", "+  "] {
        let input = wrapped(marker);
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == marker)),
            "two-space {marker:?} after a blank must be Structure, got {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert_eq!(out, expected_wrapped(marker), "got:\n{out}");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}

#[test]
fn tab_padded_empty_markers_are_list_items() {
    for marker in ["-\t", "*\t", "+\t"] {
        let input = wrapped(marker);
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == marker)),
            "tab-padded {marker:?} after a blank must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains(marker.chars().next().unwrap())
                    && p.contains("After empty item.")
            )),
            "tab-padded empty marker must not join following prose, got {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert_eq!(out, expected_wrapped(marker), "got:\n{out}");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}

#[test]
fn one_space_empty_marker_still_splits() {
    for marker in ["- ", "* ", "+ "] {
        let input = wrapped(marker);
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == marker)),
            "one-space {marker:?} after a blank must stay Structure, got {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert_eq!(out, expected_wrapped(marker), "got:\n{out}");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}

#[test]
fn three_or_more_spaces_keep_leftover_as_content() {
    for pad in ["   ", "    "] {
        let marker = format!("-{pad}");
        let input = wrapped(&marker);
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == "- ")),
            "3+ space pad must keep #329 one-space marker, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == marker.as_str())),
            "3+ space pad must not fold into an empty marker, got {regions:?}"
        );
    }
}

#[test]
fn two_space_dash_after_paragraph_stays_setext() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        "-  \n",
        "After empty item. Next sentence.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Intro sentence here.")
        )),
        "setext title must stay Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "-  ")),
        "setext -  must not be an empty list marker, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Intro sentence here. Another intro sentence.\n-  "),
        "setext title+underline must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains("Intro sentence here.\nAnother intro sentence.\n-  "),
        "must not split the setext title, got:\n{out}"
    );
    assert!(
        out.contains("After empty item.\nNext sentence."),
        "body after setext must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
