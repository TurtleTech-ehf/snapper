//! snapper-rrez: same-line lowercase proper noun after a period.
//!
//! Existing-newline `First sentence.\niCloud` stays two lines (snapper-7ict).
//! Same-line `First sentence. iCloud starts the second sentence.` must also
//! emit two lines under `format_text` `Format::Markdown` `max_width=0`.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

#[test]
fn same_line_icloud_emits_two_markdown_lines() {
    let input = "First sentence. iCloud starts the second sentence.\n";
    let expected = "First sentence.\niCloud starts the second sentence.\n";
    let cfg = FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    };
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(out, expected, "must split same-line iCloud, got:\n{out}");
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}

#[test]
fn existing_newline_icloud_stays_two_markdown_lines() {
    let input = "First sentence.\niCloud starts the second sentence.\n";
    let cfg = FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    };
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "must keep existing break before iCloud, got:\n{out}"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}
