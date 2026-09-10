//! GitHub #173 / snapper-9jyo: CommonMark 0.31.2 sec 4.7 allows dest
//! after one line ending. `is_link_reference_definition` is same-line
//! dest only; `[foo]:` then `/url/a.b` must still be one LRD Structure.

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
        "See [foo]. Next sentence.\n",
        "\n",
        "[foo]:\n",
        "/url/a.b\n",
        "\n",
        "After. More.\n",
    )
}

#[test]
fn dest_next_line_is_structure_not_prose() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]:")
        )),
        "[foo]: must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("/url/a.b")
        )),
        "dest line must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("[foo]:") || s.contains("/url/a.b")
        )),
        "CM 4.7 dest-next-line must not be Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("See [foo].") && s.contains("Next sentence.")
        )),
        "preceding paragraph must stay Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("After.") && s.contains("More.")
        )),
        "following paragraph must stay Prose, got {regions:?}"
    );
}

#[test]
fn dest_next_line_fixture_does_not_reflow_definition() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("See [foo].\nNext sentence."),
        "first paragraph must still reflow, got:\n{out}"
    );
    assert!(
        out.contains("[foo]:\n/url/a.b"),
        "label and dest must stay Structure lines, got:\n{out}"
    );
    assert!(
        !out.contains("[foo]: /url") && !out.contains("Next sentence. [foo]:"),
        "must not join dest onto the label or previous sentence, got:\n{out}"
    );
    assert!(
        out.contains("After.\nMore."),
        "following prose must still split, got:\n{out}"
    );
    assert!(
        !out.contains("After. More."),
        "fused following prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn dest_next_line_label_only_without_dest_stays_prose() {
    let input = "[foo]:\nAfter. More.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]:")
        )),
        "label-only without a dest line is not an LRD, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("[foo]:")
        )),
        "label-only without dest must stay Prose, got {regions:?}"
    );
}
