//! Unmatched VERSE / EXAMPLE / COMMENT (`#+BEGIN_NAME` without
//! `#+END_NAME`) is a paragraph. GitHub #355 only special-cased
//! unmatched EXPORT / SRC.

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

fn verse_fixture() -> &'static str {
    concat!(
        "#+BEGIN_VERSE\n",
        "Great clouds overhead. Tiny black birds.\n",
        "After the block. Next.\n",
    )
}

fn assert_unmatched_opaque_is_paragraph(input: &str, opener: &str) {
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains(opener)
                    && s.contains("After the block.")
                    && s.contains("Next.")
        )),
        "unmatched {opener} is a paragraph, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("After the block."))),
        "unmatched {opener} must not swallow following prose, got {regions:?}"
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
fn unmatched_verse_is_paragraph() {
    assert_unmatched_opaque_is_paragraph(verse_fixture(), "#+BEGIN_VERSE");
}

#[test]
fn unmatched_example_is_paragraph() {
    let input = concat!(
        "#+BEGIN_EXAMPLE\n",
        "Great clouds overhead. Tiny black birds.\n",
        "After the block. Next.\n",
    );
    assert_unmatched_opaque_is_paragraph(input, "#+BEGIN_EXAMPLE");
}

#[test]
fn unmatched_comment_is_paragraph() {
    let input = concat!(
        "#+BEGIN_COMMENT\n",
        "Great clouds overhead. Tiny black birds.\n",
        "After the block. Next.\n",
    );
    assert_unmatched_opaque_is_paragraph(input, "#+BEGIN_COMMENT");
}

#[test]
fn closed_verse_still_structure() {
    let input = concat!(
        "#+BEGIN_VERSE\n",
        "Great clouds overhead. Tiny black birds.\n",
        "#+END_VERSE\n",
        "After the block. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("Great clouds overhead.") && s.contains("Tiny black birds.")
        )),
        "closed verse body stays Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#+BEGIN_VERSE\n",
            "Great clouds overhead. Tiny black birds.\n",
            "#+END_VERSE\n",
            "After the block.\n",
            "Next.\n",
        ),
        "closed verse frozen; After. / Next. still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
