use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// snapper-9dc1: Markdown `* 0. A.` is one list item. pulldown treats
/// `0. A.` as a nested ordered list. A sembr hang after `0.` is a
/// continuation paragraph, so the HTML oracle vetoes. Keep the opener
/// with the next sentence; do not invent a nested `0. ` Structure.

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn compact_ol_opener_stays_on_one_line() {
    let input = "* 0. A.";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, input, "must not sembr-hang after 0., got:\n{out}");
    assert!(
        !out.contains("0.\n"),
        "must not emit * 0.\\n  A., got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
    assert!(
        !snapper_fmt::oracle::matches(Format::Markdown, input, "* 0.\n  A."),
        "hung form must stay an oracle veto"
    );
    let twice = format_text(&out, &md_cfg()).unwrap();
    assert_eq!(
        out, twice,
        "compact ol opener must be identity, got:\n{twice}"
    );
}

#[test]
fn compact_ol_opener_is_not_a_nested_structure() {
    let regions = MarkdownParser.parse("* 0. A.");
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "* ")),
        "* marker must stay Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("0. A."))),
        "0. A. must stay one Prose region, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "0.")),
        "must not invent a nested 0. Structure, got: {regions:?}"
    );
}

#[test]
fn already_hung_ol_opener_stays_hung() {
    let input = "* 0.\n  A.";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, input,
        "already-hung continuation must stay hung, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn ordinary_list_item_still_hangs() {
    let input = "* One. Two.";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, "* One.\n  Two.",
        "ordinary sembr hang must stay, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}
