//! Pandoc line block: `| ` with no closing pipe is not a table row and
//! not a paragraph. The line stays whole. A following prose sentence
//! still splits. A flanking-pipe row stays a table.

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
        "| See Dr. Smith. He left.\n",
        "After the block. Still prose.\n",
    )
}

#[test]
fn line_block_is_structure_not_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "| See Dr. Smith. He left."
        )),
        "line block must be one Structure line, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Dr. Smith")
        )),
        "line block must not be Prose, got {regions:?}"
    );
}

#[test]
fn line_block_stays_one_line_and_following_prose_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("| See Dr. Smith. He left.\n"),
        "line block must stay one line, got:\n{out}"
    );
    assert!(
        !out.contains("| See Dr. Smith.\n"),
        "must not sentence-split the line block, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nStill prose.\n"),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn flanking_pipe_table_row_stays_a_table() {
    let input = concat!(
        "| Dr. Smith | He left. |\n",
        "| --- | --- |\n",
        "| A. | B. |\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().all(|r| matches!(r, Region::Structure(_))),
        "closing-pipe rows must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, input, "table bytes must stay, got:\n{out}");
}
