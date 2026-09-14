//! Leftover LaTeX cmds/envs that must stay atomic Structure / Code.
//! Following flush prose must not join; After. / Next. still split.

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
