//! snapper-3tj3 / GitHub #98: Overleaf verbatimEnvNames are Code, not prose.
//! GitHub #209: fancyvrb BVerbatim / LVerbatim are the same.
//! GitHub #213: fancyvrb SaveVerbatim / VerbatimOut are the same FV@Scan class.
//! GitHub #247: fvextra VerbatimWrite is the same FV@Scan class as VerbatimOut.
//! GitHub #292: fvextra VerbatimBuffer is the same raw grab as VerbatimWrite.
//! GitHub #293: fvextra VerbEnv is the environment form of Verb.
//! GitHub #230: alltt.sty is a standard verbatim-like env (raw line breaks).
//! GitHub #234: listings.sty lstlisting* is the same raw scan as lstlisting.
//! GitHub #235: spverbatim.sty env and \\spverb are the same class as verb.
//! GitHub #244: fancyvrb Verbatim* / BVerbatim* / LVerbatim* are the same FV@Scan class.
//! GitHub #245: minted.sty \\mintinline / \\mint take {lang} then a FancyVerb body.
//! GitHub #394: minted.sty \\inputminted is one leftover command;
//! following flush prose stays on its own line.
//! GitHub #400: tcolorbox \\tcbinputlisting is one leftover command
//! (one keyval group); following flush prose stays on its own line.
//! GitHub #273: minted.sty minted* is the starred twin of minted (same raw body).
//! GitHub #275: fancyvrb \\SaveVerb{name}|body| is the same delimiter body as \\Verb.
//! GitHub #246: tcolorbox listings tcblisting* is the starred twin of tcblisting.
//! GitHub #280: tcolorbox tcbverbatimwrite / tcbwritetemp write the env
//! body raw to a file (same verbatim grab as VerbatimOut).
//! GitHub #334: leftover tcolorbox tcboutputlisting / tcbexternal /
//! dispExample / dispExample* / dispListing / dispListing* are the
//! same raw grab (tcblistingscore / tcbexternal / tcbdocumentation).
//! GitHub #250: moreverb verbatimtab is the same raw class as boxedverbatim.
//! GitHub #279: moreverb listing / listingcont / listing* / listingcont*
//! are verbatim@start raw bodies (starred twins do not expand tabs).
//! GitHub #306: moreverb verbatimwrite writes the env body raw via
//! verbatim@start (same class as VerbatimOut / tcbverbatimwrite).
//! GitHub #353: leftover sverb verbwrite / ignore / demo / demo*
//! are the same sv@readenv raw grab (write / discard / demo display).
//! GitHub #249: pythontex.sty pyblock / pyverbatim / pyconsole are the same
//! VerbatimEnvironment class as pycode.
//! GitHub #276: pythontex.sty pycode* / pyblock* / pyverbatim* / pyconsole*
//! are the starred twins (same VerbatimEnvironment class).
//! GitHub #274: pythontex.sty pygments is the same VerbatimEnvironment class.
//! GitHub #277: pythontex.sty sympycode / sympyblock / sympyverbatim /
//! sympyconsole and starred twins are the same VerbatimEnvironment class.
//! GitHub #278: pythontex.sty pylabcode / pylabblock / pylabverbatim /
//! pylabconsole and starred twins are the same VerbatimEnvironment class.
//! GitHub #333: leftover default-family pyconcode / pyconverbatim /
//! pysub / pyconsub / sympycon* / pylabcon* / pythontexcustomcode are
//! the same VerbatimEnvironment class as landed pycode / pyconsole.
//! GitHub #352: option-family usefamily leftovers (rubycode
//! representative; ruby / rb / julia / juliacon / jl / matlab / octave /
//! bash / sage / rust / rs / R / Rcon / perl / pl / perlsix / psix /
//! javascript / js) are the same VerbatimEnvironment class.
//! GitHub #294: filecontentsdef.sty filecontentsdef writes the env body
//! verbatim into a macro (same raw grab as filecontents).
//! GitHub #299: leftover filecontentsdef.dtx siblings filecontentsgdef /
//! filecontentsdefmacro / filecontentsgdefmacro / filecontentshere and
//! starred twins filecontentsdef* / filecontentsgdef* / filecontentshere*
//! are the same raw grab.
//! GitHub #298: sagetex.sty sageverbatim / sageexample / sagecommandline
//! use verbatim@start like tree-sitter sagesilent / sageblock.
//! GitHub #304: scontents.sty scontents stores verbatim into a sequence;
//! verbatimsc is the package verbatim display env.
//! GitHub #305: piton.sty Piton is a verbatim listing env; \\piton|...|
//! is verb-like; \\piton{...} stays one token via generic cmd-arg.
//! GitHub #307: pythonhighlight.sty python (lstnewenvironment python)
//! is the same listings raw scan as lstlisting.
//! GitHub #346: pyluatex.sty pythonq / pythonrepl are verbatim python /
//! REPL bodies (landed python stays Code).
//! GitHub #345: showexpl.sty LTXexample (lstnewenvironment LTXexample)
//! is the same listings raw scan as lstlisting.
//! GitHub #348: luamplib.dtx mplibcode is the same raw grab class as luacode.
//! GitHub #385: luacode.sty leftover luaexec is the same raw grab class as luacode.
//! GitHub #350: codehigh.sty codehigh / demohigh / codehigh* /
//! demohigh* (NewCodeHighEnv) are leftover listing envs.
//! GitHub #308: verbments.sty pyglist wraps fancyvrb VerbatimOut; the
//! body is a raw listing.
//! GitHub #342: texments.sty / pygmentex.sty pygmented is
//! VerbatimEnvironment plus VerbatimOut.
//! GitHub #391: listings.sty \\lstinputlisting is one atomic command;
//! following flush prose does not join the command line.

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

/// Ticket fixture (GitHub #279): moreverb `listing` required `{1}`
/// stays on begin; body stays Code; following prose still splits.
#[test]
fn listing_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{listing}{1}\n",
        "First line. Second line.\n",
        "\\end{listing}\n",
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
        panic!("listing must be Code, got: {regions:?}");
    };
    assert!(
        header.contains("\\begin{listing}{1}"),
        "required start-line arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "listing body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains("\\end{listing}"),
        "listing footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("First line") || p.contains("{1}")
        )),
        "listing start-line/body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{listing}{1}") && out.contains("\\end{listing}"),
        "listing begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "listing body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "listing must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after listing must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #279): moreverb `listingcont` / `listing*` /
