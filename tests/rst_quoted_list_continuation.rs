use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #130 / snapper-kb7g: list continuation inside an open quote
/// keeps the two-space hang so Docutils still sees a list item.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        "* \"First sentence.\n",
        "  Second sentence.\"\n",
        "* Next item.\n",
    )
}

#[test]
fn open_quote_list_continuation_keeps_two_space_hang() {
    let cfg = rst_cfg();
    let input = ticket_fixture();
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "open-quote list continuation must keep two-space hang, got:\n{out}"
    );
    assert!(
        out.contains("\n  Second sentence.\""),
        "continuation must be two spaces, not column 0, got:\n{out}"
    );
    assert!(
        !out.contains("\nSecond sentence."),
        "must not outdent the quoted continuation, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung open-quote list must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn open_quote_three_line_list_keeps_hang() {
    let cfg = rst_cfg();
    let input = concat!(
        "* \"First sentence.\n",
        "  Second sentence.\n",
        "  Third sentence.\"\n",
        "* Next item.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "multi-line open quote must keep hang on every continuation, got:\n{out}"
    );
}
