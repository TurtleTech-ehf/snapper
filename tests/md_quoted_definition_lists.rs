//! Quoted compact definition lists stay Structure like unquoted.
//! pulldown ENABLE_DEFINITION_LIST after `>`.

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
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "> Term\n",
        "> : First definition sentence. Second sentence.\n",
        "\n",
        "After the quote. Next.\n",
    )
}

#[test]
fn quoted_dl_term_and_marker_are_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains('>') && s.contains("Term")
        )),
        "quoted term must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(':') && !s.contains("First definition")
        )),
        "quoted :  marker must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Term")
        )),
        "quoted term must not be Prose, got {regions:?}"
    );
}

#[test]
fn quoted_dl_body_hangs_and_following_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("> Term\n"),
        "quoted term must stay, got:\n{out}"
    );
    assert!(
        out.contains("> : First definition sentence."),
        "quoted marker plus first sentence must stay, got:\n{out}"
    );
    assert!(
        !out.contains("> : First definition sentence. Second sentence."),
        "quoted definition body must split, got:\n{out}"
    );
    assert!(
        out.contains("After the quote.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