/// `listingcont*` stay Code; `listing*` keeps `{1}` on begin; following
/// prose still splits.
#[test]
fn listing_twins_fixture_is_code_and_does_not_reflow() {
    let cases = [
        ("listingcont", ""),
        ("listing*", "{1}"),
        ("listingcont*", ""),
    ];
    for (name, arg) in cases {
        let begin = format!("\\begin{{{name}}}{arg}");
        let input =
            format!("{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n");
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
            header.contains(&begin),
            "{name} begin must stay on the header, got header={header:?}"
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
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("First line") || p.contains("{1}")
            )),
            "{name} body/arg must not be Prose, got: {regions:?}"
        );
        let out = format_text(&input, &latex_cfg()).unwrap();
        assert!(
            out.contains(&begin) && out.contains(&format!("\\end{{{name}}}")),
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

/// Ticket fixture (GitHub #306): moreverb `verbatimwrite` writes the
/// env body raw via `verbatim@start`. Required `{out.tex}` stays on
/// begin; body stays Code; following prose still splits.
#[test]
fn moreverb_verbatimwrite_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{verbatimwrite}{out.tex}\n",
        "First line. Second line.\n",
        "\\end{verbatimwrite}\n",
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
        panic!("verbatimwrite must be Code, got: {regions:?}");
    };
    assert!(
        header.contains("\\begin{verbatimwrite}{out.tex}"),
        "required file arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "verbatimwrite body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains("\\end{verbatimwrite}"),
        "verbatimwrite footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "verbatimwrite body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{verbatimwrite}{out.tex}") && out.contains("\\end{verbatimwrite}"),
        "verbatimwrite begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "verbatimwrite body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "verbatimwrite must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after verbatimwrite must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #353): leftover sverb.sty `verbwrite` /
/// `ignore` / `demo` / `demo*` (`sv@readenv` raw grab) stay Code.
/// Required `{tmp.tex}` stays on the verbwrite begin header; following
/// prose still splits. Landed moreverb `verbatimwrite` stays Code.
#[test]
fn sverb_leftover_write_and_demo_envs_are_code_and_do_not_reflow() {
    let cases = [
        ("verbwrite", "{tmp.tex}"),
        ("ignore", ""),
        ("demo", "{Title}"),
        ("demo*", "{Title}"),
    ];
    for (name, arg) in cases {
        let begin = format!("\\begin{{{name}}}{arg}");
        let input =
            format!("{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n");
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
            header.contains(&begin),
            "{name} begin must stay on the header, got header={header:?}"
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
            out.contains(&begin) && out.contains(&format!("\\end{{{name}}}")),
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

    let landed = concat!(
        "\\begin{verbatimwrite}{out.tex}\n",
        "First line. Second line.\n",
        "\\end{verbatimwrite}\n",
        "After the block. Next.\n",
    );
    let landed_out = format_text(landed, &latex_cfg()).unwrap();
    assert!(
        landed_out.contains(
            "\\begin{verbatimwrite}{out.tex}\nFirst line. Second line.\n\\end{verbatimwrite}"
        ),
        "landed verbatimwrite must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("After the block.\nNext."),
        "prose after landed verbatimwrite must still split, got:\n{landed_out}"
    );
    assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
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

/// Ticket fixture (GitHub #280): tcolorbox `tcbverbatimwrite` bodies
/// stay Code; the required `{out.tex}` arg stays on begin; following
/// prose still splits.
#[test]
fn tcbverbatimwrite_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{tcbverbatimwrite}{out.tex}\n",
        "First line. Second line.\n",
        "\\end{tcbverbatimwrite}\n",
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
        panic!("tcbverbatimwrite must be Code, got: {regions:?}");
    };
    assert!(
        header.contains("\\begin{tcbverbatimwrite}{out.tex}"),
        "required file arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "tcbverbatimwrite body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains("\\end{tcbverbatimwrite}"),
        "tcbverbatimwrite footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "tcbverbatimwrite body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{tcbverbatimwrite}{out.tex}")
            && out.contains("\\end{tcbverbatimwrite}"),
        "tcbverbatimwrite begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "tcbverbatimwrite body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "tcbverbatimwrite must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after tcbverbatimwrite must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #280): tcolorbox `tcbwritetemp` bodies stay
/// Code; following prose still splits.
#[test]
fn tcbwritetemp_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{tcbwritetemp}\n",
        "First line. Second line.\n",
        "\\end{tcbwritetemp}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "tcbwritetemp body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "tcbwritetemp body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{tcbwritetemp}") && out.contains("\\end{tcbwritetemp}"),
        "tcbwritetemp begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "tcbwritetemp body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "tcbwritetemp must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after tcbwritetemp must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #334): leftover tcolorbox write/listing
/// envs `tcboutputlisting` / `tcbexternal` / `dispExample` /
/// `dispExample*` / `dispListing` / `dispListing*` stay Code;
/// following prose still splits. Landed `tcbverbatimwrite` /
/// `tcbwritetemp` stay Code.
#[test]
fn tcolorbox_leftover_write_listing_envs_are_code_and_do_not_reflow() {
    let cases = [
        ("tcboutputlisting", ""),
        ("tcbexternal", "{name}"),
        ("dispExample", ""),
        ("dispExample*", "{sbs}"),
        ("dispListing", ""),
        ("dispListing*", "{listing only}"),
    ];
    for (name, arg) in cases {
        let begin = format!("\\begin{{{name}}}{arg}");
        let input =
            format!("{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n");
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
            header.contains(&begin),
            "{name} begin must stay on the header, got header={header:?}"
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
            out.contains(&begin) && out.contains(&format!("\\end{{{name}}}")),
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

    let landed = concat!(
        "\\begin{tcbverbatimwrite}{out.tex}\n",
        "First line. Second line.\n",
        "\\end{tcbverbatimwrite}\n",
        "\\begin{tcbwritetemp}\n",
        "First line. Second line.\n",
        "\\end{tcbwritetemp}\n",
        "After the block. Next.\n",
    );
    let landed_out = format_text(landed, &latex_cfg()).unwrap();
    assert!(
        landed_out.contains(
            "\\begin{tcbverbatimwrite}{out.tex}\nFirst line. Second line.\n\\end{tcbverbatimwrite}"
        ),
        "landed tcbverbatimwrite must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("\\begin{tcbwritetemp}\nFirst line. Second line.\n\\end{tcbwritetemp}"),
        "landed tcbwritetemp must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("After the block.\nNext."),
        "prose after landed tcolorbox write envs must still split, got:\n{landed_out}"
    );
    assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
}

/// Ticket fixture (GitHub #294): filecontentsdef.sty `filecontentsdef`
/// bodies stay Code; the required `{\body}` arg stays on begin;
/// following prose still splits. Landed filecontents / filecontents*
/// stay Code.
#[test]
fn filecontentsdef_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{filecontentsdef}{\\body}\n",
        "First line. Second line.\n",
        "\\end{filecontentsdef}\n",
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
        panic!("filecontentsdef must be Code, got: {regions:?}");
    };
    assert!(
        header.contains(r"\begin{filecontentsdef}{\body}"),
        "required macro arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "filecontentsdef body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains(r"\end{filecontentsdef}"),
        "filecontentsdef footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "filecontentsdef body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\begin{filecontentsdef}{\body}") && out.contains(r"\end{filecontentsdef}"),
        "filecontentsdef begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "filecontentsdef body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "filecontentsdef must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after filecontentsdef must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let landed = concat!(
        "\\begin{filecontents}{x.tex}\n",
        "First line. Second line.\n",
        "\\end{filecontents}\n",
        "\\begin{filecontents*}\n",
        "First line. Second line.\n",
        "\\end{filecontents*}\n",
        "After the block. Next.\n",
    );
    let landed_out = format_text(landed, &latex_cfg()).unwrap();
    assert!(
        landed_out.contains(
            "\\begin{filecontents}{x.tex}\nFirst line. Second line.\n\\end{filecontents}"
        ),
        "landed filecontents must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out
            .contains("\\begin{filecontents*}\nFirst line. Second line.\n\\end{filecontents*}"),
        "landed filecontents* must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("After the block.\nNext."),
        "prose after landed filecontents* must still split, got:\n{landed_out}"
    );
    assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
}

/// Ticket fixture (GitHub #299): leftover filecontentsdef.dtx siblings
/// stay Code; required `{\body}` stays on begin; following prose still
/// splits. Landed filecontents / filecontents* / filecontentsdef stay
/// Code.
#[test]
fn filecontentsdef_sibling_envs_are_code_and_do_not_reflow() {
    let names = [
        "filecontentsgdef",
        "filecontentsdefmacro",
        "filecontentsgdefmacro",
        "filecontentshere",
        "filecontentsdef*",
        "filecontentsgdef*",
        "filecontentshere*",
    ];
    for name in names {
        let begin = format!(r"\begin{{{name}}}{{\body}}");
        let input =
            format!("{begin}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n");
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
            header.contains(&begin),
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
            out.contains(&begin) && out.contains(&format!("\\end{{{name}}}")),
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

    let landed = concat!(
        "\\begin{filecontents}{x.tex}\n",
        "First line. Second line.\n",
        "\\end{filecontents}\n",
        "\\begin{filecontents*}\n",
        "First line. Second line.\n",
        "\\end{filecontents*}\n",
        "\\begin{filecontentsdef}{\\body}\n",
        "First line. Second line.\n",
        "\\end{filecontentsdef}\n",
        "After the block. Next.\n",
    );
    let landed_out = format_text(landed, &latex_cfg()).unwrap();
    assert!(
        landed_out.contains(
            "\\begin{filecontents}{x.tex}\nFirst line. Second line.\n\\end{filecontents}"
        ),
        "landed filecontents must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out
            .contains("\\begin{filecontents*}\nFirst line. Second line.\n\\end{filecontents*}"),
        "landed filecontents* must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains(
            "\\begin{filecontentsdef}{\\body}\nFirst line. Second line.\n\\end{filecontentsdef}"
        ),
        "landed filecontentsdef must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("After the block.\nNext."),
        "prose after landed filecontentsdef must still split, got:\n{landed_out}"
    );
    assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
}

/// Ticket fixture (GitHub #298): sagetex.sty `sageverbatim` /
/// `sageexample` / `sagecommandline` bodies stay Code; following
/// prose still splits. Landed `sagesilent` / `sageblock` stay Code.
#[test]
fn sagetex_sageverbatim_family_fixture_is_code_and_does_not_reflow() {
    for name in ["sageverbatim", "sageexample", "sagecommandline"] {
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
            header.contains(&format!("\\begin{{{name}}}")),
            "{name} begin must stay on the header, got header={header:?}"
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

    for name in ["sagesilent", "sageblock"] {
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
            "landed {name} must stay Code, got: {regions:?}"
        );
        let out = format_text(&input, &latex_cfg()).unwrap();
        assert!(
            out.contains("First line. Second line.") && out.contains("After the block.\nNext."),
            "landed {name} must stay verbatim with following prose split, got:\n{out}"
        );
        assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
    }
}

/// Ticket fixture (GitHub #304): scontents.sty `scontents` stores
/// verbatim into a sequence; `verbatimsc` is the package verbatim
/// display env. Body stays Code; following prose still splits.
/// Landed filecontentsdef family stays Code.
#[test]
fn scontents_verbatimsc_fixture_is_code_and_does_not_reflow() {
    for name in ["scontents", "verbatimsc"] {
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
            header.contains(&format!("\\begin{{{name}}}")),
            "{name} begin must stay on the header, got header={header:?}"
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

    let landed = concat!(
        "\\begin{filecontents}{x.tex}\n",
        "First line. Second line.\n",
        "\\end{filecontents}\n",
        "\\begin{filecontents*}\n",
        "First line. Second line.\n",
        "\\end{filecontents*}\n",
        "\\begin{filecontentsdef}{\\body}\n",
        "First line. Second line.\n",
        "\\end{filecontentsdef}\n",
        "After the block. Next.\n",
    );
    let landed_out = format_text(landed, &latex_cfg()).unwrap();
    assert!(
        landed_out.contains(
            "\\begin{filecontents}{x.tex}\nFirst line. Second line.\n\\end{filecontents}"
        ),
        "landed filecontents must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out
            .contains("\\begin{filecontents*}\nFirst line. Second line.\n\\end{filecontents*}"),
        "landed filecontents* must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains(
            "\\begin{filecontentsdef}{\\body}\nFirst line. Second line.\n\\end{filecontentsdef}"
        ),
        "landed filecontentsdef must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("After the block.\nNext."),
        "prose after landed filecontentsdef must still split, got:\n{landed_out}"
    );
    assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
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

/// Ticket fixture (GitHub #308): verbments.sty `pyglist` wraps fancyvrb
/// VerbatimOut; the body is a raw listing. Optional `[language=python]`
/// stays on begin; body stays Code and one source line; following
/// prose still splits. Landed VerbatimOut / minted stay Code.
#[test]
fn verbments_pyglist_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{pyglist}[language=python]\n",
        "First line. Second line.\n",
        "\\end{pyglist}\n",
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
        panic!("pyglist must be Code, got: {regions:?}");
    };
    assert!(
        header.contains(r"\begin{pyglist}[language=python]"),
        "optional language arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "pyglist body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains(r"\end{pyglist}"),
        "pyglist footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "pyglist body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\begin{pyglist}[language=python]") && out.contains(r"\end{pyglist}"),
        "pyglist begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "pyglist body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "pyglist must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after pyglist must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let verbatimout = concat!(
        "\\begin{VerbatimOut}{foo}\n",
        "First line. Second line.\n",
        "\\end{VerbatimOut}\n",
        "After the block. Next.\n",
    );
    let verbatimout_out = format_text(verbatimout, &latex_cfg()).unwrap();
    assert!(
        verbatimout_out
            .contains("\\begin{VerbatimOut}{foo}\nFirst line. Second line.\n\\end{VerbatimOut}"),
        "landed VerbatimOut must stay a code env, got:\n{verbatimout_out}"
    );
    assert!(
        verbatimout_out.contains("After the block.\nNext."),
        "prose after landed VerbatimOut must still split, got:\n{verbatimout_out}"
    );
    assert_eq!(
        format_text(&verbatimout_out, &latex_cfg()).unwrap(),
        verbatimout_out
    );

    let minted = concat!(
        "\\begin{minted}{python}\n",
        "First line. Second line.\n",
        "\\end{minted}\n",
        "After the block. Next.\n",
    );
    let minted_out = format_text(minted, &latex_cfg()).unwrap();
    assert!(
        minted_out.contains("\\begin{minted}{python}\nFirst line. Second line.\n\\end{minted}"),
        "landed minted must stay a code env, got:\n{minted_out}"
    );
    assert!(
        minted_out.contains("After the block.\nNext."),
        "prose after landed minted must still split, got:\n{minted_out}"
    );
    assert_eq!(format_text(&minted_out, &latex_cfg()).unwrap(), minted_out);
}

