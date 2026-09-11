//! snapper-3tj3 / GitHub #98: Overleaf verbatimEnvNames are Code, not prose.
//! GitHub #209: fancyvrb BVerbatim / LVerbatim are the same.
//! GitHub #213: fancyvrb SaveVerbatim / VerbatimOut are the same FV@Scan class.
//! GitHub #247: fvextra VerbatimWrite is the same FV@Scan class as VerbatimOut.
//! GitHub #230: alltt.sty is a standard verbatim-like env (raw line breaks).
//! GitHub #234: listings.sty lstlisting* is the same raw scan as lstlisting.
//! GitHub #235: spverbatim.sty env and \\spverb are the same class as verb.
//! GitHub #244: fancyvrb Verbatim* / BVerbatim* / LVerbatim* are the same FV@Scan class.
//! GitHub #245: minted.sty \\mintinline / \\mint take {lang} then a FancyVerb body.
//! GitHub #275: fancyvrb \\SaveVerb{name}|body| is the same delimiter body as \\Verb.
//! GitHub #246: tcolorbox listings tcblisting* is the starred twin of tcblisting.
//! GitHub #250: moreverb verbatimtab is the same raw class as boxedverbatim.
//! GitHub #249: pythontex.sty pyblock / pyverbatim / pyconsole are the same
//! VerbatimEnvironment class as pycode.
//! GitHub #274: pythontex.sty pygments is the same VerbatimEnvironment class.

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

/// Ticket fixture (GitHub #250): moreverb `verbatimtab` bodies stay
/// Code; following prose still splits.
#[test]
fn verbatimtab_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{verbatimtab}\n",
        "First line. Second line.\n",
        "\\end{verbatimtab}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "verbatimtab body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "verbatimtab body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{verbatimtab}") && out.contains("\\end{verbatimtab}"),
        "verbatimtab begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "verbatimtab body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "verbatimtab must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after verbatimtab must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #246): tcolorbox listings `tcblisting*`
