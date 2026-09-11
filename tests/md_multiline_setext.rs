//! GitHub #208 / snapper-4wxk: CommonMark 4.3 ex. 50–51. pulldown
//! `parse_setext_heading` promotes the whole open paragraph. Pairing only
//! the last title line with the underline left earlier lines as Prose.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Markdown).
fn ticket_fixture() -> &'static str {
    concat!(
        "Foo is the first title line. Still title.\n",
        "Bar is the second title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    )
}

#[test]
fn multiline_setext_title_lines_and_underline_are_structure() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "Foo is the first title line. Still title.\n"
        )),
        "first title line must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "Bar is the second title line.\n"
        )),
        "second title line must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "=======")),
        "underline must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Still title")
                    || p.contains("Foo is the first")
                    || p.contains("Bar is the second")
        )),
        "title lines must not be Prose: {regions:?}"
    );
}

#[test]
fn multiline_setext_body_still_splits() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Body after setext") && p.contains("Second body")
        )),
        "body must stay Prose, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.starts_with(
            "Foo is the first title line. Still title.\nBar is the second title line.\n=======\n"
        ),
        "both title lines plus underline must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split the setext title, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
    assert!(
        !out.contains("Body after setext. Second body."),
        "fused body must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn multiline_setext_survives_safety_backstops() {
    let input = ticket_fixture();
    let cfg = FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    };
    let out = format_text(input, &cfg).unwrap();
    assert!(
        out.contains(
            "Foo is the first title line. Still title.\nBar is the second title line.\n======="
        ),
        "CLI backstops must not reflow the title, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "CLI backstops must not revert the body split, got:\n{out}"
    );
}

#[test]
fn multiline_setext_dash_underline_is_heading_not_hr() {
    let input = concat!(
        "Foo is the first title line. Still title.\n",
        "Bar is the second title line.\n",
        "-------\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "Foo is the first title line. Still title.\n"
        )),
        "first dash-setext title line must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "-------")),
        "dash underline must stay setext Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title")
        )),
        "dash-setext title must not be Prose: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// `<!-- toc -->` then setext must not duplicate the comment.
#[test]
fn setext_after_html_comment_emits_comment_once() {
    let input = concat!(
        "<!-- toc -->\n",
        "My Title\n",
        "========\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    let comments = regions
        .iter()
        .filter(|r| matches!(r, Region::Structure(s) if s.contains("<!-- toc -->")))
        .count();
    assert_eq!(
        comments, 1,
        "HTML comment must appear once, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "My Title\n")),
        "setext title must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("My Title") || p.contains("<!-- toc")
        )),
        "comment and title must not be Prose: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out.matches("<!-- toc -->").count(),
        1,
        "formatted comment must appear once, got:\n{out}"
    );
    assert!(
        out.contains("My Title\n========"),
        "title plus underline must stay intact, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// Indented code then setext must not copy the code as Structure.
#[test]
fn setext_after_indented_code_emits_code_once() {
    let input = concat!(
        "    code line\n",
        "Heading here\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert_eq!(
        regions
            .iter()
            .filter(|r| matches!(r, Region::Code { .. }))
            .count(),
        1,
        "indented code must appear once, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("code line")
        )),
        "indented code must not be re-emitted as Structure: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "Heading here\n")),
        "setext title must be Structure, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out.matches("code line").count(),
        1,
        "formatted code line must appear once, got:\n{out}"
    );
    assert!(
        out.contains("Heading here\n======="),
        "title plus underline must stay intact, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// GitHub #258 / CommonMark 5.2: only ordered start 1 interrupts a
/// paragraph. `2.` continues the open paragraph, so the following
/// underline promotes Foo and Bar.
fn ordered_start_fixture(marker: &str) -> String {
    format!(
        concat!(
            "Foo is the first title line. Still title.\n",
            "{marker} Bar is the second title line.\n",
            "=======\n",
            "\n",
            "Body after setext. Second body.\n",
        ),
        marker = marker
    )
}

fn assert_cm52_setext_promoted(input: &str, second_title: &str) {
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "Foo is the first title line. Still title.\n"
        )),
        "first title line must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == second_title
        )),
        "second title line must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "=======")),
        "underline must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Still title")
                    || p.contains("Foo is the first")
                    || p.contains("Bar is the second")
        )),
        "title lines must not be Prose: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split the setext title, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
    assert!(
        !out.contains("Body after setext. Second body."),
        "fused body must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn ordered_start_not_one_setext_is_structure() {
    for marker in ["2.", "0.", "10.", "2)"] {
        let input = ordered_start_fixture(marker);
        let second = format!("{marker} Bar is the second title line.\n");
        assert_cm52_setext_promoted(&input, &second);
    }
}

