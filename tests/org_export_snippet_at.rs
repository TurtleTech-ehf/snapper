//! org-element-export-snippet-parser searches to the next `@@`.
//! The value may contain a single `@` (`@@html:@import ...@@`).

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn wrap_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 16,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    "See @@html:@import url(a. b)@@ today. Next.\n"
}

#[test]
fn at_in_snippet_value_stays_one_wrap_token() {
    let input = ticket_fixture();
    let token = "@@html:@import url(a. b)@@";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(token) && s.contains("today.") && s.contains("Next.")
        )),
        "snippet with @ in the value stays inline Prose, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("today."))),
        "trailing prose must not freeze as Structure, got {regions:?}"
    );
    let out = format_text(input, &wrap_cfg()).unwrap();
    assert!(
        !out.lines().any(|l| l.trim_start().starts_with("@@")),
        "wrap must not park the snippet at BOL, got:\n{out}"
    );
    assert!(
        out.contains("@@html:@import") && out.contains("url(a. b)@@"),
        "snippet with @ in the value must stay in the formatted text, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("today. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg()).unwrap(), out);
}

#[test]
fn whole_line_snippet_with_at_is_structure() {
    let input = "Text before.\n@@html:@import url(a. b)@@\nText after.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("@@html:@import url(a. b)@@")
        )),
        "whole-line snippet with @ in the value is Structure, got {regions:?}"
    );
}
