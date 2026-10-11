//! A ```` with no `''` after it is literal, as an unmatched backtick run
//! is in CommonMark. It must not hold the rest of the paragraph on one line.
//! A closed LaTeX quote still keeps its sentences together.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn cfg(format: Format) -> FormatConfig {
    FormatConfig {
        format,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn unclosed_double_backtick_does_not_stop_splits() {
    let input = "Unclosed `` tick here. Next sentence. Third one.\n";
    let out = format_text(input, &cfg(Format::Markdown)).unwrap();
    assert_eq!(
        out, "Unclosed `` tick here.\nNext sentence.\nThird one.\n",
        "an unmatched `` must not open a span, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg(Format::Markdown)).unwrap(), out);
}

#[test]
fn unclosed_double_backtick_after_a_closed_quote_still_splits() {
    let input = "He said ``Stop. Now.'' and left. Then `` stray. Next one.\n";
    let out = format_text(input, &cfg(Format::Plaintext)).unwrap();
    assert!(
        out.contains("``Stop. Now.''"),
        "a closed LaTeX quote must stay one span, got:\n{out}"
    );
    assert!(
        out.contains("stray.\nNext one.\n"),
        "the unmatched `` after it must not open a span, got:\n{out}"
    );
}

#[test]
fn closed_latex_quote_stays_whole() {
    let input = "He said ``Stop. Now.'' and left. Next sentence.\n";
    let out = format_text(input, &cfg(Format::Latex)).unwrap();
    assert!(
        out.contains("``Stop. Now.'' and left.\nNext sentence.\n"),
        "a closed LaTeX quote must stay one span, got:\n{out}"
    );
}
