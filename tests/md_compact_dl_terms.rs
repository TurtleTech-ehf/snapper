//! Extra compact definition-list terms beyond the landed single-term case
//! stay Structure. pulldown ENABLE_DEFINITION_LIST.

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
        "Alpha term. Still alpha.\n",
        "Bravo term. Still bravo.\n",
        ": First definition sentence. Second sentence.\n",
        "\n",
        "After the list. Next.\n",
    )
}

#[test]
fn extra_compact_terms_are_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Alpha term. Still alpha.")
        )),
        "first compact term must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bravo term. Still bravo.")
        )),
        "second compact term must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still alpha") || p.contains("Still bravo")
        )),
        "compact terms must not be Prose, got {regions:?}"
    );
}

#[test]
fn list_item_compact_dl_terms_are_structure() {
    let input = concat!(
        "- Alpha term. Still alpha.\n",
        "  Bravo term. Still bravo.\n",
        "  : First definition sentence. Second sentence.\n",
        "\n",
        "After the list. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Alpha term. Still alpha.")
        )),
        "list-item compact term must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("First definition sentence.")
        )),
        "definition body must be leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("Alpha term.\n"),
        "must not sentence-split a compact term, got:\n{out}"
    );
    assert!(
        out.contains("After the list.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn extra_compact_terms_do_not_split_and_following_does() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Alpha term. Still alpha.\nBravo term. Still bravo.\n"),
        "compact terms must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains("Alpha term.\n"),
        "must not sentence-split a compact term, got:\n{out}"
    );
    assert!(
        out.contains(": First definition sentence.\n  Second sentence."),
        "definition body must still hang and split, got:\n{out}"
    );
    assert!(
        out.contains("After the list.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
