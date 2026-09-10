use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #132 / snapper-rcgv: five-space indent after a nested list
/// item is a Docutils definition list. Collapsing it to four spaces
/// (marker hang) makes one paragraph.
#[test]
fn overindented_list_continuation_keeps_five_space_definition() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "* Parent\n",
        "\n",
        "  * Check all paths.\n",
        "     If a node is visited again, push it.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "five-space definition indent must stay, got:\n{out}"
    );
    assert!(
        out.contains("\n     If a node is visited again, push it."),
        "continuation must stay five spaces, got:\n{out}"
    );
    assert!(
        !out.contains("\n    If a node is visited again, push it."),
        "continuation must not collapse to four spaces, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung nested definition must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}
