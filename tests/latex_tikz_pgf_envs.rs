//! snapper-e916 / GitHub #97: Overleaf tikzcd / pgfplots axis / pgfpicture
//! are Structure, not prose.

use snapper_fmt::format::Format;
use snapper_fmt::parser::latex::LatexParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn latex_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Latex,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture: `\begin{tikzcd}` / long sentence / `\end{tikzcd}`.
#[test]
fn tikzcd_fixture_is_structure_and_does_not_reflow() {
    let input = concat!(
        "\\begin{tikzcd}\n",
        "This is a long sentence that must not reflow as prose inside tikzcd.\n",
        "\\end{tikzcd}\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("must not reflow as prose inside tikzcd")
        )),
        "tikzcd body must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("must not reflow as prose inside tikzcd")
        )),
        "tikzcd body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("This is a long sentence that must not reflow as prose inside tikzcd."),
        "tikzcd body must stay one source line, got:\n{out}"
    );
    assert_eq!(out, input, "tikzcd env must be identity, got:\n{out}");
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

#[test]
fn axis_pgfpicture_and_stars_do_not_reflow() {
    for name in ["tikzcd*", "axis", "axis*", "pgfpicture", "pgfpicture*"] {
        let input = format!(
            "\\begin{{{name}}}\nThis is a long sentence that must not reflow as prose inside {name}. Another sentence stays put.\n\\end{{{name}}}\n"
        );
        let fused = format!(
            "This is a long sentence that must not reflow as prose inside {name}. Another sentence stays put."
        );
        let regions = LatexParser::default().parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(&fused))),
            "{name} body must be Structure, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("must not reflow"))),
            "{name} body must not be Prose, got: {regions:?}"
        );
        let out = format_text(&input, &latex_cfg()).unwrap();
        assert_eq!(out, input, "{name} env must be identity, got:\n{out}");
        assert!(
            !out.contains(&format!("inside {name}.\nAnother")),
            "{name} must not reflow as prose, got:\n{out}"
        );
    }
}
