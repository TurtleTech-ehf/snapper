//! GitHub #171 / snapper-zogf: CommonMark ATX headings allow 0–3
//! spaces of indent. `HEADING_RE` was `^(#{1,6}\\s+)`, so a
//! three-space `# Title` never matched and the title reflowed as Prose.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture: three-space ATX heading plus two-sentence body.
fn three_space_atx_fixture() -> &'static str {
    concat!(
        "   # Title. Still the title.\n",
        "\n",
        "Body sentence one. Body sentence two.\n",
    )
}

#[test]
fn three_space_atx_heading_is_structure_body_still_splits() {
    let input = three_space_atx_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "   # Title. Still the title.\n"
        )),
        "ATX line including the three spaces must be Structure, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("Still the title"))),
        "heading title must not be Prose: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("Body sentence one"))),
        "body must stay Prose, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.starts_with("   # Title. Still the title.\n"),
        "heading must stay one Structure line, got:\n{out}"
    );
    assert!(
        !out.contains("Title.\nStill") && !out.contains("Title.\n   Still"),
        "must not reflow the ATX title, got:\n{out}"
    );
    assert!(
        out.contains("Body sentence one.\nBody sentence two."),
        "body Prose must still split, got:\n{out}"
    );
    assert!(
        !out.contains("Body sentence one. Body sentence two."),
        "fused body must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn atx_heading_indent_zero_to_three_is_structure_four_is_code() {
    for n in 0..=3 {
        let pad = " ".repeat(n);
        let line = format!("{pad}# Title. Still the title.");
        let regions = MarkdownParser.parse(&line);
        assert_eq!(
            regions,
            vec![Region::Structure(line.clone())],
            "{n}-space ATX must be Structure"
        );
    }
    let four = "    # Title. Still the title.\n";
    let regions = MarkdownParser.parse(four);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Code { body, .. } if body.contains("# Title"))),
        "four-space line is indented code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("# Title"))),
        "four-space # must not be an ATX heading, got: {regions:?}"
    );
}
