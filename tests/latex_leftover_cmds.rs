//! Leftover LaTeX cmds/envs that must stay atomic Structure / Code.
//! Following flush prose must not join; After. / Next. still split.

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

fn leftover_cmd_stays_atomic(cmd: &str) {
    let input = format!("Before. Next.\n{cmd}\nAfter. Next.\n");
    let regions = LatexParser::default().parse(&input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(cmd)
        )),
        "{cmd} must stay one Structure command, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains(cmd)
        )),
        "{cmd} must not leak into Prose, got: {regions:?}"
    );
    let out = format_text(&input, &latex_cfg()).unwrap();
    assert!(
        out.contains(&format!("{cmd}\n")),
        "{cmd} must stay one atomic command, got:\n{out}"
    );
    assert!(
        !out.contains(&format!("{cmd} After.")),
        "following flush prose must not join the {cmd} line, got:\n{out}"
    );
    assert!(
        out.contains("Before.\nNext."),
        "prose before {cmd} must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after {cmd} must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &latex_cfg()).unwrap(), out);
}

fn extras_skip_no_brace(name: &str) {
    let extras_cfg = FormatConfig {
        format: Format::Latex,
        latex_verbatim_commands: vec![name.to_string()],
        ..Default::default()
    }
    .without_safety_backstops();
    let out = format_text(
        &format!("Before. Next.\n\\{name} After. Next.\n"),
        &extras_cfg,
    )
    .unwrap();
    assert!(
        out.contains("After.\nNext."),
        "configured extra {name} must not re-tokenize the no-brace form as Delim, got:\n{out}"
    );
}

#[test]
fn listings_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\lstset{language=Python}");
    leftover_cmd_stays_atomic(r"\lstdefinestyle{mystyle}{language=Python}");
    leftover_cmd_stays_atomic(r"\lstMakeShortInline|");
    leftover_cmd_stays_atomic(r"\lstDeleteShortInline|");
    leftover_cmd_stays_atomic(r"\lstnewenvironment{mylst}{\lstset{language=Python}}{}");
    extras_skip_no_brace("lstset");
    extras_skip_no_brace("lstnewenvironment");
    extras_skip_no_brace("lstMakeShortInline");
}

#[test]
fn fvextra_clear_write_buffer_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\ClearBuffer");
    leftover_cmd_stays_atomic(r"\WriteBuffer");
    leftover_cmd_stays_atomic(r"\ClearBuffer[foo]");
    leftover_cmd_stays_atomic(r"\WriteBuffer[foo]");
    extras_skip_no_brace("ClearBuffer");
    extras_skip_no_brace("WriteBuffer");
}

#[test]
fn pyluatex_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\pyfile{foo.py}");
    leftover_cmd_stays_atomic(r"\pyfileq{foo.py}");
    leftover_cmd_stays_atomic(r"\pyfilerepl{foo.py}");
    leftover_cmd_stays_atomic(r"\pyq{print(1)}");
    leftover_cmd_stays_atomic(r"\pycq{print(1)}");
    extras_skip_no_brace("pyfile");
    extras_skip_no_brace("pyq");
}

#[test]
fn tcboxverb_does_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\tcboxverb{First. Second.}");
    leftover_cmd_stays_atomic(r"\tcboxverb|First. Second.|");
    extras_skip_no_brace("tcboxverb");
}

#[test]
fn sverb_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\verbinput{foo.py}");
    leftover_cmd_stays_atomic(r"\verbwrite{foo.py}");
    extras_skip_no_brace("verbinput");
    extras_skip_no_brace("verbwrite");
}

#[test]
fn scontents_setup_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\setupsc{print-cmd=true}");
    leftover_cmd_stays_atomic(r"\countsc{foo}");
    leftover_cmd_stays_atomic(r"\cleanseqsc{foo}");
    extras_skip_no_brace("setupsc");
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
fn sverb_verbwrite_star_env_is_code() {
    code_env_does_not_reflow("verbwrite*", "{tmp.tex}");
}

