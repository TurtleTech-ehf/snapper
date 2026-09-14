//! org-element-clock-line-re leftover is CLOCK: plus a timestamp or
//! => duration. Bare CLOCK: prose still splits.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn bare_clock_prefix_is_paragraph() {
    let input = concat!(
        "CLOCK: hello. World after that.\n",
        "After the line. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("CLOCK: hello.") && p.contains("World after that.")
        )),
        "bare CLOCK: must stay Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("CLOCK: hello")
        )),
        "bare CLOCK: must not be a clock line, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("CLOCK: hello.\nWorld after that."),
        "bare CLOCK: prose must still split, got:\n{out}"
    );
    assert!(
        out.contains("After the line.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn valid_clock_timestamp_stays_structure() {
    let input = concat!(
        "CLOCK: [2026-01-01 Thu 10:00]\n",
        "After the line. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("CLOCK: [2026-01-01 Thu 10:00]")
        )),
        "valid CLOCK: must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("CLOCK: [2026-01-01 Thu 10:00]\n"),
        "valid CLOCK: must not split, got:\n{out}"
    );
    assert!(
        out.contains("After the line.\nNext."),
        "following prose must still split, got:\n{out}"
    );
}
