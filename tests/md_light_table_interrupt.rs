//! pulldown GFM 4.10: a light (pipe-less) table does not interrupt a
//! paragraph. Only a heavy table (header starts with `|`) does.

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
        "Foo is a sentence. Bar is another.\n",
        "alpha. | bravo.\n",
        "--- | ---\n",
        "After. Next.\n",
    )
}

#[test]
fn light_table_after_prose_stays_paragraph() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("alpha.") && p.contains("| bravo.")
        )),
        "light table after prose must stay Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("alpha.")
        )),
        "light table after prose must not become Structure, got {regions:?}"
    );
}

#[test]
fn light_table_after_prose_still_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Foo is a sentence.\nBar is another."),
        "Foo / Bar must still split, got:\n{out}"
    );
    assert!(
        out.contains("alpha.") && out.contains("bravo."),
        "light row must remain, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "After. / Next. must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn heavy_table_still_interrupts() {
    let input = concat!(
        "Foo is a sentence. Bar is another.\n",
        "| alpha. | bravo. |\n",
        "| --- | --- |\n",
        "After. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("| alpha.")
        )),
        "heavy table must still interrupt, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
}
