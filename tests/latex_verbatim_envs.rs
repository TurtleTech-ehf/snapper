//! snapper-3tj3 / GitHub #98: Overleaf verbatimEnvNames are Code, not prose.

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

/// Ticket fixture: `\begin{Verbatim}` / First line. Second line. / `\end{Verbatim}`.
#[test]
fn verbatim_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{Verbatim}\n",
        "First line. Second line.\n",
        "\\end{Verbatim}\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "Verbatim body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "Verbatim body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("First line. Second line."),
        "Verbatim body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "Verbatim must not reflow as prose, got:\n{out}"
    );
    assert_eq!(out, input, "Verbatim env must be identity, got:\n{out}");
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

#[test]
fn boxedverbatim_tcblisting_codeexample_do_not_reflow() {
    for name in ["boxedverbatim", "tcblisting", "codeexample"] {
        let input = format!("\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\n");
        let regions = LatexParser::default().parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. } if body.contains("First line. Second line.")
            )),
            "{name} body must be Code, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "{name} body must not be Prose, got: {regions:?}"
        );
        let out = format_text(&input, &latex_cfg()).unwrap();
        assert_eq!(out, input, "{name} env must be identity, got:\n{out}");
        assert!(
            !out.contains("First line.\nSecond line."),
            "{name} must not reflow as prose, got:\n{out}"
        );
    }
}
