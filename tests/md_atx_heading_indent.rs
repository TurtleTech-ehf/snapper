use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #171 / snapper-zogf: CommonMark 0.31.2 §4.2 ATX headings
/// allow 0–3 spaces of indent. `HEADING_RE` used to require column 0.

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
        "   # Title. Still the title.\n",
        "\n",
        "Body sentence one. Body sentence two.\n",
    )
}

#[test]
fn three_space_atx_line_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        matches!(
            &regions[0],
            Region::Structure(s) if s == "   # Title. Still the title.\n"
        ),
        "ATX line including the three spaces must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Title") || p.contains('#')
        )),
        "indented ATX title must not be Prose: {regions:?}"
    );
}

#[test]
fn three_space_atx_body_prose_still_splits() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Body sentence one")
                && p.contains("Body sentence two")
        )),
        "body must stay Prose so it can split, got: {regions:?}"
    );
    let out = format_text(ticket_fixture(), &md_cfg()).unwrap();
    assert!(
        out.starts_with("   # Title. Still the title.\n"),
        "indented ATX must stay one line, got:\n{out}"
    );
    assert!(
        out.contains("Body sentence one.\nBody sentence two."),
        "body Prose must still split, got:\n{out}"
    );
    assert!(
        !out.contains("# Title.\n"),
        "must not reflow the ATX title, got:\n{out}"
    );
    let twice = format_text(&out, &md_cfg()).unwrap();
    assert_eq!(
        out, twice,
        "indented ATX fixture must be identity, got:\n{twice}"
    );
}
