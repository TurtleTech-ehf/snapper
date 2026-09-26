//! Pandoc example list `(@)` is a list marker. The marker stays on the
//! item line. A second sentence may hang on a continuation line and
//! must not drop to column 0.

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

#[test]
fn example_marker_is_structure_and_body_stays_prose() {
    let input = "(@) Example one. Second sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "(@) ")),
        "(@) marker must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Example one.") && p.contains("Second sentence.")
        )),
        "item body must stay Prose, got {regions:?}"
    );
}

#[test]
fn example_second_sentence_hangs_inside_the_item() {
    let input = "(@) Example one. Second sentence.\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, "(@) Example one.\n    Second sentence.\n",
        "second sentence must hang under (@), got:\n{out}"
    );
    assert!(
        !out.contains("\nSecond sentence."),
        "second sentence must not leave the item, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
