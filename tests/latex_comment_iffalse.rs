//! snapper-4zen: comment env is Code/Structure; `\iffalse...\fi` is Structure.

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
fn comment_environment_body_is_not_prose() {
    let input = "\\begin{comment}\nThis is a long sentence that must not reflow as prose inside comment.\n\\end{comment}\n";
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| match r {
            Region::Code { body, .. } => {
                body.contains("must not reflow as prose inside comment")
            }
            Region::Structure(s) => s.contains("must not reflow as prose inside comment"),
            _ => false,
        }),
        "comment env body must be Code or Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("must not reflow as prose inside comment")
        )),
        "comment env body must not be Prose, got: {regions:?}"
    );
}

#[test]
fn iffalse_block_is_structure_not_prose() {
    let input =
        "\\iffalse\nThis is a long sentence that must not reflow as prose inside iffalse.\n\\fi\n";
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("must not reflow as prose inside iffalse")
        )),
        "iffalse body must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("must not reflow as prose inside iffalse")
        )),
        "iffalse body must not be Prose, got: {regions:?}"
    );
}

#[test]
fn same_line_iffalse_before_end_document_is_not_prose() {
    let input = "\\begin{document}\nKeep this. \\iffalse Hidden one. Hidden two. \\fi After. \\end{document}\n";
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(r"\iffalse") && s.contains("Hidden one")
        )),
        "iffalse before \\end{{document}} must be Structure, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("Hidden one"))),
        "iffalse payload must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("Hidden one. Hidden two."),
        "iffalse two sentences must not reflow, got:\n{out}"
    );
    assert!(
        !out.contains("Hidden one.\nHidden two."),
        "iffalse must stay one source line, got:\n{out}"
    );
}

#[test]
fn same_line_iffalse_before_begin_equation_is_not_prose() {
    let input = "\\begin{document}\n\\iffalse Hidden one. Hidden two. \\fi \\begin{equation}x=1\\end{equation}\nAfter. Next.\n\\end{document}\n";
    let regions = LatexParser::default().parse(input);
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("Hidden one"))),
        "iffalse before begin must not leak Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("Hidden one. Hidden two."),
        "iffalse before begin must not reflow, got:\n{out}"
    );
    assert!(
        !out.contains("Hidden one.\nHidden two."),
        "iffalse must stay one source line, got:\n{out}"
    );
}
