//! org-element latex-environment closer is `\\end{NAME}[ \t]*$` (EOL),
//! not leftover-start. Begin is `^[ \t]*\\begin{[A-Za-z0-9*]+}`
//! (case-fold). Trailing non-ws after `\end{name}` is not a closer.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    concat!(
        "\\begin{equation}\n",
        "x = 1. 2 \\end{equation}\n",
        "After. Next.\n",
    )
}

fn assert_after_next_prose_and_split(out: &str, regions: &[Region]) {
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("After.") && s.contains("Next.")
        )),
        "After. / Next. must stay Prose, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("After."))),
        "After. must not freeze as Structure, got {regions:?}"
    );
    assert!(
        out.contains("After.\nNext."),
        "After. / Next. must still split, got:\n{out}"
    );
    assert!(
        !out.contains("After. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(out, &org_cfg()).unwrap(), out);
}

#[test]
fn mid_line_eol_end_closes_equation_body() {
    let input = ticket_fixture();
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("\\begin{equation}")
                    || s.contains("x = 1. 2 \\end{equation}")
        )),
        "body through \\end{{equation}} must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("x = 1. 2") && s.contains("\\end{equation}")
        )),
        "equation body plus EOL closer stay Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("x = 1. 2"))),
        "closed equation body must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("\\begin{equation}\nx = 1. 2 \\end{equation}\nAfter.\nNext."),
        "closed env frozen; After. / Next. still split, got:\n{out}"
    );
    assert_after_next_prose_and_split(&out, &regions);
}

#[test]
fn case_fold_begin_equation_matches_end() {
    let input = concat!(
        "\\begin{Equation}\n",
        "x = 1. 2 \\end{equation}\n",
        "After. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("x = 1. 2") && s.contains("\\end{equation}")
        )),
        "\\begin{{Equation}} / \\end{{equation}} must close, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("x = 1. 2"))),
        "case-fold equation body must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_after_next_prose_and_split(&out, &regions);
}

#[test]
fn same_line_leftover_start_eol_closer_unchanged() {
    let input = "\\begin{equation} x = 1 \\end{equation}\nAfter. Next.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("\\begin{equation}") && s.contains("\\end{equation}")
        )),
        "same-line leftover-start env must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("\\begin{equation} x = 1 \\end{equation}\nAfter.\nNext."),
        "same-line EOL closer unchanged, got:\n{out}"
    );
    assert_after_next_prose_and_split(&out, &regions);
}

#[test]
fn end_with_trailing_non_ws_is_not_a_closer() {
    let input = concat!(
        "\\begin{equation}\n",
        "x = 1. 2 \\end{equation} still open\n",
        "After. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("\\begin{equation}")
                    && s.contains("x = 1. 2")
                    && s.contains("After.")
                    && s.contains("Next.")
        )),
        "\\end{{equation}} with trailing non-ws is not a closer, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("\\begin{equation}"))),
        "unclosed begin must not be Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_after_next_prose_and_split(&out, &regions);
}
