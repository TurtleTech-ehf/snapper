//! Four-space nested-looking list after a parent item is a nested list
//! (pulldown / CM 5.2: hang ≤ indent < hang+4), not indented code.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    concat!(
        "- Parent item is a sentence. Second sentence.\n",
        "    - Child item is a sentence. Second sentence.\n",
        "\n",
        "After the list. Next.\n",
    )
}

#[test]
fn four_space_nested_list_marker_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains('-') && s.starts_with("    -")
        )),
        "4-space nested marker must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Child item is a sentence.")
        )),
        "nested item body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(r, Region::Code { .. })),
        "4-space nested list must not be indented code, got {regions:?}"
    );
}

#[test]
fn four_space_nested_list_hangs_and_following_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("    - Child item is a sentence."),
        "nested marker must stay, got:\n{out}"
    );
    assert!(
        !out.contains("    - Child item is a sentence. Second sentence."),
        "nested item must still split, got:\n{out}"
    );
    assert!(
        out.contains("- Parent item is a sentence.\n  Second sentence."),
        "parent item must still split, got:\n{out}"
    );
    assert!(
        out.contains("After the list.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
