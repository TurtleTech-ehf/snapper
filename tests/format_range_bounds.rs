//! `format_range` receives raw bounds from the WebAssembly and MCP entry
//! points, which skip the CLI's range validation.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_range};

fn cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Plaintext,
        max_width: 0,
        ..Default::default()
    }
}

#[test]
fn zero_end_leaves_the_text_unchanged() {
    let input = "A one. B two.\n";
    assert_eq!(format_range(input, &cfg(), 0, 0).unwrap(), input);
    assert_eq!(format_range(input, &cfg(), 1, 0).unwrap(), input);
}

#[test]
fn inverted_range_leaves_the_text_unchanged() {
    let input = "A one. B two.\nC three.\nD four. E five.\n";
    assert_eq!(format_range(input, &cfg(), 3, 1).unwrap(), input);
}

#[test]
fn empty_text_with_any_range_stays_empty() {
    assert_eq!(format_range("", &cfg(), 1, 1).unwrap(), "");
    assert_eq!(format_range("", &cfg(), 2, 1).unwrap(), "");
}