/// body stays Code; `{listing only}` stays on begin; following prose
/// still splits.
#[test]
fn tcblisting_star_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{tcblisting*}{listing only}\n",
        "First line. Second line.\n",
        "\\end{tcblisting*}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    let code = regions.iter().find_map(|r| match r {
        Region::Code {
            header,
            body,
            footer,
            ..
        } => Some((header.as_str(), body.as_str(), footer.as_str())),
        _ => None,
    });
    let Some((header, body, footer)) = code else {
        panic!("tcblisting* must be Code, got: {regions:?}");
    };
    assert!(
        header.contains("\\begin{tcblisting*}{listing only}"),
        "required listing options must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "tcblisting* body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains("\\end{tcblisting*}"),
        "tcblisting* footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "tcblisting* body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{tcblisting*}{listing only}") && out.contains("\\end{tcblisting*}"),
        "tcblisting* begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "tcblisting* body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "tcblisting* must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after tcblisting* must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
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

/// Ticket fixture (GitHub #244): fancyvrb Verbatim* / BVerbatim* /
/// LVerbatim* bodies stay Code; following prose still splits.
#[test]
fn verbatim_star_envs_fixture_is_code_and_does_not_reflow() {
    for name in ["Verbatim*", "BVerbatim*", "LVerbatim*"] {
        let input = format!(
            concat!(
                "\\begin{{{name}}}\n",
                "First line. Second line.\n",
                "\\end{{{name}}}\n",
                "After the block. Next.\n",
            ),
            name = name
        );
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
        assert!(
            out.contains(&format!("\\begin{{{name}}}"))
                && out.contains(&format!("\\end{{{name}}}")),
            "{name} begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "{name} body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "{name} must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after {name} must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}

/// Ticket fixture (GitHub #213): fancyvrb SaveVerbatim and VerbatimOut
/// bodies stay Code; the required `{foo}` arg stays on begin; following
/// prose still splits.
#[test]
fn saveverbatim_verbatimout_fixture_is_code_and_does_not_reflow() {
    for name in ["SaveVerbatim", "VerbatimOut"] {
        let input = format!(
            concat!(
                "\\begin{{{name}}}{{foo}}\n",
                "First line. Second line.\n",
                "\\end{{{name}}}\n",
                "After the block. Next.\n",
            ),
            name = name
        );
        let regions = LatexParser::default().parse(&input);
        let code = regions.iter().find_map(|r| match r {
            Region::Code {
                header,
                body,
                footer,
                ..
            } => Some((header.as_str(), body.as_str(), footer.as_str())),
            _ => None,
        });
        let Some((header, body, footer)) = code else {
            panic!("{name} must be Code, got: {regions:?}");
        };
        assert!(
            header.contains(&format!("\\begin{{{name}}}{{foo}}")),
            "{name} required arg must stay on the begin header, got header={header:?}"
        );
        assert!(
            body.contains("First line. Second line."),
            "{name} body must be Code, got body={body:?}"
        );
        assert!(
            footer.contains(&format!("\\end{{{name}}}")),
            "{name} footer must stay, got footer={footer:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
            "{name} body must not be Prose, got: {regions:?}"
        );
        let out = format_text(&input, &latex_cfg()).unwrap();
        assert!(
            out.contains(&format!("\\begin{{{name}}}{{foo}}"))
                && out.contains(&format!("\\end{{{name}}}")),
            "{name} begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "{name} body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "{name} must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after {name} must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}

/// Ticket fixture (GitHub #247): fvextra VerbatimWrite bodies stay
/// Code; the required `{out.tex}` arg stays on begin; following prose
/// still splits.
#[test]
fn verbatimwrite_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{VerbatimWrite}{out.tex}\n",
        "First line. Second line.\n",
        "\\end{VerbatimWrite}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    let code = regions.iter().find_map(|r| match r {
        Region::Code {
            header,
            body,
            footer,
            ..
        } => Some((header.as_str(), body.as_str(), footer.as_str())),
        _ => None,
    });
    let Some((header, body, footer)) = code else {
        panic!("VerbatimWrite must be Code, got: {regions:?}");
    };
    assert!(
        header.contains(r"\begin{VerbatimWrite}{out.tex}"),
        "VerbatimWrite required arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "VerbatimWrite body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains(r"\end{VerbatimWrite}"),
        "VerbatimWrite footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "VerbatimWrite body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\begin{VerbatimWrite}{out.tex}") && out.contains(r"\end{VerbatimWrite}"),
        "VerbatimWrite begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "VerbatimWrite body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "VerbatimWrite must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after VerbatimWrite must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #230): alltt.sty bodies stay Code; following
/// prose still splits.
#[test]
fn alltt_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{document}\n",
        "Before. More.\n",
        "\\begin{alltt}\n",
        "First line. Second line.\n",
        "\\end{alltt}\n",
        "After the block. Next.\n",
        "\\end{document}\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "alltt body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "alltt body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{alltt}") && out.contains("\\end{alltt}"),
        "alltt begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "alltt body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "alltt must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after alltt must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let two = concat!(
        "\\begin{alltt}\n",
        "First line. Second line.\n",
        "\\end{alltt}\n",
        "After the block. Next.\n",
    );
    let two_out = format_text(two, &latex_cfg()).unwrap();
    assert!(
        two_out.contains("First line. Second line."),
        "alltt two-sentence body must stay one source line, got:\n{two_out}"
    );
    assert!(
        !two_out.contains("First line.\nSecond line."),
        "alltt two-sentence body must not split, got:\n{two_out}"
    );
    assert!(
        two_out.contains("After the block.\nNext."),
        "prose after alltt must still split, got:\n{two_out}"
    );
    assert_eq!(format_text(&two_out, &latex_cfg()).unwrap(), two_out);
}

/// Ticket fixture (GitHub #234): listings.sty lstlisting* bodies stay
/// Code; following prose still splits.
#[test]
fn lstlisting_star_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{lstlisting*}\n",
        "First line. Second line.\n",
        "\\end{lstlisting*}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "lstlisting* body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "lstlisting* body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{lstlisting*}") && out.contains("\\end{lstlisting*}"),
        "lstlisting* begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "lstlisting* body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "lstlisting* must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after lstlisting* must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #235): spverbatim.sty `spverbatim` body stays
/// Code; `\spverb|a.b%|` is one token; following `Next.` still splits.
#[test]
fn spverbatim_and_spverb_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{spverbatim}\n",
        "First line. Second line.\n",
        "\\end{spverbatim}\n",
        "See \\spverb|a.b%| please. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "spverbatim body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "spverbatim body must not be Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| {
            matches!(r, Region::Structure(s) if s.contains("%|") || s.trim() == "%|\n")
        }),
        "inner % of \\spverb must not be a comment, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{spverbatim}") && out.contains("\\end{spverbatim}"),
        "spverbatim begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "spverbatim body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "spverbatim must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains(r"\spverb|a.b%|"),
        "\\spverb|a.b%| must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("\\spverb|a.\n") && !out.contains("\\spverb|a.b%\n"),
        "inner .!?% must not split \\spverb, got:\n{out}"
    );
    assert!(
        out.contains("See \\spverb|a.b%| please.\nNext."),
        "prose after \\spverb must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #249): pythontex.sty `pyblock` / `pyverbatim`
/// / `pyconsole` bodies stay Code; following prose still splits.
#[test]
fn pythontex_pyblock_pyverbatim_pyconsole_fixture_is_code_and_does_not_reflow() {
    for name in ["pyblock", "pyverbatim", "pyconsole"] {
        let input = format!(
            concat!(
                "\\begin{{{name}}}\n",
                "First line. Second line.\n",
                "\\end{{{name}}}\n",
                "After the block. Next.\n",
            ),
            name = name
        );
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
        assert!(
            out.contains(&format!("\\begin{{{name}}}"))
                && out.contains(&format!("\\end{{{name}}}")),
            "{name} begin/end must stay, got:\n{out}"
        );
        assert!(
            out.contains("First line. Second line."),
            "{name} body must stay one source line, got:\n{out}"
        );
        assert!(
            !out.contains("First line.\nSecond line."),
            "{name} must not reflow as prose, got:\n{out}"
        );
        assert!(
            out.contains("After the block.\nNext."),
            "prose after {name} must still split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}

/// Ticket fixture (GitHub #274): pythontex.sty `pygments` required
/// `{lang}` stays on the begin header; body stays Code; following
/// prose still splits.
#[test]
fn pythontex_pygments_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{pygments}{python}\n",
        "First line. Second line.\n",
        "\\end{pygments}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    let code = regions.iter().find_map(|r| match r {
        Region::Code {
            header,
            body,
            footer,
            ..
        } => Some((header.as_str(), body.as_str(), footer.as_str())),
        _ => None,
    });
    let Some((header, body, footer)) = code else {
        panic!("pygments must be Code, got: {regions:?}");
    };
    assert!(
        header.contains(r"\begin{pygments}{python}"),
        "required lexer arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "pygments body must keep both sentences, got body={body:?}"
    );
    assert!(
        footer.contains(r"\end{pygments}"),
        "pygments footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("First line") || p.contains("{python}")
        )),
        "pygments lexer/body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\begin{pygments}{python}") && out.contains(r"\end{pygments}"),
        "pygments begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "pygments body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "pygments must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after pygments must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #245): `\mintinline{python}|a.b! c|` is one
