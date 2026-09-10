//! snapper-t4lj / GitHub #95: LaTeX figure and table captions reflow.

use snapper_fmt::format::Format;
use snapper_fmt::parser::latex::LatexParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text, oracle};

fn latex_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Latex,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture:
/// `\begin{figure}` / `\centering` / `\caption{First claim. Second claim about the plot.}` / `\end{figure}`
#[test]
fn figure_caption_fixture_reflows_long_argument() {
    let input = concat!(
        "\\begin{figure}\n",
        "\\centering\n",
        "\\caption{First claim. Second claim about the plot.}\n",
        "\\end{figure}\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(r"\centering"))),
        "float chrome must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("First claim. Second claim about the plot.")
        )),
        "caption long argument must be Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains(r"\centering") || p.contains(r"\begin{figure}")
        )),
        "float chrome must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\caption{First claim.\nSecond claim about the plot.}"),
        "caption must sembr, got:\n{out}"
    );
    assert!(
        !out.contains("\\caption{First claim. Second claim about the plot.}"),
        "caption must not stay fused, got:\n{out}"
    );
    assert!(
        out.contains("\\begin{figure}\n\\centering\n"),
        "float chrome must stay put, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    assert!(
        oracle::matches(Format::Latex, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn table_caption_fixture_reflows_long_argument() {
    let input = concat!(
        "\\begin{table}\n",
        "\\centering\n",
        "\\caption{First claim. Second claim about the plot.}\n",
        "\\end{table}\n",
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\caption{First claim.\nSecond claim about the plot.}"),
        "table caption must sembr, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    assert!(
        oracle::matches(Format::Latex, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn nested_tabular_tikzpicture_stay_structure() {
    let input = concat!(
        "\\begin{figure}\n",
        "\\begin{tikzpicture}\n",
        "First sentence inside tikz. Second stays put.\n",
        "\\end{tikzpicture}\n",
        "\\begin{tabular}{ll}\n",
        "a & b. c & d.\n",
        "\\end{tabular}\n",
        "\\caption{First claim. Second claim about the plot.}\n",
        "\\end{figure}\n",
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("First sentence inside tikz. Second stays put."),
        "tikzpicture must not sembr, got:\n{out}"
    );
    assert!(
        out.contains("a & b. c & d."),
        "tabular must not sembr, got:\n{out}"
    );
    assert!(
        out.contains("\\caption{First claim.\nSecond claim about the plot.}"),
        "caption must still sembr, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}