/// Ticket fixture (GitHub #342): texments.sty / pygmentex.sty
/// `pygmented` is VerbatimEnvironment plus VerbatimOut. Required
/// `{lang}` stays on begin; body stays Code and one source line;
/// following prose still splits. Landed pygments / pyglist stay Code.
#[test]
fn texments_pygmented_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{pygmented}{python}\n",
        "First line. Second line.\n",
        "\\end{pygmented}\n",
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
        panic!("pygmented must be Code, got: {regions:?}");
    };
    assert!(
        header.contains(r"\begin{pygmented}{python}"),
        "required lexer arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "pygmented body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains(r"\end{pygmented}"),
        "pygmented footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("First line") || p.contains("{python}")
        )),
        "pygmented lexer/body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\begin{pygmented}{python}") && out.contains(r"\end{pygmented}"),
        "pygmented begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "pygmented body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "pygmented must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after pygmented must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let pygments = concat!(
        "\\begin{pygments}{python}\n",
        "First line. Second line.\n",
        "\\end{pygments}\n",
        "After the block. Next.\n",
    );
    let pygments_out = format_text(pygments, &latex_cfg()).unwrap();
    assert!(
        pygments_out
            .contains("\\begin{pygments}{python}\nFirst line. Second line.\n\\end{pygments}"),
        "landed pygments must stay a code env, got:\n{pygments_out}"
    );
    assert!(
        pygments_out.contains("After the block.\nNext."),
        "prose after landed pygments must still split, got:\n{pygments_out}"
    );
    assert_eq!(
        format_text(&pygments_out, &latex_cfg()).unwrap(),
        pygments_out
    );

    let pyglist = concat!(
        "\\begin{pyglist}[language=python]\n",
        "First line. Second line.\n",
        "\\end{pyglist}\n",
        "After the block. Next.\n",
    );
    let pyglist_out = format_text(pyglist, &latex_cfg()).unwrap();
    assert!(
        pyglist_out.contains(
            "\\begin{pyglist}[language=python]\nFirst line. Second line.\n\\end{pyglist}"
        ),
        "landed pyglist must stay a code env, got:\n{pyglist_out}"
    );
    assert!(
        pyglist_out.contains("After the block.\nNext."),
        "prose after landed pyglist must still split, got:\n{pyglist_out}"
    );
    assert_eq!(
        format_text(&pyglist_out, &latex_cfg()).unwrap(),
        pyglist_out
    );
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

