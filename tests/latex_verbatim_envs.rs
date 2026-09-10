//! snapper-3tj3 / GitHub #98: Overleaf verbatimEnvNames are Code, not prose.
//! GitHub #209: fancyvrb BVerbatim / LVerbatim are the same.

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

/// Ticket fixture (GitHub #209): fancyvrb BVerbatim and LVerbatim
/// bodies stay Code; following prose still splits.
#[test]
fn bverbatim_lverbatim_fixture_is_code_and_does_not_reflow() {
    for name in ["BVerbatim", "LVerbatim"] {
        let input = format!(
            concat!(
                "\\begin{{document}}\n",
                "Before the listing. More before.\n",
                "\\begin{{{name}}}\n",
                "This is a long sentence that must not reflow as prose inside {name}.\n",
                "\\end{{{name}}}\n",
                "After the listing. Second sentence.\n",
                "\\end{{document}}\n",
            ),
            name = name
        );
        let regions = LatexParser::default().parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Code { body, .. }
                    if body.contains("This is a long sentence that must not reflow as prose")
            )),
            "{name} body must be Code, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("This is a long sentence that must not reflow as prose")
            )),
            "{name} body must not be Prose, got: {regions:?}"
        );
        let out = format_text(&input, &latex_cfg()).unwrap();
        assert!(
            out.contains(&format!("\\begin{{{name}}}"))
                && out.contains(&format!("\\end{{{name}}}")),
            "{name} begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains(&format!(
                "This is a long sentence that must not reflow as prose inside {name}."
            )),
            "{name} body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("must not reflow as prose inside\n"),
            "{name} must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the listing.\nSecond sentence."),
            "prose after {name} must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

        let two = format!("\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\n");
        let two_out = format_text(&two, &latex_cfg()).unwrap();
        assert_eq!(two_out, two, "{name} env must be identity, got:\n{two_out}");
        assert!(
            !two_out.contains("First line.\nSecond line."),
            "{name} two-sentence body must not split, got:\n{two_out}"
        );
    }
}
