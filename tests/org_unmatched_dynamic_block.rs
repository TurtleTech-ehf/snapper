//! Unmatched dynamic block (`#+BEGIN: NAME` without `#+END:`) is a
//! paragraph. org-element does not swallow following prose to EOF.

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
        "#+BEGIN: clocktable :scope file\n",
        "This is a long sentence. Second sentence.\n",
        "After the block. Next.\n",
    )
}

#[test]
fn unmatched_dynamic_block_is_paragraph() {
    let input = ticket_fixture();
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("#+BEGIN: clocktable")
                    && s.contains("After the block.")
                    && s.contains("Next.")
        )),
        "unmatched #+BEGIN: is a paragraph, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("After the block."))),
        "unmatched dynamic block must not swallow following prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("After the block.\nNext."),
        "After the block. / Next. must still split, got:\n{out}"
    );
    assert!(
        !out.contains("After the block. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn closed_dynamic_block_still_structure() {
    let input = concat!(
        "#+BEGIN: clocktable :scope file\n",
        "This is a long sentence. Second sentence.\n",
        "#+END:\n",
        "After the block. Next.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#+BEGIN: clocktable :scope file\n",
            "This is a long sentence. Second sentence.\n",
            "#+END:\n",
            "After the block.\n",
            "Next.\n",
        ),
        "closed #+BEGIN: stays Structure, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