#[test]
fn tcolorbox_external_write_envs_are_code() {
    code_env_does_not_reflow("extcolorbox", "{name}");
    code_env_does_not_reflow("extikzpicture", "{name}");
}

#[test]
fn minted_setminted_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\setminted{style=bw}");
    leftover_cmd_stays_atomic(r"\setminted[python]{style=bw}");
    leftover_cmd_stays_atomic(r"\setmintedinline{breaklines=true}");
    leftover_cmd_stays_atomic(r"\usemintedstyle{bw}");
    extras_skip_no_brace("setminted");
}

#[test]
fn fvset_and_fvinlineset_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\fvset{fontsize=\small}");
    leftover_cmd_stays_atomic(r"\fvinlineset{breaklines=true}");
    extras_skip_no_brace("fvset");
    extras_skip_no_brace("fvinlineset");
}

#[test]
fn pythontex_print_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\printpythontex");
    leftover_cmd_stays_atomic(r"\stdoutpythontex");
    leftover_cmd_stays_atomic(r"\stderrpythontex");
    leftover_cmd_stays_atomic(r"\setpythontexfv{gobble=2}");
    extras_skip_no_brace("printpythontex");
    extras_skip_no_brace("setpythontexfv");
}

#[test]
fn pyluatex_pysession_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\pysession{main}");
    leftover_cmd_stays_atomic(r"\pyoption{verbose}{true}");
    extras_skip_no_brace("pysession");
    extras_skip_no_brace("pyoption");
}

#[test]
fn piton_options_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\PitonOptions{language=Python}");
    leftover_cmd_stays_atomic(r"\SetPitonStyle{Number=\bfseries}");
    extras_skip_no_brace("PitonOptions");
}

#[test]
fn tcolorbox_use_listing_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\tcbuselistingtext");
    leftover_cmd_stays_atomic(r"\tcbuselistinglisting");
    leftover_cmd_stays_atomic(r"\tcbusetemplisting");
    extras_skip_no_brace("tcbuselistingtext");
}

