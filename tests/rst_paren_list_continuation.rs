use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #131 / snapper-cjrg: an open parenthesis keeps the list-item
/// hang on the next source line so Docutils does not leave the item.
#[test]
fn open_paren_list_continuation_keeps_two_space_hang() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "- Ordering (smaller indices mean higher priority.\n",
        "  Recurse to the left side of the array)\n",
        "- Next item.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "open-paren continuation must stay two-space hung, got:\n{out}"
    );
    assert!(
        out.contains("\n  Recurse to the left side of the array)"),
        "continuation must be two spaces, not column 0, got:\n{out}"
    );
    assert!(
        !out.contains("\nRecurse to the left side of the array)"),
        "continuation must not outdent, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung paren continuation must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );

    let fused = concat!(
        "- Ordering (smaller indices mean higher priority. Recurse to the left side of the array)\n",
        "- Next item.\n",
    );
    let fused_out = format_text(fused, &cfg).unwrap();
    assert_eq!(
        fused_out, fused,
        "one-line paren item must stay one sentence, got:\n{fused_out}"
    );
}
