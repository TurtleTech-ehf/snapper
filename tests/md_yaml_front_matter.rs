//! snapper-4why: pulldown `scan_metadata_block` / `scan_closing_metadata_block`.
//! `---` opens only when the next line is neither blank nor the closer.
//! Closer is `---` or `...`. Blank after `---` is a thematic break.

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

/// Ticket fixture (Format::Markdown).
fn ticket_fixture() -> &'static str {
    concat!(
        "---\n",
        "title: Hello. World. This is a long title that would reflow.\n",
        "...\n",
        "\n",
        "Body after yaml. Second sentence.\n",
    )
}

#[test]
fn ellipsis_closes_yaml_and_body_splits() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("title: Hello. World.")
        )),
        "YAML title must stay Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "...")),
        "... must close YAML, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("title:") || p.contains("Hello. World.")
        )),
        "YAML title must not reflow as Prose, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Body after yaml.")
        )),
        "body after YAML must be Prose, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("---\ntitle: Hello. World. This is a long title that would reflow.\n...\n"),
        "YAML block including ... closer must stay verbatim, got:\n{out}"
    );
    assert!(
        !out.contains("Hello.\nWorld"),
        "YAML title must not sentence-split, got:\n{out}"
    );
    assert!(
        out.contains("Body after yaml.\nSecond sentence."),
        "body after yaml must split, got:\n{out}"
    );
    assert!(
        !out.contains("Body after yaml. Second sentence."),
        "fused body must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn blank_after_opening_dashes_is_thematic_break() {
    let input = "---\n\nBody after yaml. Second sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "---")),
        "--- then blank is a thematic break, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Body after yaml.")
        )),
        "body after thematic break must be Prose, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.starts_with("---\n\nBody after yaml.\nSecond sentence."),
        "--- then blank must not swallow the file as Structure, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn dash_closer_still_ends_yaml() {
    let input = "---\ntitle: Hello. World.\n---\n\nBody after yaml. Second sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("title: Hello. World.")
        )),
        "classic --- closer must keep YAML as Structure, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Body after yaml.\nSecond sentence."),
        "body after --- closer must split, got:\n{out}"
    );
}