/// Ticket fixture (GitHub #292): fvextra VerbatimBuffer bodies stay
/// Code; following prose still splits.
#[test]
fn verbatimbuffer_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{VerbatimBuffer}\n",
        "First line. Second line.\n",
        "\\end{VerbatimBuffer}\n",
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
        panic!("VerbatimBuffer must be Code, got: {regions:?}");
    };
    assert!(
        header.contains(r"\begin{VerbatimBuffer}"),
        "VerbatimBuffer begin must stay on the header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "VerbatimBuffer body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains(r"\end{VerbatimBuffer}"),
        "VerbatimBuffer footer must stay, got footer={footer:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "VerbatimBuffer body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\begin{VerbatimBuffer}") && out.contains(r"\end{VerbatimBuffer}"),
        "VerbatimBuffer begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "VerbatimBuffer body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "VerbatimBuffer must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after VerbatimBuffer must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

/// Ticket fixture (GitHub #293): fvextra VerbEnv bodies stay Code;
/// following prose still splits.
#[test]
fn verbenv_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{VerbEnv}\n",
        "First line. Second line.\n",
        "\\end{VerbEnv}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "VerbEnv body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "VerbEnv body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains(r"\begin{VerbEnv}") && out.contains(r"\end{VerbEnv}"),
        "VerbEnv begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "VerbEnv body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "VerbEnv must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after VerbEnv must still split, got:\n{out}"
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

/// Ticket fixture (GitHub #307): pythonhighlight.sty `python`
/// (`lstnewenvironment{python}`) is the same listings raw scan as
/// `lstlisting`. Body stays Code and one source line; following prose
/// still splits. `lstlisting` / `pycode` unchanged.
#[test]
fn pythonhighlight_python_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{python}\n",
        "First line. Second line.\n",
        "\\end{python}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "python body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "python body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{python}") && out.contains("\\end{python}"),
        "python begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "python body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "python must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after python must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let lstlisting = concat!(
        "\\begin{lstlisting}\n",
        "First line. Second line.\n",
        "\\end{lstlisting}\n",
        "After the block. Next.\n",
    );
    let lstlisting_out = format_text(lstlisting, &latex_cfg()).unwrap();
    assert!(
        lstlisting_out.contains("\\begin{lstlisting}\nFirst line. Second line.\n\\end{lstlisting}"),
        "lstlisting must stay a code env, got:\n{lstlisting_out}"
    );
    assert!(
        lstlisting_out.contains("After the block.\nNext."),
        "prose after lstlisting must still split, got:\n{lstlisting_out}"
    );
    assert_eq!(
        format_text(&lstlisting_out, &latex_cfg()).unwrap(),
        lstlisting_out
    );

    let pycode = concat!(
        "\\begin{pycode}\n",
        "First line. Second line.\n",
        "\\end{pycode}\n",
        "After the block. Next.\n",
    );
    let pycode_out = format_text(pycode, &latex_cfg()).unwrap();
    assert!(
        pycode_out.contains("\\begin{pycode}\nFirst line. Second line.\n\\end{pycode}"),
        "pycode must stay a code env, got:\n{pycode_out}"
    );
    assert!(
        pycode_out.contains("After the block.\nNext."),
        "prose after pycode must still split, got:\n{pycode_out}"
    );
    assert_eq!(format_text(&pycode_out, &latex_cfg()).unwrap(), pycode_out);
}

