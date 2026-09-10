//! snapper-twmw / GitHub #181: leftover Overleaf IEEEeqnarray /
//! subeqnarray / xltabular / math* are Structure, not prose.

use snapper_fmt::format::Format;
use snapper_fmt::parser::latex::LatexParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{format_text, FormatConfig};

fn latex_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Latex,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture: `\begin{IEEEeqnarray}{rCl}` / long sentence / `\end{IEEEeqnarray}`.
#[test]
fn ieeeeqnarray_fixture_is_structure_and_surrounding_prose_reflows() {
    let input = concat!(
        "\\begin{IEEEeqnarray}{rCl}\n",
        "This is a long sentence that must stay inside IEEEeqnarray and must not reflow as prose.\n",
        "\\end{IEEEeqnarray}\n",
        "After the array. Second sentence.\n",
    );
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
        "surrounding After the array must still reflow, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

#[test]
fn leftover_overleaf_names_are_structure_and_do_not_reflow() {
    for name in [
        "IEEEeqnarray",
        "IEEEeqnarray*",
        "subeqnarray",
        "subeqnarray*",
        "xltabular",
        "math*",
    ] {
        let input = format!(
            "\\begin{{{name}}}\nThis is a long sentence that must not reflow as prose inside {name}. Another sentence stays put.\n\\end{{{name}}}\nAfter the array. Second sentence.\n"
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
        assert!(
            out.contains(&fused),
            "{name} body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains(&format!("inside {name}.\nAnother")),
            "{name} must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the array.\nSecond sentence."),
            "{name} surrounding prose must still reflow, got:\n{out}"
        );
    }
}
