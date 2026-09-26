//! Org list item tag (`tag :: description`) stays on the item line.
//! The description after ` :: ` reflows. A sentence in the tag does not
//! split away from the bullet.

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

#[test]
fn item_tag_stays_on_the_item_line_description_reflows() {
    let input = "- Alpha. Beta :: One. Two.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "- Alpha. Beta :: "
        )),
        "tag and separator stay Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p == "One. Two.")),
        "description after :: is Prose, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("Alpha."))),
        "a sentence in the tag must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    let hang = " ".repeat("- Alpha. Beta :: ".chars().count());
    assert_eq!(
        out,
        format!("- Alpha. Beta :: One.\n{hang}Two.\n"),
        "tag stays; description splits under the separator, got:\n{out}"
    );
    assert!(
        !out.contains("- Alpha.\n"),
        "tag sentence must not leave the item marker, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn tagged_item_description_on_the_next_line_is_kept() {
    let input = "- Alpha. Beta ::\n  One. Two.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("- Alpha. Beta ::")
        )),
        "tag stays Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("One. Two."))),
        "next-line description must be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("- Alpha. Beta ::"),
        "tag line stays, got:\n{out}"
    );
    assert!(
        out.contains("One."),
        "first description sentence stays, got:\n{out}"
    );
    assert!(
        out.contains("Two."),
        "second description sentence stays, got:\n{out}"
    );
    assert!(
        !out.contains("One. Two."),
        "description must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
