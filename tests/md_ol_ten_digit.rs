use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// snapper-sp7k / GitHub #172: CommonMark 0.31.2 sec 5.2 and pulldown
/// `scan_list_marker_with_indent` cap ordered markers at 9 digits.
/// `1234567890.` is prose, not a list. Oracle stays one `<p>`.

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
fn ten_digit_marker_is_prose_not_a_list() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("1234567890."))),
        "10-digit opener must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("1234567890.")
        )),
        "10-digit opener must not be a list Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| match r {
            Region::Structure(s) => {
                let t = s.trim();
                !t.is_empty()
                    && t.chars()
                        .all(|c| c.is_ascii_digit() || c == '.' || c == ')')
            }
            _ => false,
        }),
        "must not invent an ordered-list Structure, got: {regions:?}"
    );
}

#[test]
fn ten_digit_fixture_stays_one_paragraph() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("            Another sentence."),
        "must not hang-indent as a 10-digit list, got:\n{out}"
    );
    assert!(
        out.contains("Another sentence."),
        "second sentence must remain, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
    let twice = format_text(&out, &md_cfg()).unwrap();
    assert_eq!(out, twice, "10-digit prose must be identity, got:\n{twice}");
}

#[test]
fn nine_digit_marker_is_still_a_list() {
    let input = "123456789. This is a long sentence. Another sentence.";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "123456789. ")),
        "9-digit opener must stay a list, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("This is a long sentence."))),
        "9-digit item body must stay Prose, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("           Another sentence."),
        "9-digit list must hang the next sentence, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn ten_digit_paren_marker_is_prose() {
    let input = "1234567890) This is a long sentence. Another sentence.";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("1234567890)"))),
        "10-digit paren opener must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("1234567890)")
        )),
        "10-digit paren opener must not be a list Structure, got: {regions:?}"
    );
}
