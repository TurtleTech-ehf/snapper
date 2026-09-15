//! Wrap-created `[fn:LABEL]` at column 0 is a footnote definition.
//! Skip-cut must keep the marker with the previous line.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn wrap_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 23,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn wrap_created_fn_marker_is_not_a_definition() {
    let input = "The options are apples [fn:1] extra words here. Next.\n";
    let out = format_text(input, &wrap_cfg()).unwrap();
    assert!(
        !out.lines().any(|l| l.trim_start().starts_with("[fn:")),
        "wrap must not park [fn:1] at column 0, got:\n{out}"
    );
    assert!(
        out.contains("apples [fn:1]"),
        "skip-cut keeps [fn:1] with the previous line, got:\n{out}"
    );
    assert!(
        out.contains("Next."),
        "Next. must still be present, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg()).unwrap(), out);
}
