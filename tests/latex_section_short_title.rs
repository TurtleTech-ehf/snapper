//! snapper-n8wz / GitHub #94: LaTeX section optional short title is Structure.

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
/// `\section[Short. Title.]{A long title. With two sentences.}`
/// `Body. More body.`
#[test]
fn section_optional_short_title_fixture_is_structure_and_reflows_body() {
    let input = concat!(
        "\\section[Short. Title.]{A long title. With two sentences.}\n",
        "Body. More body.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains(r"\section[Short. Title.]{A long title. With two sentences.}")
        )),
        "full section line including optional short title must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Short.") || p.contains("A long title")
        )),
        "short title and long title must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\section[Short. Title.]{A long title. With two sentences.}"),
        "optional short title line must stay one line, got:\n{out}"
    );
    assert!(
        !out.contains("Short.\n"),
        "must not split periods in optional short title:\n{out}"
    );
    assert!(
        out.contains("Body.\nMore body."),
        "body after section must still reflow, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    assert!(
        oracle::matches(Format::Latex, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn koma_addsec_addchap_addpart_optional_short_title_is_structure() {
    for cmd in [r"\addsec", r"\addchap", r"\addpart"] {
        let input = format!(
            "{cmd}[Short. Title.]{{A long title. With two sentences.}}\nBody. More body.\n"
        );
        let regions = LatexParser::default().parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s)
                    if s.contains(&format!(
                        "{cmd}[Short. Title.]{{A long title. With two sentences.}}"
                    ))
            )),
            "{cmd} line including optional short title must be Structure, got: {regions:?}"
        );
        let out = format_text(&input, &latex_cfg()).unwrap();
        assert!(
            out.contains(&format!(
                "{cmd}[Short. Title.]{{A long title. With two sentences.}}"
            )),
            "{cmd} optional short title must stay one line, got:\n{out}"
        );
        assert!(
            out.contains("Body.\nMore body."),
            "{cmd} body after section must still reflow, got:\n{out}"
        );
    }
}
