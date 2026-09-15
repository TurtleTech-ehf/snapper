//! Quoted pulldown footnote: opener is Structure; same-line body is
//! leftover Prose and still splits.

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
        "> [^1]: Footnote text. Second sentence.\n",
        "\n",
        "After the note. Next.\n",
    )
}

#[test]
fn quoted_footnote_marker_is_structure_body_is_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[^1]:")
        )),
        "quoted footnote opener must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Footnote text.") && p.contains("Second sentence.")
        )),
        "quoted footnote body must be leftover Prose, got {regions:?}"
    );
}

#[test]
fn quoted_footnote_continuation_after_blank_is_prose_not_code() {
    let input = concat!(
        "> [^1]: First sentence. Second sentence.\n",
        ">\n",
        ">     Continuation sentence. More.\n",
        ">\n",
        "> After the note. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("Continuation sentence.")
        )),
        "quoted footnote continuation must not be Code, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Continuation sentence.")
        )),
        "quoted footnote continuation must be leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Continuation sentence."),
        "continuation must remain, got:\n{out}"
    );
    assert!(
        out.contains("After the note.") && out.contains("Next."),
        "following quote prose must still split, got:\n{out}"
    );
}

#[test]
fn quoted_footnote_body_splits_and_following_does() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Footnote text.") && out.contains("Second sentence."),
        "quoted footnote body must still split, got:\n{out}"
    );
    assert!(
        !out.contains("Footnote text. Second sentence."),
        "quoted footnote body must not stay one sentence, got:\n{out}"
    );
    assert!(
        out.contains("After the note.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
