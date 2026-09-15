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

#[test]
fn footnote_continuation_after_blank_is_prose_not_code() {
    let input = concat!(
        "[^1]: First sentence. Second sentence.\n",
        "\n",
        "    Continuation sentence. More.\n",
        "\n",
        "After the note. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("Continuation sentence.")
        )),
        "footnote continuation after a blank must not be Code, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Continuation sentence.") && p.contains("More.")
        )),
        "footnote continuation must be leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Continuation sentence."),
        "continuation must remain Prose, got:\n{out}"
    );
    assert!(
        !out.contains("    Continuation sentence. More.\n"),
        "continuation must not stay one Code line, got:\n{out}"
    );
    assert!(
        out.contains("After the note.\nNext."),
        "following prose must still split, got:\n{out}"
    );
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
fn leftover_footnote_col0_after_continuation_is_new_paragraph() {
    let input = concat!(
        "[^1]: First sentence. Second sentence.\n",
        "    Continuation sentence. More.\n",
        "After the note. Next.\n",
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("More. After the note."),
        "col-0 after a footnote continuation must not lazy-join, got:\n{out}"
    );
    assert!(
        out.contains("After the note.\nNext."),
        "following prose must still split, got:\n{out}"
    );
}

#[test]
fn leftover_footnote_tab_is_continuation() {
    let input = concat!(
        "[^1]: First sentence. Second sentence.\n",
        "\tContinuation sentence. More.\n",
        "\n",
        "After the note. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Continuation sentence.")
        )),
        "tab continuation must stay leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("After the note.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn leftover_footnote_two_space_is_not_continuation() {
    let input = concat!(
        "[^1]: First sentence. Second sentence.\n",
        "  Continuation sentence. More.\n",
        "After the note. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Continuation sentence")
        )),
        "2-space line must leave the footnote, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Continuation sentence.")
        )),
        "2-space line must be document Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("First sentence.\nSecond sentence."),
        "footnote opener body must still split, got:\n{out}"
    );
    assert!(
        out.contains("After the note.\nNext.") || out.contains("More.\nAfter the note."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn footnote_definition_marker_is_structure_body_is_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "[^1]: ")),
        "footnote opener must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Footnote text.") && s.contains("Second sentence.")
        )),
        "footnote body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("[^1]:")
        )),
        "footnote marker must not stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_link_ref_line_stays_in_paragraph() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("See [foo].")
                    && s.contains("Next sentence.")
                    && s.contains("[foo]: https://example.com/a.b")
        )),
        "no-blank [foo]: dest stays in the paragraph, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]:")
        )),
        "no-blank LRD must not be Structure, got {regions:?}"
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
        out.contains("[^1]: Footnote text.\nSecond sentence."),
        "footnote body must sentence-split, got:\n{out}"
    );
    assert!(
        !out.contains("[^1]: Footnote text. Second sentence."),
        "footnote body must not stay fused, got:\n{out}"
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
