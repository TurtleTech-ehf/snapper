//! GitHub #324 / snapper-nd8i: Docutils `Body.patterns` bullet is
//! `[-+*\u2022\u2023\u2043]( +|$)`. Lone `-` / `*` at EOL and unicode
//! `•` / `‣` / `⁃` (with or without payload) are Structure; following
//! prose still splits. Lone `+` stays a grid-table leftover fragment.
//! Trailing-space ASCII bullets and enumerators stay.

use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Rst / GitHub #324).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "-\n",
        "After empty item. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "-\n",
        "After empty item.\n",
        "Next sentence.\n",
    )
}

fn wrapped(marker: &str) -> String {
    format!(
        "Intro sentence here. Another intro sentence.\n{marker}\nAfter empty item. Next sentence.\n"
    )
}

fn expected_wrapped(marker: &str) -> String {
    format!(
        "Intro sentence here.\nAnother intro sentence.\n{marker}\nAfter empty item.\nNext sentence.\n"
    )
}

fn expected_payload(marker: &str) -> String {
    let hang = " ".repeat(marker.chars().count());
    format!(
        "Intro sentence here.\nAnother intro sentence.\n{marker}After empty item.\n{hang}Next sentence.\n"
    )
}

#[test]
fn empty_dash_is_structure() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "-")),
        "lone - at EOL must be Structure, got {regions:?}"
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
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn empty_star_is_list_marker() {
    let input = wrapped("*");
    let regions = RstParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "*")),
        "lone * at EOL must be Structure, got {regions:?}"
    );
    let out = format_text(&input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("*"), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn unicode_bullets_empty_and_payload_are_list_markers() {
    for bullet in ["•", "‣", "⁃"] {
        let empty = wrapped(bullet);
        let regions = RstParser.parse(&empty);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == bullet)),
            "lone {bullet} at EOL must be Structure, got {regions:?}"
        );
        let out = format_text(&empty, &rst_cfg()).unwrap();
        assert_eq!(out, expected_wrapped(bullet), "empty {bullet} got:\n{out}");
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);

        let marked = format!("{bullet} ");
        let payload = format!(
            "Intro sentence here. Another intro sentence.\n{marked}After empty item. Next sentence.\n"
        );
        let regions = RstParser.parse(&payload);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == marked.as_str())),
            "{bullet} with payload must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("After empty item.") && p.contains("Next sentence.")
            )),
            "{bullet} payload must stay Prose, got {regions:?}"
        );
        let out = format_text(&payload, &rst_cfg()).unwrap();
        assert_eq!(
            out,
            expected_payload(&marked),
            "payload {bullet} got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
}

#[test]
fn lone_plus_stays_grid_table_leftover() {
    let input = wrapped("+");
    let regions = RstParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "+")),
        "lone + at EOL must stay leftover Structure, got {regions:?}"
    );
    let out = format_text(&input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("+"), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn trailing_space_ascii_bullets_and_enumerators_unchanged() {
    for input in [
        "- item\n",
        "* Hello world.\n",
        "+ Hello world.\n",
        "1. Hello world.\n",
        "a. Alpha item.\n",
        "(1) Paren arabic.\n",
    ] {
        let out = format_text(input, &rst_cfg()).unwrap();
        assert_eq!(out, input, "trailing-space marker must stay, got:\n{out}");
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
    let regions = RstParser.parse("- item\n");
    assert_eq!(regions[0], Region::Structure("- ".to_string()));
    assert_eq!(regions[1], Region::Prose("item".to_string()));
    let numbered = RstParser.parse("1. Hello world.\n");
    assert_eq!(numbered[0], Region::Structure("1. ".to_string()));
    assert_eq!(numbered[1], Region::Prose("Hello world.".to_string()));
    let star = RstParser.parse("* Hello world.\n");
    assert_eq!(star[0], Region::Structure("* ".to_string()));
    assert_eq!(star[1], Region::Prose("Hello world.".to_string()));
}

#[test]
fn option_lists_and_anonymous_targets_stay() {
    let option = "-a            Output all.\n";
    let out = format_text(option, &rst_cfg()).unwrap();
    assert_eq!(out, option, "option list must stay, got:\n{out}");
    let target = "__ https://www.python.org/some/very/long/path\n";
    let out = format_text(target, &rst_cfg()).unwrap();
    assert_eq!(out, target, "anonymous target must stay, got:\n{out}");
    let regions = RstParser.parse(target);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("__ https://www.python.org")
        )),
        "anonymous target must stay Structure, got {regions:?}"
    );
}