/// Ticket fixture (GitHub #345): showexpl.sty `LTXexample`
/// (`lstnewenvironment{LTXexample}`) is the same listings raw scan as
/// `lstlisting`. Body stays Code and one source line; following prose
/// still splits. `lstlisting` unchanged.
#[test]
fn showexpl_ltxexample_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{LTXexample}\n",
        "First line. Second line.\n",
        "\\end{LTXexample}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "LTXexample body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "LTXexample body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{LTXexample}") && out.contains("\\end{LTXexample}"),
        "LTXexample begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "LTXexample body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "LTXexample must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after LTXexample must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let lstlisting = concat!(
        "\\begin{lstlisting}\n",
        "First line. Second line.\n",
        "\\end{lstlisting}\n",
        "After the block. Next.\n",
    );
    let lstlisting_out = format_text(lstlisting, &latex_cfg()).unwrap();
    assert!(
        lstlisting_out.contains("\\begin{lstlisting}\nFirst line. Second line.\n\\end{lstlisting}"),
        "lstlisting must stay a code env, got:\n{lstlisting_out}"
    );
    assert!(
        lstlisting_out.contains("After the block.\nNext."),
        "prose after lstlisting must still split, got:\n{lstlisting_out}"
    );
    assert_eq!(
        format_text(&lstlisting_out, &latex_cfg()).unwrap(),
        lstlisting_out
    );
}

/// Ticket fixture (GitHub #346): pyluatex.sty `pythonq` / `pythonrepl`
/// are verbatim python / REPL bodies. Body stays Code and one source
/// line; following prose still splits. Landed `python` stays Code.
#[test]
fn pyluatex_pythonq_and_pythonrepl_fixture_is_code_and_does_not_reflow() {
    for name in ["pythonq", "pythonrepl"] {
        let input = format!(
            "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
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

    let python = concat!(
        "\\begin{python}\n",
        "First line. Second line.\n",
        "\\end{python}\n",
        "After the block. Next.\n",
    );
    let python_out = format_text(python, &latex_cfg()).unwrap();
    assert!(
        python_out.contains("\\begin{python}\nFirst line. Second line.\n\\end{python}"),
        "landed python must stay a code env, got:\n{python_out}"
    );
    assert!(
        python_out.contains("After the block.\nNext."),
        "prose after python must still split, got:\n{python_out}"
    );
    assert_eq!(format_text(&python_out, &latex_cfg()).unwrap(), python_out);
}

/// Ticket fixture (GitHub #348): luamplib.dtx `mplibcode` is the same
/// raw grab class as `luacode`. Body stays Code and one source line;
/// following prose still splits. `luacode` / `luacode*` unchanged.
#[test]
fn luamplib_mplibcode_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{mplibcode}\n",
        "First line. Second line.\n",
        "\\end{mplibcode}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "mplibcode body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "mplibcode body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{mplibcode}") && out.contains("\\end{mplibcode}"),
        "mplibcode begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "mplibcode body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "mplibcode must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after mplibcode must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    for name in ["luacode", "luacode*"] {
        let landed = format!(
            "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
        );
        let landed_out = format_text(&landed, &latex_cfg()).unwrap();
        assert!(
            landed_out.contains(&format!(
                "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}"
            )),
            "{name} must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("After the block.\nNext."),
            "prose after {name} must still split, got:\n{landed_out}"
        );
        assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
    }
}

/// Ticket fixture (GitHub #385): luacode.sty leftover `luaexec` is the
/// same raw grab class as `luacode`. Body stays Code and one source
/// line; following prose still splits. `luacode` / `luacode*` unchanged.
/// Distinct from luamplib `mplibcode`.
#[test]
fn luacode_luaexec_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{luaexec}\n",
        "First line. Second line.\n",
        "\\end{luaexec}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "luaexec body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "luaexec body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{luaexec}") && out.contains("\\end{luaexec}"),
        "luaexec begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "luaexec body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "luaexec must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after luaexec must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    for name in ["luacode", "luacode*"] {
        let landed = format!(
            "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
        );
        let landed_out = format_text(&landed, &latex_cfg()).unwrap();
        assert!(
            landed_out.contains(&format!(
                "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}"
            )),
            "{name} must stay a code env, got:\n{landed_out}"
        );
        assert!(
            landed_out.contains("After the block.\nNext."),
            "prose after {name} must still split, got:\n{landed_out}"
        );
        assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
    }
}

/// Ticket fixture (GitHub #350): codehigh.sty `codehigh` / `demohigh`
/// and starred twins (`NewCodeHighEnv`) are leftover listing envs.
/// Body stays Code and one source line; following prose still splits.
/// `lstlisting` unchanged.
#[test]
fn codehigh_leftover_envs_fixture_is_code_and_does_not_reflow() {
    for name in ["codehigh", "demohigh", "codehigh*", "demohigh*"] {
        let input = format!(
            "\\begin{{{name}}}\nFirst line. Second line.\n\\end{{{name}}}\nAfter the block. Next.\n"
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

    let lstlisting = concat!(
        "\\begin{lstlisting}\n",
        "First line. Second line.\n",
        "\\end{lstlisting}\n",
        "After the block. Next.\n",
    );
    let lstlisting_out = format_text(lstlisting, &latex_cfg()).unwrap();
    assert!(
        lstlisting_out.contains("\\begin{lstlisting}\nFirst line. Second line.\n\\end{lstlisting}"),
        "lstlisting must stay a code env, got:\n{lstlisting_out}"
    );
    assert!(
        lstlisting_out.contains("After the block.\nNext."),
        "prose after lstlisting must still split, got:\n{lstlisting_out}"
    );
    assert_eq!(
        format_text(&lstlisting_out, &latex_cfg()).unwrap(),
        lstlisting_out
    );
}

/// Ticket fixture (GitHub #273): minted.sty `minted*` bodies stay
/// Code; following prose still splits. Unstarred `minted` is unchanged.
#[test]
fn minted_star_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{minted*}\n",
        "First line. Second line.\n",
        "\\end{minted*}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "minted* body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "minted* body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{minted*}") && out.contains("\\end{minted*}"),
        "minted* begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "minted* body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "minted* must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after minted* must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let unstarred = concat!(
        "\\begin{minted}{python}\n",
        "print(1)\n",
        "print(2)\n",
        "\\end{minted}\n",
        "After the block. Next.\n",
    );
    let unstarred_out = format_text(unstarred, &latex_cfg()).unwrap();
    assert!(
        unstarred_out.contains("\\begin{minted}{python}\nprint(1)\nprint(2)\n\\end{minted}"),
        "unstarred minted must stay a code env, got:\n{unstarred_out}"
    );
    assert!(
        unstarred_out.contains("After the block.\nNext."),
        "prose after unstarred minted must still split, got:\n{unstarred_out}"
    );
    assert_eq!(
        format_text(&unstarred_out, &latex_cfg()).unwrap(),
        unstarred_out
    );
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

