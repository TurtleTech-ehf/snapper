//! org-element latex-fragment: unmatched leftover-start `\[` / `$$` is
//! not a fragment (a paragraph). Closed `\[...\]` / `$$...$$` unchanged.

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

fn unmatched_bracket_fixture() -> &'static str {
    concat!(
        "\\[\n",
        "This is a long sentence that must reflow because there is no closer.\n",
        "After. Next.\n",
    )
}

fn unmatched_dollar_fixture() -> &'static str {
    concat!(
        "$$\n",
        "This is a long sentence that must reflow because there is no closer.\n",
        "After. Next.\n",
    )
}

fn assert_unmatched_is_paragraph(input: &str, opener: &str) {
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long sentence that must reflow")
                    && s.contains("After.")
                    && s.contains("Next.")
        )),
        "unmatched {opener} is a paragraph, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("This is a long sentence that must reflow")
                    || s.contains("After.")
        )),
        "unmatched {opener} must not swallow as Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.lines().any(|l| l.trim() == opener),
        "unmatched {opener} must join the paragraph, not stay a lone structure line, got:\n{out}"
    );
    assert!(
        out.contains("After.") && out.contains("Next."),
        "After. / Next. must stay Prose after unmatched {opener}, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn unmatched_leftover_start_bracket_is_paragraph() {
    assert_unmatched_is_paragraph(unmatched_bracket_fixture(), r"\[");
}

#[test]
fn unmatched_leftover_start_dollars_is_paragraph() {
    let input = unmatched_dollar_fixture();
    assert_unmatched_is_paragraph(input, "$$");
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("After.\nNext."),
        "After. / Next. must still split after unmatched $$, got:\n{out}"
    );
    assert!(
        !out.contains("After. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
}

#[test]
fn closed_bracket_display_still_structure() {
    let input = concat!(
        "\\[\n",
        "This is a long sentence that must stay inside display math and must not reflow as prose.\n",
        "\\]\n",
        "After. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("This is a long sentence that must stay inside display math")
        )),
        "closed \\[ body must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains(
            "\\[\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n\\]"
        ),
        "closed \\[ must stay a structure block, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after closed \\] must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn closed_dollar_display_still_structure() {
    let input = concat!(
        "$$\n",
        "This is a long sentence that must stay inside display math and must not reflow as prose.\n",
        "$$\n",
        "After. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("This is a long sentence that must stay inside display math")
        )),
        "closed $$ body must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains(
            "$$\nThis is a long sentence that must stay inside display math and must not reflow as prose.\n$$"
        ),
        "closed $$ must stay a structure block, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after closed $$ must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