/// token; following `Next sentence.` still splits. `{lang}{body}` and
/// `\mint` are the same class.
#[test]
fn mintinline_and_mint_fixture_is_atomic_and_does_not_reflow() {
    let input = "\\begin{document}\nUse \\mintinline{python}|a.b! c| here. Next sentence.\n\\end{document}\n";
    let regions = LatexParser::default().parse(input);
    assert!(
        !regions.iter().any(|r| matches!(r, Region::Prose(p) if p.contains("|a.b! c|") && !p.contains(r"\mintinline"))),
        "mintinline leftover |body| must not be prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\mintinline{python}|a.b! c|"),
        "\\mintinline{{python}}|a.b! c| must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("\\mintinline{python}|a.\n") && !out.contains("\\mintinline{python}|a.b!\n"),
        "inner .!? must not split mintinline, got:\n{out}"
    );
    assert!(
        out.contains("Use \\mintinline{python}|a.b! c| here.\nNext sentence."),
        "prose after mintinline must still split, got:\n{out}"
    );

    let brace = "\\begin{document}\nUse \\mintinline{python}{a.b! c} here. Next sentence.\n\\end{document}\n";
    let brace_out = format_text(brace, &latex_cfg()).unwrap();
    assert!(
        brace_out.contains(r"\mintinline{python}{a.b! c}"),
        "mintinline {{lang}}{{body}} must stay one token, got:\n{brace_out}"
    );
    assert!(
        brace_out.contains("Use \\mintinline{python}{a.b! c} here.\nNext sentence."),
        "prose after mintinline brace body must still split, got:\n{brace_out}"
    );

    let mint =
        "\\begin{document}\nUse \\mint{python}|a.b! c| here. Next sentence.\n\\end{document}\n";
    let mint_out = format_text(mint, &latex_cfg()).unwrap();
    assert!(
        mint_out.contains(r"\mint{python}|a.b! c|"),
        "\\mint{{python}}|a.b! c| must stay one token, got:\n{mint_out}"
    );
    assert!(
        mint_out.contains("Use \\mint{python}|a.b! c| here.\nNext sentence."),
        "prose after mint must still split, got:\n{mint_out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #275): `\SaveVerb{foo}|done. Next|` is one
/// token; following `After.` still splits.
#[test]
fn saveverb_fixture_is_atomic_and_does_not_reflow() {
    let input =
        "\\begin{document}\nUse \\SaveVerb{foo}|done. Next| here. After.\n\\end{document}\n";
    let regions = LatexParser::default().parse(input);
    assert!(
        !regions.iter().any(|r| matches!(r, Region::Prose(p) if p.contains("|done. Next|") && !p.contains(r"\SaveVerb"))),
        "SaveVerb leftover |body| must not be prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\SaveVerb{foo}|done. Next|"),
        "\\SaveVerb{{foo}}|done. Next| must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("\\SaveVerb{foo}|done.\n") && !out.contains("\\SaveVerb{foo}|done. Next|\n"),
        "inner . must not split SaveVerb, got:\n{out}"
    );
    assert!(
        out.contains("Use \\SaveVerb{foo}|done. Next| here.\nAfter."),
        "prose after SaveVerb must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}