/// Ticket fixture (GitHub #276): pythontex.sty `pycode*` / `pyblock*`
/// / `pyverbatim*` / `pyconsole*` bodies stay Code; following prose
/// still splits.
#[test]
fn pythontex_starred_py_envs_fixture_is_code_and_does_not_reflow() {
    for name in ["pycode*", "pyblock*", "pyverbatim*", "pyconsole*"] {
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

/// Ticket fixture (GitHub #277): pythontex.sty `sympycode` / `sympyblock`
/// / `sympyverbatim` / `sympyconsole` and starred twins stay Code;
/// following prose still splits.
#[test]
fn pythontex_sympy_family_fixture_is_code_and_does_not_reflow() {
    for name in [
        "sympycode",
        "sympycode*",
        "sympyblock",
        "sympyblock*",
        "sympyverbatim",
        "sympyverbatim*",
        "sympyconsole",
        "sympyconsole*",
    ] {
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

/// Ticket fixture (GitHub #278): pythontex.sty `pylabcode` /
/// `pylabblock` / `pylabverbatim` / `pylabconsole` and starred twins
/// stay Code; following prose still splits.
#[test]
fn pythontex_pylab_family_fixture_is_code_and_does_not_reflow() {
    for name in [
        "pylabcode",
        "pylabcode*",
        "pylabblock",
        "pylabblock*",
        "pylabverbatim",
        "pylabverbatim*",
        "pylabconsole",
        "pylabconsole*",
    ] {
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

    let pycode = concat!(
        "\\begin{pycode}\n",
        "First line. Second line.\n",
        "\\end{pycode}\n",
        "After the block. Next.\n",
    );
    let pycode_out = format_text(pycode, &latex_cfg()).unwrap();
    assert!(
        pycode_out.contains("\\begin{pycode}\nFirst line. Second line.\n\\end{pycode}"),
        "unstarred pycode family must stay a code env, got:\n{pycode_out}"
    );
    assert!(
        pycode_out.contains("After the block.\nNext."),
        "prose after unstarred pycode must still split, got:\n{pycode_out}"
    );
    assert_eq!(format_text(&pycode_out, &latex_cfg()).unwrap(), pycode_out);
}

/// Ticket fixture (GitHub #333): leftover pythontex.sty default-family
/// envs stay Code; following prose still splits. Landed `pycode` /
/// `pyconsole` stay Code. `pythontexcustomcode` required `{py}` stays
/// on the begin header.
#[test]
fn pythontex_leftover_default_family_envs_fixture_is_code_and_does_not_reflow() {
    for name in [
        "pyconcode",
        "pyconverbatim",
        "pysub",
        "pyconsub",
        "sympyconcode",
        "sympyconverbatim",
        "sympysub",
        "sympyconsub",
        "pylabconcode",
        "pylabconverbatim",
        "pylabsub",
        "pylabconsub",
        "pythontexcustomcode",
    ] {
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

    let custom = concat!(
        "\\begin{pythontexcustomcode}{py}\n",
        "First line. Second line.\n",
        "\\end{pythontexcustomcode}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(custom);
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
        panic!("pythontexcustomcode must be Code, got: {regions:?}");
    };
    assert!(
        header.contains(r"\begin{pythontexcustomcode}{py}"),
        "required family arg must stay on the begin header, got header={header:?}"
    );
    assert!(
        body.contains("First line. Second line."),
        "pythontexcustomcode body must be Code, got body={body:?}"
    );
    assert!(
        footer.contains(r"\end{pythontexcustomcode}"),
        "pythontexcustomcode footer must stay, got footer={footer:?}"
    );
    let custom_out = format_text(custom, &latex_cfg()).unwrap();
    assert!(
        custom_out.contains(
            "\\begin{pythontexcustomcode}{py}\nFirst line. Second line.\n\\end{pythontexcustomcode}"
        ),
        "pythontexcustomcode required arg must stay on begin, got:\n{custom_out}"
    );
    assert!(
        custom_out.contains("After the block.\nNext."),
        "prose after pythontexcustomcode must still split, got:\n{custom_out}"
    );
    assert_eq!(format_text(&custom_out, &latex_cfg()).unwrap(), custom_out);

    let landed = concat!(
        "\\begin{pycode}\n",
        "First line. Second line.\n",
        "\\end{pycode}\n",
        "\\begin{pyconsole}\n",
        "First line. Second line.\n",
        "\\end{pyconsole}\n",
        "After the block. Next.\n",
    );
    let landed_out = format_text(landed, &latex_cfg()).unwrap();
    assert!(
        landed_out.contains("\\begin{pycode}\nFirst line. Second line.\n\\end{pycode}"),
        "landed pycode must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("\\begin{pyconsole}\nFirst line. Second line.\n\\end{pyconsole}"),
        "landed pyconsole must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("After the block.\nNext."),
        "prose after landed pycode/pyconsole must still split, got:\n{landed_out}"
    );
    assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
}

/// pythontex.sty option-family env names from `usefamily` /
/// `\makepythontexfamily` (GitHub #352).
fn pythontex_option_family_env_names() -> Vec<String> {
    const FAMILIES: &[&str] = &[
        "ruby",
        "rb",
        "julia",
        "jl",
        "matlab",
        "octave",
        "bash",
        "sage",
        "rust",
        "rs",
        "R",
        "perl",
        "pl",
        "perlsix",
        "psix",
        "javascript",
        "js",
    ];
    let mut names = Vec::new();
    for family in FAMILIES {
        for suffix in ["code", "block", "verbatim"] {
            names.push(format!("{family}{suffix}"));
            names.push(format!("{family}{suffix}*"));
        }
        names.push(format!("{family}sub"));
    }
    names.extend([
        "juliaconcode".into(),
        "juliaconsole".into(),
        "juliaconsole*".into(),
        "Rconcode".into(),
        "Rconsole".into(),
        "Rconsole*".into(),
    ]);
    names
}

/// Ticket fixture (GitHub #352): pythontex.sty option-family
/// `usefamily` / `\makepythontexfamily` envs stay Code; following
/// prose still splits. Landed `pycode` / `pyconsole` stay Code.
/// Default-family leftover `pyconcode` stays Code.
#[test]
fn pythontex_option_family_envs_fixture_is_code_and_does_not_reflow() {
    for name in pythontex_option_family_env_names() {
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

    let landed = concat!(
        "\\begin{pycode}\n",
        "First line. Second line.\n",
        "\\end{pycode}\n",
        "\\begin{pyconsole}\n",
        "First line. Second line.\n",
        "\\end{pyconsole}\n",
        "\\begin{pyconcode}\n",
        "First line. Second line.\n",
        "\\end{pyconcode}\n",
        "After the block. Next.\n",
    );
    let landed_out = format_text(landed, &latex_cfg()).unwrap();
    assert!(
        landed_out.contains("\\begin{pycode}\nFirst line. Second line.\n\\end{pycode}"),
        "landed pycode must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("\\begin{pyconsole}\nFirst line. Second line.\n\\end{pyconsole}"),
        "landed pyconsole must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("\\begin{pyconcode}\nFirst line. Second line.\n\\end{pyconcode}"),
        "leftover default-family pyconcode must stay a code env, got:\n{landed_out}"
    );
    assert!(
        landed_out.contains("After the block.\nNext."),
        "prose after landed pycode/pyconsole must still split, got:\n{landed_out}"
    );
    assert_eq!(format_text(&landed_out, &latex_cfg()).unwrap(), landed_out);
}

/// Ticket fixture (GitHub #394): minted.sty `\inputminted{lang}{file}`
/// stays one atomic command. Following flush `After.` does not join
/// the command line. `After.` / `Next.` still split. mintinline /
/// minted unchanged.
#[test]
fn inputminted_fixture_does_not_join_following_prose() {
    let input = concat!(
        "Before. Next.\n",
        "\\inputminted{python}{foo.py}\n",
        "After. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(r"\inputminted{python}{foo.py}")
        )),
        "inputminted must stay one Structure command, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains(r"\inputminted{python}{foo.py}")
        )),
        "inputminted must not leak into Prose, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After.") && p.contains("Next.")
        )),
        "After. / Next. must stay Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\inputminted{python}{foo.py}\n"),
        "inputminted must stay one atomic command, got:\n{out}"
    );
    assert!(
        !out.contains("\\inputminted{python}{foo.py} After."),
        "following flush prose must not join the command line, got:\n{out}"
    );
    assert!(
        out.contains("Before.\nNext."),
        "prose before inputminted must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after inputminted must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let mintinline = "Use \\mintinline{python}|a.b! c| here. Next sentence.\n";
    let mintinline_out = format_text(mintinline, &latex_cfg()).unwrap();
    assert!(
        mintinline_out.contains("Use \\mintinline{python}|a.b! c| here.\nNext sentence."),
        "mintinline must stay intact and still split, got:\n{mintinline_out}"
    );

    let minted = concat!(
        "\\begin{minted}{python}\n",
        "First line. Second line.\n",
        "\\end{minted}\n",
        "After the block. Next.\n",
    );
    let minted_out = format_text(minted, &latex_cfg()).unwrap();
    assert!(
        minted_out.contains("\\begin{minted}{python}\nFirst line. Second line.\n\\end{minted}"),
        "minted must stay a code env, got:\n{minted_out}"
    );
    assert!(
        minted_out.contains("After the block.\nNext."),
        "prose after minted must still split, got:\n{minted_out}"
    );
}

