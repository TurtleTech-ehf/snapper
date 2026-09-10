//! snapper-s5la / GitHub #180: Org footnote definitions are not prose.
//! `[fn:1]` is Structure; the body is hung Prose that still splits.

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

/// Ticket fixture: inline `[fn:1]` reference plus a definition.
fn ticket_fixture() -> &'static str {
    concat!(
        "See the claim.[fn:1]\n",
        "\n",
        "[fn:1] This is a long footnote sentence that must stay inside the definition. Second sentence.\n",
    )
}

#[test]
fn footnote_opener_is_structure_body_is_hung_prose() {
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "[fn:1] ")),
        "[fn:1] must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long footnote sentence that must stay inside the definition.")
                    && s.contains("Second sentence.")
        )),
        "footnote body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a long footnote sentence")
        )),
        "footnote body must not stay Structure, got {regions:?}"
    );
}

#[test]
fn footnote_fixture_body_splits_and_hangs() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "See the claim.[fn:1]\n",
            "\n",
            "[fn:1] This is a long footnote sentence that must stay inside the definition.\n",
            "       Second sentence.\n",
        ),
        "footnote opener stays; body hangs and splits, got:\n{out}"
    );
    assert!(
        !out.contains("\nSecond sentence."),
        "split body must not become a column-0 paragraph, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &org_cfg()).unwrap(),
        out,
        "hung footnote must be identity, got:\n{out}"
    );
}

#[test]
fn named_footnote_is_structure_plus_hung_prose() {
    let input = "[fn:note] Footnote text. Second sentence.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "[fn:note] ")),
        "[fn:note] must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Footnote text.") && s.contains("Second sentence.")
        )),
        "[fn:note] body must be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out, "[fn:note] Footnote text.\n          Second sentence.\n",
        "[fn:note] body must hang and split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
