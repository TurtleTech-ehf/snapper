//! GitHub #320 / snapper-fsdk: Emacs 30.2 `org-item-re` allows a
//! bullet then whitespace or EOL. A lone `-` / `+` / `1)` / `1.` /
//! indented `*` is Structure; following prose still splits. Column-0
//! `*` is a headline, not a list. Trailing-space markers stay.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Org / GitHub #320).
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

#[test]
fn empty_dash_is_structure() {
    let regions = OrgParser.parse(ticket_fixture());
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
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn empty_plus_is_list_marker() {
    let input = wrapped("+");
    let regions = OrgParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "+")),
        "lone + at EOL must be Structure, got {regions:?}"
    );
    let out = format_text(&input, &org_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("+"), "got:\n{out}");
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn empty_paren_number_is_list_marker() {
    let input = wrapped("1)");
    let regions = OrgParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "1)")),
        "lone 1) at EOL must be Structure, got {regions:?}"
    );
    let out = format_text(&input, &org_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("1)"), "got:\n{out}");
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn empty_dot_number_is_list_marker_not_uax_sentence() {
    let input = wrapped("1.");
    let regions = OrgParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "1.")),
        "1. at EOL must be a list marker, not a UAX sentence, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("1."))),
        "1. at EOL must not stay Prose, got {regions:?}"
    );
    let out = format_text(&input, &org_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("1."), "got:\n{out}");
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn empty_indented_star_is_list_marker() {
    let input = wrapped("  *");
    let regions = OrgParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "  *")),
        "indented * at EOL must be Structure, got {regions:?}"
    );
    let out = format_text(&input, &org_cfg()).unwrap();
    assert_eq!(out, expected_wrapped("  *"), "got:\n{out}");
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn column_0_star_is_not_a_list() {
    let input = wrapped("*");
    let regions = OrgParser.parse(&input);
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "*")),
        "column-0 * is a headline, not a list, got {regions:?}"
    );
}

#[test]
fn trailing_space_markers_unchanged() {
    for input in [
        "- item\n",
        "1. Hello world.\n",
        "1) Hello world.\n",
        "  * child\n",
    ] {
        let out = format_text(input, &org_cfg()).unwrap();
        assert_eq!(out, input, "trailing-space marker must stay, got:\n{out}");
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }
    let regions = OrgParser.parse("- item\n");
    assert_eq!(regions[0], Region::Structure("- ".to_string()));
    assert_eq!(regions[1], Region::Prose("item".to_string()));
    let numbered = OrgParser.parse("1. Hello world.\n");
    assert_eq!(numbered[0], Region::Structure("1. ".to_string()));
    assert_eq!(numbered[1], Region::Prose("Hello world.".to_string()));
    let paren = OrgParser.parse("1) Hello world.\n");
    assert_eq!(paren[0], Region::Structure("1) ".to_string()));
    assert_eq!(paren[1], Region::Prose("Hello world.".to_string()));
}
