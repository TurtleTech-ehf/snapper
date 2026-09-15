//! Wrap-created file: / http:// at column 0 is leftover plain-link,
//! not a block. Skip-cut keeps the token with the previous line.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn wrap_cfg(width: usize) -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: width,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn wrap_does_not_park_file_colon_at_column_zero() {
    let input = "See the figure file:/tmp/plot.png extra words here.\n";
    let out = format_text(input, &wrap_cfg(28)).unwrap();
    assert!(
        !out.lines().any(|l| l.starts_with("file:")),
        "wrap must not park file: at column 0, got:\n{out}"
    );
    let regions = OrgParser.parse(&out);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("file:/tmp/plot.png extra")
        )),
        "wrap-created file: must not freeze the rest of the line, got {regions:?}"
    );
    assert_eq!(format_text(&out, &wrap_cfg(28)).unwrap(), out);
}

#[test]
fn wrap_does_not_park_begin_env_at_column_zero() {
    let input =
        "See the display \\begin{equation} x = 1. 2 \\end{equation} extra words here after.\n";
    let out = format_text(input, &wrap_cfg(24)).unwrap();
    assert!(
        !out.lines().any(|l| l.starts_with("\\begin{")),
        "wrap must not park \\\\begin{{ at column 0, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg(24)).unwrap(), out);
}

#[test]
fn wrap_does_not_park_http_at_column_zero() {
    let input = "See the site http://example.com/a extra words here.\n";
    let out = format_text(input, &wrap_cfg(24)).unwrap();
    assert!(
        !out.lines().any(|l| l.starts_with("http://")),
        "wrap must not park http:// at column 0, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg(24)).unwrap(), out);
}
