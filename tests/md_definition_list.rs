//! GitHub #210: Markdown definition lists stay structure.
//! pulldown ENABLE_DEFINITION_LIST: Term is Structure; `: ` is Structure;
//! the body hangs and splits. A column-0 second sentence is wrong.

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
    "Term\n: This is a long definition sentence that must hang. Second sentence.\n"
}

#[test]
fn definition_term_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("Term"))),
        "Term must stay Structure, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("Term"))),
        "Term must not join the body as Prose, got: {regions:?}"
    );
}

#[test]
fn definition_marker_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ": ")),
        "`: ` must stay Structure, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.starts_with(':') || p.contains(": This"))),
        "`: ` must not join the body as Prose, got: {regions:?}"
    );
}

#[test]
fn definition_body_hangs_and_splits() {
    let out = format_text(ticket_fixture(), &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Term\n",
            ": This is a long definition sentence that must hang.\n",
            "  Second sentence.\n",
        ),
        "Term stays; `: ` stays; body hangs and splits, got:\n{out}"
    );
    assert!(
        !out.contains("\nSecond sentence."),
        "second sentence must not land at column 0, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn definition_list_survives_safety_backstops() {
    let input = ticket_fixture();
    let cfg = FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    };
    let out = format_text(input, &cfg).unwrap();
    assert!(
        out.contains(": This is a long definition sentence that must hang.\n  Second sentence."),
        "CLI backstops must not revert the hung split, got:\n{out}"
    );
    assert!(
        !out.contains("\nSecond sentence."),
        "CLI backstops must not emit a column-0 second sentence, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle must accept the reflow\n in={input:?}\n out={out:?}"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}

#[test]
fn indented_definition_marker_hangs() {
    let input = "Term\n  : This is a long definition sentence that must hang. Second sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "  : ")),
        "indented `: ` must stay Structure, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Term\n",
            "  : This is a long definition sentence that must hang.\n",
            "    Second sentence.\n",
        ),
        "indented marker hangs at its width, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn lone_colon_line_is_not_a_definition() {
    let input = ": This is a long definition sentence that must hang. Second sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains(": This is a long definition sentence")
        )),
        "a leading `: ` without a term is prose, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ": ")),
        "lone `: ` must not invent a definition marker, got: {regions:?}"
    );
}
