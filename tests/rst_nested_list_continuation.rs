use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #125 / snapper-435i: compact nested RST list hang stays inside
/// the inner item. Four-space continuation must not collapse to two.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        "- - Document typed fixture exports.\n",
        "    The package already includes ``py.typed``.\n",
    )
}

#[test]
fn nested_list_continuation_keeps_four_space_hang() {
    let cfg = rst_cfg();
    let input = ticket_fixture();
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "four-space nested continuation must stay inside the inner item, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung nested list must be identity, got:\n{twice}"
    );
}

#[test]
fn nested_list_sentence_split_hangs_at_inner_marker() {
    let cfg = rst_cfg();
    let input = "- - Document typed fixture exports. The package already includes ``py.typed``.\n";
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        ticket_fixture(),
        "inner hang width must be four spaces after the split, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "split nested list must be identity, got:\n{twice}"
    );
}
