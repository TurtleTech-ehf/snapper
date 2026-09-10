use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #125 / snapper-435i: same-line nested RST list hang stays
/// at the inner item so Docutils does not treat the continuation as
/// a sibling.
#[test]
fn nested_same_line_list_continuation_hangs_at_inner_width() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = "- - Document typed fixture exports. The package already includes ``py.typed``.\n";
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        concat!(
            "- - Document typed fixture exports.\n",
            "    The package already includes ``py.typed``.\n",
        ),
        "inner nested list must keep four-space continuation, got:\n{out}"
    );
    assert!(
        out.contains("\n    The package already includes ``py.typed``."),
        "continuation must be four spaces, not two, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung nested list must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );

    let already_hung = concat!(
        "- - Document typed fixture exports.\n",
        "    The package already includes ``py.typed``.\n",
    );
    let hung_out = format_text(already_hung, &cfg).unwrap();
    assert_eq!(
        hung_out, already_hung,
        "four-space nested continuation must stay identity, got:\n{hung_out}"
    );
}
