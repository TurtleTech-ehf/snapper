//! snapper-wjmq: tree-sitter / latexindent math and align-delim envs are Structure.

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

#[test]
fn alignat_body_is_structure_not_prose() {
    let input = "\\begin{alignat}{2}\nThis is a long sentence that must not reflow as prose inside alignat.\n\\end{alignat}\n";
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("must not reflow as prose inside alignat")
        )),
        "alignat body must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("must not reflow as prose inside alignat")
        )),
        "alignat body must not be Prose, got: {regions:?}"
    );
}

#[test]
fn tabularx_body_is_structure_not_prose() {
    let input = "\\begin{tabularx}{\\textwidth}{l}\nThis is a long sentence that must not reflow as prose inside tabularx.\n\\end{tabularx}\n";
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("must not reflow as prose inside tabularx")
        )),
        "tabularx body must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("must not reflow as prose inside tabularx")
        )),
        "tabularx body must not be Prose, got: {regions:?}"
    );
}

#[test]
fn alignat_and_tabularx_two_sentences_do_not_reflow() {
    let alignat = "\\begin{alignat}{2}\nThis is a long sentence that must not reflow as prose inside alignat. Another sentence stays put.\n\\end{alignat}\n";
    let tabularx = "\\begin{tabularx}{\\textwidth}{l}\nThis is a long sentence that must not reflow as prose inside tabularx. Another sentence stays put.\n\\end{tabularx}\n";
    for (input, fused) in [
        (
            alignat,
            "This is a long sentence that must not reflow as prose inside alignat. Another sentence stays put.",
        ),
        (
            tabularx,
            "This is a long sentence that must not reflow as prose inside tabularx. Another sentence stays put.",
        ),
    ] {
        let out = format_text(input, &latex_cfg()).unwrap();
        assert!(
            out.contains(fused),
            "env body must not split into prose sentences, got:\n{out}"
        );
        assert!(
            !out.contains("alignat.\nAnother") && !out.contains("tabularx.\nAnother"),
            "env body must not reflow as prose, got:\n{out}"
        );
        assert_eq!(out, input, "structure env must be identity, got:\n{out}");
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}
