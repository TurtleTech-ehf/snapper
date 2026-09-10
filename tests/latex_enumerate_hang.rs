use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

#[test]
fn enumerate_item_hangs_next_sentence() {
    let cfg = FormatConfig {
        format: Format::Latex,
        max_width: 0,
        ..Default::default()
    };
    let input = "\\begin{enumerate}\n\\item First sentence. Second sentence.\n\\end{enumerate}\n";
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        "\\begin{enumerate}\n\\item First sentence.\n      Second sentence.\n\\end{enumerate}\n",
        "enumerate item must hang at \\\\item  width, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(out, twice, "hung enumerate must be identity, got:\n{twice}");
}
