use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #131 / snapper-cjrg: list hang stays while a parenthesis is open.
/// The splitter keeps `(priority.\nRecurse)` as one sentence, so reflow
/// must re-apply the two-space prefix to the source newline.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        "- Ordering (smaller indices mean higher priority.\n",
        "  Recurse to the left side of the array)\n",
        "- Next item.\n",
    )
}

#[test]
fn open_paren_list_continuation_keeps_two_space_hang() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "open-paren list continuation must keep two-space hang, got:\n{out}"
    );
    assert!(
        out.contains("\n  Recurse to the left side of the array)"),
        "continuation must be two spaces, got:\n{out}"
    );
    assert!(
        !out.contains("\nRecurse to the left side of the array)"),
        "must not outdent inside the open parenthesis, got:\n{out}"
    );
    let twice = format_text(&out, &rst_cfg()).unwrap();
    assert_eq!(
        out, twice,
        "hung open-paren list must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn fused_paren_list_item_stays_one_sentence() {
    let input = "- Ordering (smaller indices mean higher priority. Recurse to the left side of the array)\n- Next item.\n";
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "must not split inside open parentheses, got:\n{out}"
    );
}
