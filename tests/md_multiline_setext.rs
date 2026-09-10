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

/// Four-space `<!--` is title text, not a type-2 interrupt (snapper-5sck).
#[test]
fn lazy_four_space_html_comment_stays_setext_title() {
    let input = concat!(
        "Foo is the first title line. Still title.\n",
        "    <!-- toc -->\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("<!-- toc")
        )),
        "4-space comment must stay in the setext title, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split the 4-space-comment title, got:\n{out}"
    );
    assert!(
        out.contains("    <!-- toc -->\n======="),
        "4-space comment plus underline must stay intact, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// List and lazy-quote setext promote every title line (snapper-awpt).
#[test]
fn list_and_quote_multiline_setext_do_not_split() {
    let list = concat!(
        "- Foo is the first title line. Still title.\n",
        "  Bar is the second title line.\n",
        "  =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let list_regions = MarkdownParser.parse(list);
    assert!(
        !list_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "list setext first title line must not be Prose: {list_regions:?}"
    );
    let list_out = format_text(list, &md_cfg()).unwrap();
    assert!(
        !list_out.contains("first title line.\nStill"),
        "must not sentence-split a list setext title, got:\n{list_out}"
    );
    assert!(
        list_out.contains("Body after setext.\nSecond body."),
        "body after list setext must still split, got:\n{list_out}"
    );

    let quote = concat!(
        "> Foo is the first title line. Still title.\n",
        "Bar is the second title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let quote_regions = MarkdownParser.parse(quote);
    assert!(
        !quote_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "quote setext first title line must not be Prose: {quote_regions:?}"
    );
    let quote_out = format_text(quote, &md_cfg()).unwrap();
    assert!(
        !quote_out.contains("first title line.\nStill"),
        "must not sentence-split a quote setext title, got:\n{quote_out}"
    );
}

/// Hard-break title lines stay in the open paragraph (snapper-j945).
#[test]
fn hard_break_multiline_setext_does_not_split() {
    let input = concat!(
        "Foo is the first title line. Still title.  \n",
        "Bar is the second title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "hard-break setext first title line must not be Prose: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split a hard-break setext title, got:\n{out}"
    );
    assert!(
        out.contains("Foo is the first title line. Still title.  \n")
            || out.contains("Foo is the first title line. Still title.  \r"),
        "hard-break spaces must remain, got:\n{out:?}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}
