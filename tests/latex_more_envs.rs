//! Math and verbatim environments whose bodies must not reflow as prose.
//! Surrounding prose still splits.

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

fn structure_env_does_not_reflow(name: &str, begin_suffix: &str) {
    let input = format!(
        "\\begin{{{name}}}{begin_suffix}\nThis is a long sentence that must stay inside {name} and must not reflow as prose. Another sentence stays put.\n\\end{{{name}}}\nAfter the env. Second sentence.\n"
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
        "{name} body must stay one source line, got:\n{out}"
    );
    assert!(
        out.contains("After the env.\nSecond sentence."),
        "prose after {name} must still reflow, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

fn code_env_does_not_reflow(name: &str, begin_suffix: &str) {
    let input =
        format!("\\begin{{{name}}}{begin_suffix}\nFirst. Second.\n\\end{{{name}}}\nAfter. Next.\n");
    let regions = LatexParser::default().parse(&input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First. Second.")
        )),
        "{name} body must be Code, got: {regions:?}"
    );
    let out = format_text(&input, &latex_cfg()).unwrap();
    assert!(
        out.contains("First. Second."),
        "{name} body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First.\nSecond."),
        "{name} must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after {name} must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

#[test]
fn more_math_envs_stay_structure() {
    structure_env_does_not_reflow("IEEEeqnarraybox", "{rCl}");
    structure_env_does_not_reflow("tikzpicture*", "");
    structure_env_does_not_reflow("loglogaxis", "");
    structure_env_does_not_reflow("semilogxaxis", "");
    structure_env_does_not_reflow("semilogyaxis", "");
    structure_env_does_not_reflow("groupplot", "");
    structure_env_does_not_reflow("smithchart", "");
    structure_env_does_not_reflow("smithchartaxis", "");
    structure_env_does_not_reflow("polaraxis", "");
    structure_env_does_not_reflow("ternaryaxis", "");
    structure_env_does_not_reflow("dgroup", "");
    structure_env_does_not_reflow("dgroup*", "");
    structure_env_does_not_reflow("darray", "");
    structure_env_does_not_reflow("darray*", "");
}

#[test]
fn more_verbatim_envs_stay_code() {
    code_env_does_not_reflow("semiverbatim", "");
    code_env_does_not_reflow("verbbox", "");
    code_env_does_not_reflow("myverbbox", "{\\mybox}");
    code_env_does_not_reflow("verbnobox", "");
    code_env_does_not_reflow("markdown", "");
    code_env_does_not_reflow("markdown*", "");
}
