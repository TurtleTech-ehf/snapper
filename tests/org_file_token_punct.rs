//! GitHub #169 / snapper-u77y: Org `file:` tokens must not swallow
//! trailing sentence punctuation.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{format_text, FormatConfig};

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
