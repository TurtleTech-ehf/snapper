//! GitHub #169 / snapper-u77y: Org `file:` tokens must not swallow
//! trailing sentence punctuation.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn org_file_token_same_line_splits() {
    let two_line = "See file:/tmp/foo.\nNext sentence.\n";
    let out = format_text(two_line, &org_cfg()).unwrap();
    assert_eq!(
        out, two_line,
        "existing newline must stay two lines, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

    let out = format_text("See file:/tmp/foo. Next sentence.\n", &org_cfg()).unwrap();
    assert_eq!(
        out, two_line,
        "same-line file: period must split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn org_file_token_bang_and_question_split() {
    let cfg = org_cfg();
    let out = format_text("See file:/tmp/foo! Next sentence.\n", &cfg).unwrap();
    assert_eq!(out, "See file:/tmp/foo!\nNext sentence.\n", "got:\n{out}");
    let out = format_text("See file:/tmp/foo? Next sentence.\n", &cfg).unwrap();
    assert_eq!(out, "See file:/tmp/foo?\nNext sentence.\n", "got:\n{out}");
}
