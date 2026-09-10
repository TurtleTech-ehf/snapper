use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #106 / snapper-5m2a: CM 4.7 `[label]: dest` and pulldown `[^id]:`.
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
        "[foo]: https://example.com/a.b\n",
        "\n",
        "See [^1]. Next.\n",
        "\n",
        "[^1]: Footnote text. Second sentence.\n",
    )
}

#[test]
fn footnote_definition_is_structure_not_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[^1]: Footnote text. Second sentence.")
        )),
        "footnote definition must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("[^1]:") || s.contains("Footnote text")
        )),
        "footnote body must not be Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_link_ref_line_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]: https://example.com/a.b")
        )),
        "ticket [foo]: dest must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("[foo]:")
        )),
        "ticket [foo]: dest must not be Prose, got {regions:?}"
    );
}

#[test]
fn link_reference_after_blank_is_structure_not_prose() {
    let input = "See [foo]. Next sentence.\n\n[foo]: https://example.com/a.b\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]: https://example.com/a.b")
        )),
        "[foo]: dest after a blank must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("[foo]:")
        )),
        "[foo]: dest must not be Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_does_not_reflow_definitions() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("See [foo].\nNext sentence."),
        "first paragraph must still reflow, got:\n{out}"
    );
    assert!(
        out.contains("Next sentence.\n[foo]: https://example.com/a.b"),
        "link-reference dest must stay its own line, got:\n{out}"
    );
    assert!(
        !out.contains("Next sentence. [foo]:"),
        "must not glue dest onto the previous sentence, got:\n{out}"
    );
    assert!(
        out.contains("See [^1].\nNext."),
        "footnote reference paragraph must still reflow, got:\n{out}"
    );
    assert!(
        out.contains("[^1]: Footnote text. Second sentence."),
        "footnote definition must not sentence-split, got:\n{out}"
    );
    assert!(
        !out.contains("[^1]: Footnote text.\nSecond sentence."),
        "footnote must not leak a shorter body, got:\n{out}"
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
