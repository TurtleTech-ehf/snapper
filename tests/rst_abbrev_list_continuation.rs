use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #129 / snapper-u4lh: RST list continuations after `No.` / `etc.`
/// keep the two-space hang. Abbreviation merge leaves the join_prose_gap
/// newline inside one sentence; reflow must still hang that line.
#[test]
fn abbrev_list_continuation_keeps_two_space_hang() {
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
        "continuation after No./etc. must keep two-space hang, got:\n{out}"
    );
    assert!(
        out.contains("\n  A second sentence."),
        "No. continuation must be two spaces, got:\n{out}"
    );
    assert!(
        out.contains("\n  Another sentence."),
        "etc. continuation must be two spaces, got:\n{out}"
    );
    assert!(
        !out.contains("\nA second sentence."),
        "No. continuation must not outdent, got:\n{out}"
    );
    assert!(
        !out.contains("\nAnother sentence."),
        "etc. continuation must not outdent, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung abbrev list must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}
