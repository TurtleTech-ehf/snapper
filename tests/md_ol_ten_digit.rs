use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #172 / snapper-sp7k: CommonMark 0.31.2 §5.2 and pulldown
/// `scan_list_marker_with_indent` (`ix-start < 10`) reject 10+ digit
/// ordered list markers. `LIST_ITEM_RE` used `\d+` so `1234567890.`
/// became a list.

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    "1234567890. This is a long sentence. Another sentence."
}

#[test]
fn ten_digit_marker_is_prose_only() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().all(|r| matches!(r, Region::Prose(_))),
        "10-digit marker must be Prose only, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("1234567890.") && p.contains("Another sentence.")
        )),
        "fixture must stay one Prose region, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(r, Region::Structure(_))),
        "must not invent a 10-digit list Structure, got {regions:?}"
    );
}

#[test]
fn ten_digit_marker_reflows_as_prose_not_list() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("1234567890.\nThis is a long sentence.\nAnother sentence."),
        "10-digit opener must sembr as prose, got:\n{out}"
    );
    assert!(
        !out.contains("            Another sentence."),
        "10-digit opener must not hang as a list, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle must stay one paragraph\n in={input:?}\n out={out:?}"
    );
    let twice = format_text(&out, &md_cfg()).unwrap();
    assert_eq!(
        out, twice,
        "10-digit prose must be identity on a second pass, got:\n{twice}"
    );
}

#[test]
fn nine_digit_marker_is_still_a_list() {
    let input = "123456789. This is a long sentence. Another sentence.";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "123456789. ")),
        "9-digit marker must stay a list, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("123456789. This is a long sentence.\n           Another sentence."),
        "9-digit marker must still hang, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "9-digit list hang must stay oracle-ok\n in={input:?}\n out={out:?}"
    );
}
