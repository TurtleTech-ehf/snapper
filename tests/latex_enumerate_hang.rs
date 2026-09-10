//! LaTeX enumerate hang (snapper-sawy / GitHub #60 class).
//! Fails on origin/main: the second sentence starts at column 0.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

#[test]
fn enumerate_item_second_sentence_hangs_under_item() {
    let input = "\\begin{enumerate}\n\\item First sentence. Second sentence.\n\\end{enumerate}\n";
    let cfg = FormatConfig {
        format: Format::Latex,
        max_width: 0,
        ..Default::default()
    };
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        "\\begin{enumerate}\n\\item First sentence.\n      Second sentence.\n\\end{enumerate}\n",
        "second sentence must hang under the item body, got:\n{out}"
    );
    let lines: Vec<_> = out.lines().collect();
    assert_eq!(
        lines[0], "\\begin{enumerate}",
        "env opener must stay structure"
    );
    assert_eq!(lines[1], "\\item First sentence.");
    assert_eq!(
        lines[2], "      Second sentence.",
        "hang is spaces after `\\item `, not column 0"
    );
    assert_eq!(
        lines[3], "\\end{enumerate}",
        "env closer must stay structure"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}
