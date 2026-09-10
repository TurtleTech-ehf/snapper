use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #129 / snapper-u4lh: compact RST list hang after `No.` / `etc.`
/// stays at two spaces so Docutils keeps the continuation in the item.
#[test]
fn rst_list_continuation_after_abbreviation_keeps_hang() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "* **Answer**: No.\n",
        "  A second sentence.\n",
        "* Uses queues, caches, etc.\n",
        "  Another sentence.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "abbreviation list continuation must keep two-space hang, got:\n{out}"
    );
    assert!(
        out.contains("\n  A second sentence."),
        "continuation after No. must be two spaces, got:\n{out}"
    );
    assert!(
        out.contains("\n  Another sentence."),
        "continuation after etc. must be two spaces, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung abbreviation list must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}