#[test]
fn quoted_ordered_start_not_one_setext_is_structure() {
    let input = concat!(
        "> Foo is the first title line. Still title.\n",
        "> 2. Bar is the second title line.\n",
        "> =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "> Foo is the first title line. Still title.\n"
        )),
        "quoted first title must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "> 2. Bar is the second title line.\n"
        )),
        "quoted 2. title must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "> =======")),
        "quoted underline must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Still title")
                    || p.contains("Foo is the first")
                    || p.contains("Bar is the second")
        )),
        "quoted title lines must not be Prose: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split the quoted setext title, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

#[test]
fn quoted_hung_ordered_start_not_one_setext_is_structure() {
    let input = concat!(
        "> Foo is the first title line. Still title.\n",
        "> 2. Bar is the second title line.\n",
        ">    =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "> Foo is the first title line. Still title.\n"
        )),
        "hung quoted first title must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("2. Bar is the second title line")
        )),
        "hung quoted 2. title must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("======="))),
        "hung quoted underline must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Bar is the second")
        )),
        "hung quoted title must not be Prose: {regions:?}"
    );
}

/// CM 5.2 ex. 301: `1.` after an open paragraph still interrupts.
#[test]
fn ordered_start_one_still_interrupts() {
    let input = concat!(
        "Foo is the first title line. Still title.\n",
        "1. Bar is the second title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "interrupted Foo must stay Prose, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "1.")),
        "1. must stay a list opener, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "Foo is the first title line. Still title.\n"
        )),
        "1. must not promote Foo into the heading, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Foo is the first title line.\nStill title."),
        "interrupted Foo must still split, got:\n{out}"
    );
}

/// Bullets still interrupt a paragraph (CM 5.2).
#[test]
fn bullet_still_interrupts_setext_paragraph() {
    for marker in ["-", "*", "+"] {
        let input = format!(
            concat!(
                "Foo is the first title line. Still title.\n",
                "{marker} Bar is the second title line.\n",
                "=======\n",
                "\n",
                "Body after setext. Second body.\n",
            ),
            marker = marker
        );
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
            )),
            "{marker} must leave Foo as Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s == "Foo is the first title line. Still title.\n"
            )),
            "{marker} must not promote Foo, got: {regions:?}"
        );
    }
}

/// A later underline still promotes the whole paragraph, including `2. Bar`.
#[test]
fn later_underline_promotes_start_not_one_continuation() {
    let input = concat!(
        "Foo is the first title line. Still title.\n",
        "2. Bar is the second title line.\n",
        "Baz is the third title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    assert_cm52_setext_promoted(input, "2. Bar is the second title line.\n");
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "Baz is the third title line.\n"
        )),
        "third title line must be Structure, got: {regions:?}"
    );
}

/// One-line list setext with a hung underline still holds (6g55).
#[test]
fn one_line_list_hung_underline_is_setext() {
    let input = concat!(
        "1. Bar is the title. Still title.\n",
        "   =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bar is the title")
        )),
        "1. Bar hung setext must be Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "=======")),
        "hung underline must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Bar is the title")
        )),
        "list setext title must not be Prose: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("Bar is the title.\n"),
        "must not sentence-split the list setext, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// Quoted list continuation plus quote-relative column-0 underline is
/// not a heading (GitHub #208). First-line list-opener reject is not
/// enough — Bar and `> =======` must stay out of Structure.
#[test]
fn quoted_list_column0_underline_is_not_setext() {
    let input = concat!(
        "> - Foo is the first title line. Still title.\n",
        ">   Bar is the second title line.\n",
        "> =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "quoted list Foo must stay Prose, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Bar is the second")
        )),
        "quoted list continuation Bar must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("Foo is the first") && s.contains("=======")
        )),
        "quoted list plus col-0 underline must not be one Structure heading, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bar is the second")
        )),
        "Bar must not become a setext title, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("=======")
        )),
        "quote-relative column-0 underline must not be Structure, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Foo is the first title line.\n") || out.contains("Still title."),
        "quoted list Foo must still split, got:\n{out}"
    );
}

/// 705k hung closer: `>   =======` after a quoted list stays a heading.
#[test]
fn quoted_list_hung_underline_is_setext() {
    let input = concat!(
        "> - Foo is the first title line. Still title.\n",
        ">   Bar is the second title line.\n",
        ">   =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bar is the second")
        )),
        "hung quoted list title must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("=======")
        )),
        "hung quoted underline must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Bar is the second")
        )),
        "hung quoted list title must not be Prose: {regions:?}"
    );
}
