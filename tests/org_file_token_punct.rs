//! GitHub #169 / snapper-u77y: Org `file:` tokens must not swallow
//! trailing sentence punctuation.

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

#[test]
fn leftover_plain_link_after_path_hangs_and_splits() {
    let input = concat!("file:/tmp/plot.png leftover. Next.\n", "After. Next.\n",);
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("file:/tmp/plot.png"))),
        "plain link path must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("leftover.") && p.contains("Next.")
        )),
        "leftover after the path must be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("file:/tmp/plot.png leftover. Next."),
        "leftover after the path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_id_plain_link_after_path_hangs_and_splits() {
    let input = concat!("id:abc-123 leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("id:abc-123 leftover. Next."),
        "leftover after id path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_file_emacs_plain_link_after_path_hangs_and_splits() {
    let input = concat!(
        "file+emacs:/tmp/plot.png leftover. Next.\n",
        "After. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("file+emacs:/tmp/plot.png")
        )),
        "file+emacs path must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("file+emacs:/tmp/plot.png leftover. Next."),
        "leftover after file+emacs path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_file_sys_plain_link_after_path_hangs_and_splits() {
    let input = concat!("file+sys:/tmp/plot.png leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("file+sys:/tmp/plot.png leftover. Next."),
        "leftover after file+sys path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_shell_plain_link_after_path_hangs_and_splits() {
    let input = concat!("shell:ls leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("shell:ls leftover. Next."),
        "leftover after shell path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_elisp_plain_link_after_path_hangs_and_splits() {
    let input = concat!("elisp:(message \"hi\") leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("elisp:(message \"hi\") leftover. Next."),
        "leftover after elisp path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_help_plain_link_after_path_hangs_and_splits() {
    let input = concat!("help:org leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("help:org leftover. Next."),
        "leftover after help path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_info_plain_link_after_path_hangs_and_splits() {
    let input = concat!("info:org leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("info:org leftover. Next."),
        "leftover after info path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_eww_plain_link_after_path_hangs_and_splits() {
    let input = concat!(
        "eww:https://example.org leftover. Next.\n",
        "After. Next.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("eww:https://example.org leftover. Next."),
        "leftover after eww path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_irc_plain_link_after_path_hangs_and_splits() {
    let input = concat!(
        "irc:/irc.libera.chat/#org leftover. Next.\n",
        "After. Next.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("irc:/irc.libera.chat/#org leftover. Next."),
        "leftover after irc path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_attachment_plain_link_after_path_hangs_and_splits() {
    let input = concat!("attachment:plot.png leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("attachment:plot.png leftover. Next."),
        "leftover after attachment path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_ftp_plain_link_after_path_hangs_and_splits() {
    let input = concat!("ftp://example.com leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("ftp://example.com leftover. Next."),
        "leftover after ftp path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_doi_plain_link_after_path_hangs_and_splits() {
    let input = concat!("doi:10.1000/foo leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("doi:10.1000/foo leftover. Next."),
        "leftover after doi path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_mailto_plain_link_after_path_hangs_and_splits() {
    let input = concat!("mailto:dev@example.com leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("mailto:dev@example.com leftover. Next."),
        "leftover after mailto path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_https_plain_link_after_path_hangs_and_splits() {
    let input = concat!("https://example.com/a leftover. Next.\n", "After. Next.\n",);
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("https://example.com/a leftover. Next."),
        "leftover after https path must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn org_file_token_same_line_splits() {
    let two_line = "See file:/tmp/foo.\nNext sentence.\n";
    let out = format_text(two_line, &org_cfg()).unwrap();
    assert_eq!(
        out, two_line,
        "existing newline must stay two lines, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

    let out = format_text("See file:/tmp/foo. Next sentence.\n", &org_cfg()).unwrap();
    assert_eq!(
        out, two_line,
        "same-line file: period must split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn org_file_token_bang_and_question_split() {
    let cfg = org_cfg();
    let out = format_text("See file:/tmp/foo! Next sentence.\n", &cfg).unwrap();
    assert_eq!(out, "See file:/tmp/foo!\nNext sentence.\n", "got:\n{out}");
    let out = format_text("See file:/tmp/foo? Next sentence.\n", &cfg).unwrap();
    assert_eq!(out, "See file:/tmp/foo?\nNext sentence.\n", "got:\n{out}");
}