#[test]
fn leftover_constructors_do_not_join() {
    leftover_cmd_stays_atomic(r"\newminted{python}{linenos}");
    leftover_cmd_stays_atomic(r"\newmint[py]{python}{linenos}");
    leftover_cmd_stays_atomic(r"\newmintinline{python}{linenos}");
    leftover_cmd_stays_atomic(r"\newmintedfile{python}{linenos}");
    leftover_cmd_stays_atomic(r"\DefineVerbatimEnvironment{code}{Verbatim}{fontsize=\small}");
    leftover_cmd_stays_atomic(r"\RecustomVerbatimEnvironment{Verbatim}{Verbatim}{fontsize=\small}");
    leftover_cmd_stays_atomic(r"\DefineVerbatimCommand{myverb}{Verb}{fontsize=\small}");
    leftover_cmd_stays_atomic(r"\NewPitonEnvironment{py}{}{}{}");
    leftover_cmd_stays_atomic(r"\DeclarePitonEnvironment{py}{}{}{}");
    leftover_cmd_stays_atomic(r"\renewminted{python}{linenos}");
    leftover_cmd_stays_atomic(r"\renewmint{python}{linenos}");
    leftover_cmd_stays_atomic(r"\CustomVerbatimEnvironment{code}{Verbatim}{fontsize=\small}");
    leftover_cmd_stays_atomic(r"\newtcblisting{mylst}{listing options}");
    leftover_cmd_stays_atomic(r"\CustomVerbatimCommand{myverb}{Verb}{fontsize=\small}");
    leftover_cmd_stays_atomic(r"\setpygmentsfv{gobble=2}");
    leftover_cmd_stays_atomic(r"\listoflistings");
    leftover_cmd_stays_atomic(r"\MintedRegisterTempFileExtension{.listing}");
    leftover_cmd_stays_atomic(r"\pyif{a == 1}{$a = 1$}{$a \\neq 1$}");
    leftover_cmd_stays_atomic(r"\lstdefineformat{C}{;=}");
    leftover_cmd_stays_atomic(r"\NewTCBListing{code}{O{}}{listing only}");
    leftover_cmd_stays_atomic(r"\newtcbinputlisting{\mylisting}{listing file={foo.py}}");
    leftover_cmd_stays_atomic(r"\newtcolorbox{mybox}{colback=red}");
    leftover_cmd_stays_atomic(r"\NewTColorBox{mybox}{O{}}{colback=red}");
    leftover_cmd_stays_atomic(r"\tcbset{colback=red}");
    leftover_cmd_stays_atomic(r"\tcbuselibrary{listings}");
    leftover_cmd_stays_atomic(r"\setpythontexprettyprinter{pygments}");
    leftover_cmd_stays_atomic(r"\setpythontexprettyprinter[py]{pygments}");
    leftover_cmd_stays_atomic(r"\setpygmentsprettyprinter{pygments}");
    leftover_cmd_stays_atomic(r"\setpythontexworkingdir{plots}");
    leftover_cmd_stays_atomic(r"\setpythontexoutputdir{pythontex-files}");
    leftover_cmd_stays_atomic(r"\renewtcbinputlisting{\mylisting}{listing file={foo.py}}");
    leftover_cmd_stays_atomic(r"\RenewTCBInputListing{\mylisting}{O{}}{listing file={foo.py}}");
    leftover_cmd_stays_atomic(r"\lstloadaspects{strings}");
    leftover_cmd_stays_atomic(r"\BufferMdfivesum");
    leftover_cmd_stays_atomic(r"\BufferMdfivesum[foo]");
    leftover_cmd_stays_atomic(r"\sagetexpause");
    leftover_cmd_stays_atomic(r"\sagetexunpause");
    leftover_cmd_stays_atomic(r"\newenvsc{foo}{beg. end}{end. more}");
    leftover_cmd_stays_atomic(r"\newtcbox{\mybox}{colback=red}");
    leftover_cmd_stays_atomic(r"\NewTotalTCBox{\foo}{v}{colback=red}{done. Next}");
    leftover_cmd_stays_atomic(r"\RenewTotalTCBox{\foo}{v}{colback=red}{done. Next}");
    leftover_cmd_stays_atomic(r"\tcbsetforeverylayer{colback=red}");
    leftover_cmd_stays_atomic(r"\newtcbtheorem{theo}{Theorem}{colback=red}{th}");
    leftover_cmd_stays_atomic(r"\tcblistof{fig}{List of theorems}");
    leftover_cmd_stays_atomic(r"\tcboxmath{x = 1. 2}");
    leftover_cmd_stays_atomic(r"\tcbox{First. Second.}");
    leftover_cmd_stays_atomic(r"\tcboxfit{First. Second.}");
    leftover_cmd_stays_atomic(r"\tcbincludegraphics{foo.png}");
    leftover_cmd_stays_atomic(r"\tcbsubtitle{First. Second.}");
    leftover_cmd_stays_atomic(r"\newtcboxfit{\mybox}{colback=red}");
    leftover_cmd_stays_atomic(r"\NewTCBoxFit{\foo}{O{}}{colback=red}");
    leftover_cmd_stays_atomic(r"\ProvideTCBox{\mybox}{O{}}{colback=red}");
    leftover_cmd_stays_atomic(r"\renewtcboxfit{\mybox}{colback=red}");
    leftover_cmd_stays_atomic(r"\tcbsetforeverylisting{listing options}");
    leftover_cmd_stays_atomic(r"\tcbsetfiltered{colback=red}");
    leftover_cmd_stays_atomic(r"\tcbuselisting");
    leftover_cmd_stays_atomic(r"\NewTotalTColorBox{\foo}{O{}}{colback=red}{done. Next}");
    leftover_cmd_stays_atomic(r"\NewTotalTCBoxFit{\foo}{O{}}{colback=red}{done. Next}");
    leftover_cmd_stays_atomic(r"\renewtcbtheorem{theo}{Theorem}{colback=red}{th}");
    leftover_cmd_stays_atomic(r"\tcolorboxenvironment{quote}{colback=red}");
    leftover_cmd_stays_atomic(r"\tcbincludepdf{foo.pdf}");
    leftover_cmd_stays_atomic(r"\tcbtitle");
    leftover_cmd_stays_atomic(r"\listinglabel");
    leftover_cmd_stays_atomic(r"\listingoffset");
    leftover_cmd_stays_atomic(r"\verbatimtabsize");
    leftover_cmd_stays_atomic(r"\CatchFileBetweenTags*{\tmp}{foo.tex}{TAG}");
    leftover_cmd_stays_atomic(r"\ExecuteMetaData*{tag}");
    leftover_cmd_stays_atomic(r"\tcboxraise{1em}");
    leftover_cmd_stays_atomic(r"\tcbsetmanagedlayer{1}");
    leftover_cmd_stays_atomic(r"\listingscaption");
    leftover_cmd_stays_atomic(r"\listoflistingscaption");
    leftover_cmd_stays_atomic(r"\FancyVerbFormatInline{First. Second.}");
    leftover_cmd_stays_atomic(r"\DepythontexOn");
    leftover_cmd_stays_atomic(r"\tcbsidebyside{left. More}{right. More}");
    leftover_cmd_stays_atomic(r"\tcbstartrecording");
    leftover_cmd_stays_atomic(r"\tcbsetmanagedlayers{3}");
    leftover_cmd_stays_atomic(r"\tcbsubskin{mine}{standard}{colback=red}");
    leftover_cmd_stays_atomic(r"\tcbifoddpage{odd. More}{even. More}");
    leftover_cmd_stays_atomic(r"\tcbheightfromgroup{\h}{grp}");
    leftover_cmd_stays_atomic(r"\tcbpatcharcround");
    leftover_cmd_stays_atomic(r"\tcbhyperref{sec:foo}");
    leftover_cmd_stays_atomic(r"\tcbline");
    leftover_cmd_stays_atomic(r"\tcbmakeprefixed{\myref}{th}");
    leftover_cmd_stays_atomic(r"\setpythontexcontext{foo=bar}");
    leftover_cmd_stays_atomic(r"\restartpythontexsession");
    extras_skip_no_brace("newminted");
    extras_skip_no_brace("newmint");
    extras_skip_no_brace("renewminted");
    extras_skip_no_brace("DefineVerbatimEnvironment");
    extras_skip_no_brace("NewPitonEnvironment");
    extras_skip_no_brace("newtcblisting");
    extras_skip_no_brace("listoflistings");
    extras_skip_no_brace("pyif");
    extras_skip_no_brace("lstdefineformat");
    extras_skip_no_brace("newtcolorbox");
    extras_skip_no_brace("tcbset");
    extras_skip_no_brace("tcbuselibrary");
    extras_skip_no_brace("setpythontexprettyprinter");
    extras_skip_no_brace("lstloadaspects");
    extras_skip_no_brace("BufferMdfivesum");
    extras_skip_no_brace("sagetexpause");
    extras_skip_no_brace("newenvsc");
    extras_skip_no_brace("newtcbox");
    extras_skip_no_brace("tcbsetforeverylayer");
    extras_skip_no_brace("newtcbtheorem");
    extras_skip_no_brace("tcblistof");
    extras_skip_no_brace("tcboxmath");
    extras_skip_no_brace("tcbox");
    extras_skip_no_brace("tcboxfit");
    extras_skip_no_brace("tcbincludegraphics");
    extras_skip_no_brace("ProvideTCBox");
    extras_skip_no_brace("tcbsetforeverylisting");
    extras_skip_no_brace("tcbuselisting");
    extras_skip_no_brace("tcolorboxenvironment");
    extras_skip_no_brace("listinglabel");
    extras_skip_no_brace("CatchFileBetweenTags");
    extras_skip_no_brace("tcboxraise");
    extras_skip_no_brace("tcbsetmanagedlayer");
    extras_skip_no_brace("listingscaption");
    extras_skip_no_brace("FancyVerbFormatInline");
    extras_skip_no_brace("tcbsidebyside");
    extras_skip_no_brace("tcbsetmanagedlayers");
    extras_skip_no_brace("tcbhyperref");
    extras_skip_no_brace("restartpythontexsession");
}

