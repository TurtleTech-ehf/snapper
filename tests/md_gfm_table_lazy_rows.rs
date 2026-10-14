//! A GFM table runs to a blank line or a block that interrupts a paragraph
//! (GFM ex. 202). A line under the last row with no pipe is one more row,
//! so splitting it would add a row. A list, heading or outer line after the
//! table still splits.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn fmt(input: &str) -> String {
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out, "not idempotent");
    out
}

#[test]
fn line_under_the_last_row_stays_a_row() {
    let input = "| a | b |\n|---|---|\n| x | y |\nAfter table. Next.\n\nOne here. Two here.\n";
    let out = fmt(input);
    assert!(
        out.contains("| x | y |\nAfter table. Next.\n"),
        "a pipe-less line under the table is a row, got:\n{out}"
    );
    assert!(
        out.contains("One here.\nTwo here.\n"),
        "the paragraph after the blank line must still split, got:\n{out}"
    );
}

#[test]
fn list_or_heading_after_the_table_still_splits() {
    let input = "| a | b |\n|---|---|\n| x | y |\n- Item one. Item two.\n\n\
                 | a | b |\n|---|---|\n| x | y |\n# Head\nPara one. Para two.\n";
    let out = fmt(input);
    assert!(
        out.contains("- Item one.\n  Item two.\n"),
        "a list item ends the table, got:\n{out}"
    );
    assert!(
        out.contains("# Head\nPara one.\nPara two.\n"),
        "a heading ends the table, got:\n{out}"
    );
}

#[test]
fn quoted_table_keeps_its_lazy_row() {
    let input = "> | q | w |\n> |---|---|\n> | e | t |\n> lazy quoted. Row too.\n";
    assert_eq!(fmt(input), input);
}

#[test]
fn outer_line_after_a_list_item_table_still_splits() {
    let input = "- item\n\n  | k | v |\n  |---|---|\n  | p | r |\nOuter line. Second one.\n";
    let out = fmt(input);
    assert!(
        out.contains("  | p | r |\nOuter line.\nSecond one.\n"),
        "a line outside the list item is not a row, got:\n{out}"
    );
}
