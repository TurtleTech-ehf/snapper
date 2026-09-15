//! Multiline link-reference dest title: `[ref]: url` then title on the
//! next line stay Structure. CommonMark 0.31.2 §4.7. Following prose splits.

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
        "See [ref]. Next sentence.\n",
        "\n",
        "[ref]: https://example.com/a.b\n",
        "\"Title with a period. Still title.\"\n",
        "\n",
        "After the definition. More.\n",
    )
}

#[test]
fn dest_and_next_line_title_are_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[ref]: https://example.com/a.b")
        )),
        "LRD dest must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Title with a period. Still title.")
        )),
        "next-line title must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("[ref]:") || p.contains("Still title.")
        )),
        "dest and title must not be Prose, got {regions:?}"
    );
}

#[test]
fn dest_title_stay_and_following_prose_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("[ref]: https://example.com/a.b\n\"Title with a period. Still title.\""),
        "dest plus title must stay Structure lines, got:\n{out}"
    );
    assert!(
        !out.contains("Title with a period.\n"),
        "must not sentence-split the link title, got:\n{out}"
    );
    assert!(
        out.contains("After the definition.\nMore."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

fn indented_title_fixture() -> &'static str {
    concat!(
        "See [foo]. Next sentence.\n",
        "\n",
        "   [foo]:\n",
        "      /url\n",
        "           'Title with a period. Still title.'\n",
        "\n",
        "After the definition. More.\n",
    )
}

#[test]
fn indented_lrd_title_is_structure_not_code() {
    let regions = MarkdownParser.parse(indented_title_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]:")
        )),
        "label-only LRD must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("/url")
        )),
        "indented dest must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Title with a period. Still title.")
        )),
        "indented title must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(r, Region::Code { .. })),
        "indented LRD title must not be indented code, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("/url")
        )),
        "dest and title must not be Prose, got {regions:?}"
    );
}

#[test]
fn indented_lrd_title_does_not_split_and_following_does() {
    let input = indented_title_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("           'Title with a period. Still title.'"),
        "indented title must stay one Structure line, got:\n{out}"
    );
    assert!(
        !out.contains("Title with a period.\n"),
        "must not sentence-split the indented title, got:\n{out}"
    );
    assert!(
        out.contains("After the definition.\nMore."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