#[test]
fn leftover_saveprint_and_replay_cmds_do_not_join() {
    leftover_cmd_stays_atomic(r"\saveprintpythontex{out}");
    leftover_cmd_stays_atomic(r"\savestdoutpythontex{out}");
    leftover_cmd_stays_atomic(r"\useprintpythontex{out}");
    leftover_cmd_stays_atomic(r"\usestdoutpythontex[verb]{out}");
    leftover_cmd_stays_atomic(r"\PitonClearUserFunctions");
    leftover_cmd_stays_atomic(r"\PitonClearUserFunctions[Python]");
    leftover_cmd_stays_atomic(r"\lstlistoflistings");
    leftover_cmd_stays_atomic(r"\tcbusetemp");
    leftover_cmd_stays_atomic(r"\setpythontexpyglexer{py}{python}");
    extras_skip_no_brace("saveprintpythontex");
    extras_skip_no_brace("PitonClearUserFunctions");
    extras_skip_no_brace("lstlistoflistings");
    extras_skip_no_brace("tcbusetemp");
}

#[test]
fn leftover_keyval_siblings_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\pyoptions{verbose}");
    leftover_cmd_stays_atomic(r"\setpythontexautoprint{true}");
    leftover_cmd_stays_atomic(r"\setpythontexautostdout{false}");
    leftover_cmd_stays_atomic(r"\NewPitonLanguage{HTML}{morekeywords={div}}");
    leftover_cmd_stays_atomic(r"\SetPitonIdentifier{print}{\bfseries}");
    leftover_cmd_stays_atomic(r"\lstalias{shell}{bash}");
    extras_skip_no_brace("pyoptions");
    extras_skip_no_brace("lstalias");
    extras_skip_no_brace("NewPitonLanguage");
}

