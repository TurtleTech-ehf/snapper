//! snapper-t4lj / GitHub #95: figure/table caption long args reflow.
//!
//! tree-sitter caption curly_group is text; Overleaf FigureEnvironment is
//! Content<Text>. Float chrome stays Structure; nested tabular/tikzpicture
//! stay non-prose.

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

/// Ticket fixture: `\begin{figure}` / `\centering` / `\caption{First claim.
/// Second claim about the plot.}` / `\end{figure}`.
fn figure_caption_fixture() -> &'static str {
    concat!(
        "\\begin{figure}\n",
        "\\centering\n",
        "\\caption{First claim. Second claim about the plot.}\n",
        "\\end{figure}\n",
    )
}

#[test]
fn figure_caption_fixture_reflows_long_arg_not_chrome() {
    let input = figure_caption_fixture();
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(r"\begin{figure}")
        )),
        "figure begin must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(r"\centering"))),
        "centering chrome must be Structure, got: {regions:?}"
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
        "caption long argument must sembr, got:\n{out}"
    );
    assert!(
        out.contains("\\begin{figure}\n\\centering\n"),
        "float chrome must stay identity, got:\n{out}"
    );
    assert!(
        !out.contains("First claim. Second claim about the plot."),
        "fused caption must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    assert!(
        oracle::matches(Format::Latex, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn table_and_starred_float_captions_reflow() {
    for name in ["figure*", "table", "table*"] {
        let input = format!(
            "\\begin{{{name}}}\n\\centering\n\\caption{{First claim. Second claim about the plot.}}\n\\end{{{name}}}\n"
        );
        let out = format_text(&input, &latex_cfg()).unwrap();
        assert!(
            out.contains("\\caption{First claim.\nSecond claim about the plot.}"),
            "{name} caption must sembr, got:\n{out}"
        );
        assert!(
            out.contains(&format!("\\begin{{{name}}}\n\\centering\n")),
            "{name} chrome must stay identity, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}

#[test]
fn nested_tabular_and_tikzpicture_stay_non_prose() {
    let input = concat!(
        "\\begin{figure}\n",
        "\\begin{tabular}{l}\n",
        "This is a long sentence that must not reflow as prose inside tabular. Another sentence stays put.\n",
        "\\end{tabular}\n",
        "\\begin{tikzpicture}\n",
        "This is a long sentence that must not reflow as prose inside tikzpicture. Another sentence stays put.\n",
        "\\end{tikzpicture}\n",
        "\\caption{First claim. Second claim about the plot.}\n",
        "\\end{figure}\n",
    );
    let regions = LatexParser::default().parse(input);
    for needle in [
        "must not reflow as prose inside tabular",
        "must not reflow as prose inside tikzpicture",
    ] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "{needle} must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains(needle))),
            "{needle} must not be Prose, got: {regions:?}"
        );
    }
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(
            "This is a long sentence that must not reflow as prose inside tabular. Another sentence stays put."
        ) && out.contains(
            "This is a long sentence that must not reflow as prose inside tikzpicture. Another sentence stays put."
        ),
        "nested tabular/tikzpicture must not reflow, got:\n{out}"
    );
    assert!(
        out.contains("\\caption{First claim.\nSecond claim about the plot.}"),
        "caption after nested envs must still sembr, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

#[test]
fn includegraphics_and_label_stay_structure() {
    let input = concat!(
        "\\begin{figure}\n",
        "\\includegraphics[width=0.8\\textwidth]{plot.pdf}\n",
        "\\caption{First claim. Second claim about the plot.}\n",
        "\\label{fig:plot.pdf}\n",
        "\\end{figure}\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(r"\includegraphics[width=0.8\textwidth]{plot.pdf}")
        )),
        "includegraphics must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(r"\label{fig:plot.pdf}"))),
        "label must be Structure, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\includegraphics[width=0.8\\textwidth]{plot.pdf}"),
        "includegraphics must not split on .pdf, got:\n{out}"
    );
    assert!(
        !out.contains("plot.\npdf"),
        "filename period must not sembr, got:\n{out}"
    );
    assert!(
        out.contains("\\caption{First claim.\nSecond claim about the plot.}"),
        "caption must still sembr, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}
