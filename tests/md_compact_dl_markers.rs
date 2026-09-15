//! Extra compact definition-list markers (`~` / `:`) beyond the landed
//! single `: ` case stay Structure. pulldown `:` plus leftover `~`.

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
        "Term\n",
        ": First definition sentence. Second sentence.\n",
        "~ Third definition sentence. Fourth sentence.\n",
        "\n",
        "After the list. Next.\n",
    )
}

#[test]
fn extra_compact_markers_are_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ": ")),
        ":  marker must be Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "~ ")),
        "~  marker must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.starts_with("~ ") || p.contains("~ Third")
        )),
        "compact ~ marker must not stay Prose, got {regions:?}"
    );
}

#[test]
fn extra_compact_markers_hang_and_following_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains(": First definition sentence.\n  Second sentence."),
        ": body must hang and split, got:\n{out}"
    );
    assert!(
        out.contains("~ Third definition sentence.\n  Fourth sentence."),
        "~ body must hang and split, got:\n{out}"
    );
    assert!(
        !out.contains("~ Third definition sentence. Fourth sentence."),
        "fused ~ body must not survive, got:\n{out}"
    );
    assert!(
        out.contains("After the list.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
