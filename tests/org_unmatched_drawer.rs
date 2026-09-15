//! Unmatched drawer (`:LOGBOOK:` without `:END:`) is a paragraph.
//! org-element does not swallow following prose to EOF.

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

fn ticket_fixture() -> &'static str {
    concat!(
        ":LOGBOOK:\n",
        "CLOCK: [2026-01-01] First. Second.\n",
        "After the drawer. Next.\n",
    )
}

#[test]
fn unmatched_logbook_is_paragraph() {
    let input = ticket_fixture();
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains(":LOGBOOK:"))),
        "unmatched :LOGBOOK: is a paragraph, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":LOGBOOK:"))),
        "unmatched :LOGBOOK: must not be a drawer, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("After the drawer.") && s.contains("Next.")
        )),
        "After the drawer. / Next. must stay Prose, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("After the drawer."))),
        "unmatched drawer must not swallow following prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("After the drawer.\nNext."),
        "After the drawer. / Next. must still split, got:\n{out}"
    );
    assert!(
        !out.contains("After the drawer. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn closed_logbook_still_structure() {
    let input = ":LOGBOOK:\nCLOCK: [2026-01-01] First. Second.\n:END:\nAfter the drawer. Next.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out, ":LOGBOOK:\nCLOCK: [2026-01-01] First. Second.\n:END:\nAfter the drawer.\nNext.\n",
        "closed :LOGBOOK: stays a drawer, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