/// Ticket fixture (GitHub #400): tcolorbox `\tcbinputlisting{keyvals}`
/// stays one leftover command. Following flush `After.` does not join
/// the command line. `After.` / `Next.` still split. tcboutputlisting /
/// lstinputlisting / inputminted unchanged.
#[test]
fn tcbinputlisting_fixture_does_not_join_following_prose() {
    let input = concat!(
        "Before. Next.\n",
        "\\tcbinputlisting{listing file=foo.py}\n",
        "After. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(r"\tcbinputlisting{listing file=foo.py}")
        )),
        "tcbinputlisting must stay one Structure command, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains(r"\tcbinputlisting{listing file=foo.py}")
        )),
        "tcbinputlisting must not leak into Prose, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After.") && p.contains("Next.")
        )),
        "After. / Next. must stay Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\tcbinputlisting{listing file=foo.py}\n"),
        "tcbinputlisting must stay one atomic command, got:\n{out}"
    );
    assert!(
        !out.contains("\\tcbinputlisting{listing file=foo.py} After."),
        "following flush prose must not join the command line, got:\n{out}"
    );
    assert!(
        out.contains("Before.\nNext."),
        "prose before tcbinputlisting must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after tcbinputlisting must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let tcboutput = concat!(
        "\\begin{tcboutputlisting}\n",
        "First line. Second line.\n",
        "\\end{tcboutputlisting}\n",
        "After the block. Next.\n",
    );
    let tcboutput_out = format_text(tcboutput, &latex_cfg()).unwrap();
    assert!(
        tcboutput_out.contains(
            "\\begin{tcboutputlisting}\nFirst line. Second line.\n\\end{tcboutputlisting}"
        ),
        "tcboutputlisting must stay a code env, got:\n{tcboutput_out}"
    );
    assert!(
        tcboutput_out.contains("After the block.\nNext."),
        "prose after tcboutputlisting must still split, got:\n{tcboutput_out}"
    );

    let lstinput = concat!(
        "Before. Next.\n",
        "\\lstinputlisting{foo.py}\n",
        "After. Next.\n",
    );
    let lstinput_out = format_text(lstinput, &latex_cfg()).unwrap();
    assert!(
        lstinput_out.contains("\\lstinputlisting{foo.py}\n"),
        "lstinputlisting must stay one atomic command, got:\n{lstinput_out}"
    );
    assert!(
        !lstinput_out.contains("\\lstinputlisting{foo.py} After."),
        "lstinputlisting must not join following prose, got:\n{lstinput_out}"
    );
    assert!(
        lstinput_out.contains("After.\nNext."),
        "prose after lstinputlisting must still split, got:\n{lstinput_out}"
    );

    let minted_in = concat!(
        "Before. Next.\n",
        "\\inputminted{python}{foo.py}\n",
        "After. Next.\n",
    );
    let minted_in_out = format_text(minted_in, &latex_cfg()).unwrap();
    assert!(
        minted_in_out.contains("\\inputminted{python}{foo.py}\n"),
        "inputminted must stay one atomic command, got:\n{minted_in_out}"
    );
    assert!(
        !minted_in_out.contains("\\inputminted{python}{foo.py} After."),
        "inputminted must not join following prose, got:\n{minted_in_out}"
    );
    assert!(
        minted_in_out.contains("After.\nNext."),
        "prose after inputminted must still split, got:\n{minted_in_out}"
    );
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