#[test]
fn listings_load_language_leftover_cmds_do_not_join_following_prose() {
    leftover_cmd_stays_atomic(r"\lstloadlanguages{Python}");
    leftover_cmd_stays_atomic(r"\lstdefinelanguage{MyLang}{morekeywords={foo}}");
    extras_skip_no_brace("lstloadlanguages");
    extras_skip_no_brace("lstdefinelanguage");
}

#[test]
fn inc_corp_ltd_title_case_merge() {
    let cfg = FormatConfig {
        format: Format::Latex,
        ..Default::default()
    }
    .without_safety_backstops();
    let out = format_text("See Acme Inc. Next quarter.\n", &cfg).unwrap();
    assert_eq!(
        out, "See Acme Inc. Next quarter.\n",
        "Inc. must merge like inc., got:\n{out}"
    );
    let corp = format_text("See Acme Corp. Next quarter.\n", &cfg).unwrap();
    assert_eq!(
        corp, "See Acme Corp. Next quarter.\n",
        "Corp. must merge like corp., got:\n{corp}"
    );
    let ltd = format_text("See Acme Ltd. Next quarter.\n", &cfg).unwrap();
    assert_eq!(
        ltd, "See Acme Ltd. Next quarter.\n",
        "Ltd. must merge like ltd., got:\n{ltd}"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}
