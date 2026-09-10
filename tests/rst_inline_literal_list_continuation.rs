use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #143 / snapper-nftt: list hang stays while an RST inline literal
/// is open, and for the following sentence in the same item.
///
/// `find_md_code_span` does not cross newlines, so ``not valid.`` is not
/// an atomic token. Delimiter merge keeps the period inside the open
/// ``...`` as one sentence; reflow must re-apply the two-space prefix.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        "- The error is ``not valid.\n",
        "  Choose from: one, two`` and continue.\n",
        "  A separate sentence.\n",
    )
}

#[test]
fn open_inline_literal_list_continuation_keeps_two_space_hang() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "open inline-literal list continuation must keep two-space hang, got:\n{out}"
    );
    assert!(
        out.contains("\n  Choose from: one, two`` and continue."),
        "literal continuation must be two spaces, got:\n{out}"
    );
    assert!(
        out.contains("\n  A separate sentence."),
        "following sentence in the same item must hang, got:\n{out}"
    );
    assert!(
        !out.contains("\nChoose from: one, two"),
        "must not outdent inside the open inline literal, got:\n{out}"
    );
    assert!(
        !out.contains("\nA separate sentence."),
        "must not outdent the following sentence, got:\n{out}"
    );
    let twice = format_text(&out, &rst_cfg()).unwrap();
    assert_eq!(
        out, twice,
        "hung open-literal list must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn fused_inline_literal_list_item_then_hung_sentence() {
    let input = concat!(
        "- The error is ``not valid. Choose from: one, two`` and continue.\n",
        "  A separate sentence.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "following sentence after a closed literal must keep two-space hang, got:\n{out}"
    );
}
