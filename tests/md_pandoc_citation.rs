//! Pandoc citation `[@doe2020, see this. Then that]` is one span.
//! A sentence outside it still splits.

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

#[test]
fn pandoc_citation_stays_one_span_and_outside_splits() {
    let input = "See [@doe2020, see this. Then that] for details. Next sentence.\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("[@doe2020, see this. Then that]"),
        "citation must stay one span, got:\n{out}"
    );
    assert!(
        !out.contains("see this.\n"),
        "must not split inside the citation, got:\n{out}"
    );
    assert!(
        out.contains("for details.\nNext sentence.\n"),
        "a sentence outside the citation must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
