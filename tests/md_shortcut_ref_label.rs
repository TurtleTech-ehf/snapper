//! CommonMark shortcut reference `[Theorem. Proof]` is one span.
//! A sentence outside it still splits. Full and collapsed references
//! stay on their existing path.

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
fn shortcut_label_stays_one_span_and_outside_splits() {
    let input = "See [Theorem. Proof] for details. Next sentence.\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("[Theorem. Proof]"),
        "shortcut label must stay one span, got:\n{out}"
    );
    assert!(
        !out.contains("[Theorem.\n") && !out.contains("Proof]\nfor"),
        "must not split inside the shortcut label, got:\n{out}"
    );
    assert!(
        out.contains("for details.\nNext sentence.\n"),
        "a sentence outside the label must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
