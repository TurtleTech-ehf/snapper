//! CM 0.31.2 §4.7: an LRD may live inside a list item.

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
        "- Item one is a sentence. Second sentence.\n",
        "\n",
        "  [ref]: https://example.com/a.b\n",
        "  \"Title with a period. Still title.\"\n",
        "\n",
        "  After the definition. Next.\n",
    )
}

#[test]
fn list_item_lrd_is_structure_and_after_hangs() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[ref]: https://example.com/a.b")
        )),
        "list-item dest must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Title with a period. Still title.")
        )),
        "list-item title must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After the definition.") && p.contains("Next.")
        )),
        "After. must stay Prose in the item, got {regions:?}"
    );
}

#[test]
fn list_item_lrd_after_hangs_and_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("  [ref]: https://example.com/a.b"),
        "dest must stay indented Structure, got:\n{out}"
    );
    assert!(
        !out.contains("Title with a period.\n"),
        "must not sentence-split the title, got:\n{out}"
    );
    assert!(
        out.contains("After the definition.") && out.contains("Next."),
        "After. / Next. must still split, got:\n{out}"
    );
    assert!(
        !out.contains("\nAfter the definition."),
        "After. must hang in the list item, not drop to column 0, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
