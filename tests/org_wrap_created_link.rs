//! Wrap-created `[[bracket-link]]` at column 0 is standalone Structure.
//! Skip-cut must keep the link with the previous line.

use snapper_fmt::format::Format;
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
fn wrap_created_bracket_link_is_not_standalone_structure() {
    let input = "See the figure [[file:plot.png]] and more words after.\n";
    let token = "[[file:plot.png]]";
    let out = format_text(input, &wrap_cfg(20)).unwrap();
    assert!(
        !out.lines()
            .any(|l| l.trim() == token || l.trim_start().starts_with("[[file:")),
        "wrap must not park [[file:plot.png]] at column 0, got:\n{out}"
    );
    assert!(
        out.contains("figure [[file:plot.png]]")
            || out.contains("See the figure [[file:plot.png]]"),
        "skip-cut keeps the bracket link with the previous line, got:\n{out}"
    );
    assert!(
        out.contains(token),
        "bracket link must stay intact, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg(20)).unwrap(), out);
}
