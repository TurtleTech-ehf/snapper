//! snapper-twmw / GitHub #181: leftover Overleaf tokens.mjs names are
//! Structure, not prose (`IEEEeqnarray` / `IEEEeqnarray*` / `subeqnarray`
//! / `subeqnarray*` / `xltabular` / `math*`).

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

/// Ticket fixture: `\begin{IEEEeqnarray}{rCl}` / long sentence /
/// `\end{IEEEeqnarray}` / surrounding two-sentence prose.
fn ieeeeqnarray_fixture() -> &'static str {
    concat!(
        "\\begin{IEEEeqnarray}{rCl}\n",
        "This is a long sentence that must stay inside IEEEeqnarray and must not reflow as prose.\n",
        "\\end{IEEEeqnarray}\n",
        "After the array. Second sentence.\n",
    )
}

#[test]
fn ieeeeqnarray_fixture_body_is_structure_surrounding_reflows() {
    let input = ieeeeqnarray_fixture();
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("must stay inside IEEEeqnarray and must not reflow as prose")
        )),
        "IEEEeqnarray body must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("must stay inside IEEEeqnarray and must not reflow as prose")
        )),
        "IEEEeqnarray body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(
            "This is a long sentence that must stay inside IEEEeqnarray and must not reflow as prose."
        ),
        "IEEEeqnarray body must stay one source line, got:\n{out}"
    );
    assert!(
        out.contains("After the array.\nSecond sentence."),
        "prose after IEEEeqnarray must still reflow, got:\n{out}"
    );
    assert!(
        !out.contains("After the array. Second sentence."),
        "fused surrounding prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

#[test]
fn leftover_overleaf_envs_are_structure_and_do_not_reflow() {
    for (name, opener) in [
        ("IEEEeqnarray", "\\begin{IEEEeqnarray}{rCl}"),
        ("IEEEeqnarray*", "\\begin{IEEEeqnarray*}{rCl}"),
        ("subeqnarray", "\\begin{subeqnarray}"),
        ("subeqnarray*", "\\begin{subeqnarray*}"),
        ("xltabular", "\\begin{xltabular}{\\textwidth}{l}"),
        ("math*", "\\begin{math*}"),
    ] {
        let input = format!(
            "{opener}\nThis is a long sentence that must stay inside {name} and must not reflow as prose. Another sentence stays put.\n\\end{{{name}}}\nAfter the array. Second sentence.\n"
        );
        let fused = format!(
            "This is a long sentence that must stay inside {name} and must not reflow as prose. Another sentence stays put."
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
        assert!(
            out.contains(&fused),
            "{name} body must not split into prose sentences, got:\n{out}"
        );
        assert!(
            !out.contains(&format!(
                "inside {name} and must not reflow as prose.\nAnother"
            )),
            "{name} must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the array.\nSecond sentence."),
            "prose after {name} must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}
