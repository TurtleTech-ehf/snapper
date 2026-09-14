//! Quoted LRD dest/title stay Structure. Quoted LRD does not interrupt
//! a quote paragraph. CommonMark 0.31.2 §4.7.

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
fn quoted_lrd_title_next_line_is_structure() {
    let input = concat!(
        "> [ref]: https://example.com/a.b\n",
        "> \"Title with a period. Still title.\"\n",
        "\n",
        "After the definition. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[ref]: https://example.com/a.b")
        )),
        "quoted dest must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Title with a period. Still title.")
        )),
        "quoted next-line title must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title")
        )),
        "quoted title must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("Title with a period.\n"),
        "must not sentence-split the quoted title, got:\n{out}"
    );
    assert!(
        out.contains("After the definition.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn quoted_lrd_dest_next_line_is_structure() {
    let input = concat!(
        "> [foo]:\n",
        "> /url/a.b\n",
        "> \"Title with a period. Still title.\"\n",
        "\n",
        "After the definition. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]:")
        )),
        "quoted label-only must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("/url/a.b")
        )),
        "quoted dest-next-line must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Title with a period. Still title.")
        )),
        "quoted title must be Structure, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("Title with a period.\n"),
        "must not sentence-split the quoted title, got:\n{out}"
    );
    assert!(
        out.contains("After the definition.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn quoted_lrd_does_not_interrupt_quote_paragraph() {
    let input = concat!(
        "> Foo is a sentence. Bar is another.\n",
        "> [foo]: /url/a.b\n",
        "> After. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]:")
        )),
        "quoted LRD without a blank must stay in the quote paragraph, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("[foo]: /url/a.b")
        )),
        "quoted LRD must stay Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Foo is a sentence.") && out.contains("Bar is another."),
        "Foo / Bar must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.") && out.contains("Next."),
        "After. / Next. must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
