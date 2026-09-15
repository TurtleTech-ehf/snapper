//! org-element-keyword-re is `#+KEY` optional `[dual]` then `:`.
//! `#+notakeyword` is leftover paragraph, not a keyword.

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
fn leftover_hash_plus_without_colon_is_paragraph() {
    let input = concat!("#+notakeyword Hello. World.\n", "After the line. Next.\n",);
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Hello.") && p.contains("World.")
        )),
        "#+notakeyword must stay Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("notakeyword")
        )),
        "#+notakeyword must not be a keyword, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("Hello.\nWorld."),
        "leftover #+ line must still split, got:\n{out}"
    );
    assert!(
        out.contains("After the line.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn hash_plus_space_title_is_not_a_keyword() {
    let input = "#+ TITLE: Hello. World.\nAfter the line. Next.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("TITLE:")
        )),
        "#+ TITLE: with a space after #+ must stay Prose, got {regions:?}"
    );
}

#[test]
fn real_title_keyword_stays_structure() {
    let input = "#+TITLE: Hello. World.\nAfter the line. Next.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("#+TITLE:")
        )),
        "#+TITLE: must stay a keyword, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("#+TITLE: Hello. World.\n"),
        "real keyword must not split, got:\n{out}"
    );
    assert!(
        out.contains("After the line.\nNext."),
        "following prose must still split, got:\n{out}"
    );
}