/// Ticket fixture (GitHub #391): listings.sty `\lstinputlisting{file}`
/// stays one atomic command. Following flush `After.` does not join
/// the command line. `After.` / `Next.` still split. lstinline /
/// lstlisting unchanged.
#[test]
fn lstinputlisting_fixture_does_not_join_following_prose() {
    let input = concat!(
        "Before. Next.\n",
        "\\lstinputlisting{foo.py}\n",
        "After. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(r"\lstinputlisting{foo.py}")
        )),
        "lstinputlisting must stay one Structure command, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains(r"\lstinputlisting{foo.py}")
        )),
        "lstinputlisting must not leak into Prose, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After.") && p.contains("Next.")
        )),
        "After. / Next. must stay Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\lstinputlisting{foo.py}\n"),
        "lstinputlisting must stay one atomic command, got:\n{out}"
    );
    assert!(
        !out.contains("\\lstinputlisting{foo.py} After."),
        "following flush prose must not join the command line, got:\n{out}"
    );
    assert!(
        out.contains("Before.\nNext."),
        "prose before lstinputlisting must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after lstinputlisting must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let lstinline = "Use \\lstinline!a.b! here. Next sentence.\n";
    let lstinline_out = format_text(lstinline, &latex_cfg()).unwrap();
    assert!(
        lstinline_out.contains("Use \\lstinline!a.b! here.\nNext sentence."),
        "lstinline must stay intact and still split, got:\n{lstinline_out}"
    );

    let lstlisting = concat!(
        "\\begin{lstlisting}\n",
        "First line. Second line.\n",
        "\\end{lstlisting}\n",
        "After the block. Next.\n",
    );
    let lstlisting_out = format_text(lstlisting, &latex_cfg()).unwrap();
    assert!(
        lstlisting_out.contains("\\begin{lstlisting}\nFirst line. Second line.\n\\end{lstlisting}"),
        "lstlisting must stay a code env, got:\n{lstlisting_out}"
    );
    assert!(
        lstlisting_out.contains("After the block.\nNext."),
        "prose after lstlisting must still split, got:\n{lstlisting_out}"
    );
}

/// Ticket fixture (GitHub #305): piton.sty `{Piton}` body stays Code
/// on one source line; `\piton|done. Next|` is one token; following
/// prose still splits. minted / lstlisting / `\verb` unchanged.
#[test]
fn piton_env_and_pipe_cmd_fixture_is_code_and_does_not_reflow() {
    let input = concat!(
        "\\begin{Piton}\n",
        "First line. Second line.\n",
        "\\end{Piton}\n",
        "After the block. Next.\n",
    );
    let regions = LatexParser::default().parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("First line. Second line.")
        )),
        "Piton body must be Code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("First line"))),
        "Piton body must not be Prose, got: {regions:?}"
    );
    let out = format_text(input, &latex_cfg()).unwrap();
    assert!(
        out.contains("\\begin{Piton}") && out.contains("\\end{Piton}"),
        "Piton begin/end must stay, got:\n{out}"
    );
    assert!(
        out.contains("First line. Second line."),
        "Piton body must stay one source line, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "Piton must not reflow as prose, got:\n{out}"
    );
    assert!(
        out.contains("After the block.\nNext."),
        "prose after Piton must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);

    let cmd = "See \\piton|done. Next| here. After.\n";
    let cmd_out = format_text(cmd, &latex_cfg()).unwrap();
    assert!(
        cmd_out.contains(r"\piton|done. Next|"),
        "\\piton|done. Next| must stay one token, got:\n{cmd_out}"
    );
    assert!(
        !cmd_out.contains("\\piton|done.\n") && !cmd_out.contains("\\piton|done. Next|\n"),
        "pipe span must stay atomic, got:\n{cmd_out}"
    );
    assert!(
        cmd_out.contains("See \\piton|done. Next| here.\nAfter."),
        "prose after \\piton|...| must still split, got:\n{cmd_out}"
    );
    assert_eq!(format_text(&cmd_out, &latex_cfg()).unwrap(), cmd_out);

    let brace = "See \\piton{done. Next} here. After.\n";
    let brace_out = format_text(brace, &latex_cfg()).unwrap();
    assert!(
        brace_out.contains(r"\piton{done. Next}"),
        "\\piton{{done. Next}} must stay one token, got:\n{brace_out}"
    );
    assert!(
        brace_out.contains("See \\piton{done. Next} here.\nAfter."),
        "prose after \\piton{{...}} must still split, got:\n{brace_out}"
    );
    assert_eq!(format_text(&brace_out, &latex_cfg()).unwrap(), brace_out);

    let minted = concat!(
        "\\begin{minted}{python}\n",
        "print(1)\n",
        "print(2)\n",
        "\\end{minted}\n",
        "After the block. Next.\n",
    );
    let minted_out = format_text(minted, &latex_cfg()).unwrap();
    assert!(
        minted_out.contains("\\begin{minted}{python}\nprint(1)\nprint(2)\n\\end{minted}"),
        "minted must stay a code env, got:\n{minted_out}"
    );
    assert!(
        minted_out.contains("After the block.\nNext."),
        "prose after minted must still split, got:\n{minted_out}"
    );

    let listing = concat!(
        "\\begin{lstlisting}\n",
        "First line. Second line.\n",
        "\\end{lstlisting}\n",
        "After the block. Next.\n",
    );
    let listing_out = format_text(listing, &latex_cfg()).unwrap();
    assert!(
        listing_out.contains("First line. Second line."),
        "lstlisting body must stay one source line, got:\n{listing_out}"
    );
    assert!(
        !listing_out.contains("First line.\nSecond line."),
        "lstlisting must not reflow as prose, got:\n{listing_out}"
    );
    assert!(
        listing_out.contains("After the block.\nNext."),
        "prose after lstlisting must still split, got:\n{listing_out}"
    );

    let verb = "Use \\verb|a.b! c| here. Next sentence.\n";
    let verb_out = format_text(verb, &latex_cfg()).unwrap();
    assert!(
        verb_out.contains(r"\verb|a.b! c|"),
        "\\verb must stay intact, got:\n{verb_out}"
    );
    assert!(
        verb_out.contains("Use \\verb|a.b! c| here.\nNext sentence."),
        "prose after \\verb must still split, got:\n{verb_out}"
    );
}
