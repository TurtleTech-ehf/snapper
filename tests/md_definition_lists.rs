//! GitHub #210 / snapper-r9tq: Markdown definition lists hang.
//!
//! pulldown `ENABLE_DEFINITION_LIST` / `scan_definition_list_definition_marker_with_indent`.
//! Term is Structure. `: ` is Structure. The body hangs and splits.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{format_text, FormatConfig};

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
fn term_is_structure_marker_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "Term\n")),
        "Term must be Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ": ")),
        ":  marker must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long definition sentence that must hang.")
                    && s.contains("Second sentence.")
        )),
        "definition body must be Prose, got {regions:?}"
    );
}

#[test]
fn definition_body_hangs_and_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Term\n",
            ": This is a long definition sentence that must hang.\n",
            "  Second sentence.\n",
        ),
        "Term stays; :  stays; body hangs and splits, got:\n{out}"
    );
    assert!(
        !out.lines().any(|l| l == "Second sentence."),
        "must not emit a column-0 second sentence, got:\n{out}"
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
    assert_eq!(
        out,
        concat!(
            "Term\n",
            ": This is a long definition sentence that must hang.\n",
            "  Second sentence.\n",
        ),
        "CLI backstops must not revert the hang, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle must accept the reflow\n in={input:?}\n out={out:?}"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}

#[test]
fn following_prose_after_definition_still_splits() {
    let input = concat!(
        "Term\n",
        ": This is a long definition sentence that must hang. Second sentence.\n",
        "\n",
        "After the list. Next.\n",
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Term\n",
            ": This is a long definition sentence that must hang.\n",
            "  Second sentence.\n",
            "\n",
            "After the list.\n",
            "Next.\n",
        ),
        "following prose must still reflow, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn indented_definition_marker_hangs_at_marker_width() {
    let input = "Term\n  : This is a long definition sentence that must hang. Second sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "  : ")),
        "indented :  marker must be Structure, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "Term\n",
            "  : This is a long definition sentence that must hang.\n",
            "    Second sentence.\n",
        ),
        "indented marker must hang at width 4, got:\n{out}"
    );
    assert!(
        !out.lines().any(|l| l == "Second sentence."),
        "must not emit a column-0 second sentence, got:\n{out}"
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
