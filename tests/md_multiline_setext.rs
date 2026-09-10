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

/// snapper-5sck: 4-space `<!--` in an open paragraph is title text.
#[test]
fn four_space_html_comment_stays_setext_title() {
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
        "4-space comment title must not be Prose: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split the 4-space-comment setext, got:\n{out}"
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

/// snapper-awpt: list-item setext promotes every title line.
#[test]
fn list_multiline_setext_does_not_split_first_title_line() {
    let input = concat!(
        "- Foo is the first title line. Still title.\n",
        "  Bar is the second title line.\n",
        "  =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "list setext first title line must not be Prose: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split a list setext title, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// A list item plus a column-0 underline is not inside the item.
#[test]
fn list_then_column0_underline_is_not_setext() {
    let input = concat!(
        "- Foo is a list item. Still item.\n",
        "=======\n",
        "\n",
        "Body after list. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("list item") && p.contains("Still item")
        )),
        "column-0 ======= must not promote the list item, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("list item.\n  Still item."),
        "list item Prose must still split and hang, got:\n{out}"
    );
    assert!(
        out.contains("Body after list.\nSecond body."),
        "body after the list must still split, got:\n{out}"
    );
}

/// snapper-wu2v / CommonMark 4.3 ex. 93: lazy unquoted `=======` after
/// a quote is still the quote paragraph, not a setext heading.
#[test]
fn lazy_quote_equals_underline_is_not_setext() {
    let lazy = concat!(
        "> Foo is the first title line. Still title.\n",
        "Bar is the second title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(lazy);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "lazy quote ======= must stay quote Prose, got: {regions:?}"
    );
    let out = format_text(lazy, &md_cfg()).unwrap();
    assert!(
        out.contains("first title line.\n"),
        "first quote sentence must still split, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );

    let marked = concat!(
        "> Foo is the first title line. Still title.\n",
        "> Bar is the second title line.\n",
        "> =======\n",
    );
    let regions = MarkdownParser.parse(marked);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Bar is the second")
        )),
        "fully marked quote setext must not be Prose: {regions:?}"
    );
}

/// `Foo` then `> =======` is a new blockquote, not a setext closer.
#[test]
fn unquoted_title_then_quoted_underline_is_not_setext() {
    let input = concat!(
        "Foo is the first title line. Still title.\n",
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
        "Foo then > ======= must stay Prose, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("first title line.\nStill"),
        "unquoted title must still split, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// snapper-khbh: quote opener as last title line interrupts the paragraph.
#[test]
fn quote_opener_as_last_setext_line_leaves_prior_paragraph_prose() {
    let input = concat!(
        "Foo is the first title line. Still title.\n",
        "> Bar is the second title line.\n",
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
        "blockquote interrupts: Foo must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Bar is the second")
        )),
        "quoted setext title must not be Prose: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bar is the second title line.")
        )),
        "Bar plus underline must be the quote setext, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("first title line.\nStill"),
        "Foo must still split, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// snapper-khbh: list opener as last title line interrupts the paragraph.
#[test]
fn list_opener_as_last_setext_line_leaves_prior_paragraph_prose() {
    let input = concat!(
        "Previous paragraph. Still prose.\n",
        "- Foo is the title\n",
        "  =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Previous paragraph") || p.contains("Still prose")
        )),
        "list interrupts: prior paragraph must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Foo is the title")
        )),
        "list setext title must not be Prose: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Foo is the title")
        )),
        "list item must be the setext title, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Previous paragraph.\nStill prose."),
        "prior paragraph must still split, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}

/// snapper-0772: nested quote opener interrupts the outer quote paragraph.
#[test]
fn nested_quote_opener_as_last_setext_line_leaves_outer_quote_prose() {
    let tight = concat!(
        "> Foo is the first title line. Still title.\n",
        ">> Bar is the second title line.\n",
        ">> =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(tight);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "nested >> interrupts: Foo must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Bar is the second")
        )),
        "inner quote setext title must not be Prose: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bar is the second title line.")
        )),
        "Bar plus underline must be the inner quote setext, got: {regions:?}"
    );
    let out = format_text(tight, &md_cfg()).unwrap();
    assert!(
        out.contains("first title line.\n> Still"),
        "outer quote Foo must still split, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );

    let spaced = concat!(
        "> Foo is the first title line. Still title.\n",
        "> > Bar is the second title line.\n",
        "> > =======\n",
    );
    let regions = MarkdownParser.parse(spaced);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "spaced > > interrupts: Foo must stay Prose, got: {regions:?}"
    );
}

/// snapper-6g55: list opener after an open quote is a new heading.
#[test]
fn list_opener_after_quote_leaves_quote_paragraph_prose() {
    let unquoted = concat!(
        "> Foo is quoted. Still quoted.\n",
        "- Bar is the title\n",
        "  =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(unquoted);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still quoted") || p.contains("Foo is quoted")
        )),
        "list after quote: Foo must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Bar is the title")
        )),
        "list setext title must not be Prose: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bar is the title")
        )),
        "list item must be the setext title, got: {regions:?}"
    );
    let out = format_text(unquoted, &md_cfg()).unwrap();
    assert!(
        out.contains("Foo is quoted.\n> Still quoted."),
        "quote paragraph must still split, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );

    let quoted_list = concat!(
        "> Foo is quoted. Still quoted.\n",
        "> - Bar is the title\n",
        ">   =======\n",
    );
    let regions = MarkdownParser.parse(quoted_list);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still quoted") || p.contains("Foo is quoted")
        )),
        "quoted list opener: Foo must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Bar is the title")
        )),
        "quoted list setext title must not be Prose: {regions:?}"
    );
}

/// snapper-j945: hard-break flush is not a paragraph close.
#[test]
fn hard_break_multiline_setext_does_not_split_first_title_line() {
    let spaces = concat!(
        "Foo is the first title line. Still title.  \n",
        "Bar is the second title line.\n",
        "=======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(spaces);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "hard-break setext first title line must not be Prose: {regions:?}"
    );
    let out = format_text(spaces, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split a hard-break setext, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );

    let backslash = concat!(
        "Foo is the first title line. Still title.\\\n",
        "Bar is the second title line.\n",
        "=======\n",
    );
    let regions = MarkdownParser.parse(backslash);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "backslash hard-break setext must not leak Prose: {regions:?}"
    );
}

/// snapper-0dnt: hard-break flush must not let indented code steal a
/// setext title continuation.
#[test]
fn hard_break_then_indented_setext_does_not_split_first_title_line() {
    let input = concat!(
        "Foo is the first title line. Still title.  \n",
        "    Bar is a lazy title line.\n",
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
        "hard-break title must not stay Prose, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "    Bar is a lazy title line.\n"
        )),
        "4-space continuation after hard break must stay title, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(r, Region::Code { .. })),
        "4-space title continuation must not become Code: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\nStill"),
        "must not sentence-split a hard-break+indent setext, got:\n{out}"
    );
    assert!(
        out.contains("    Bar is a lazy title line.\n======="),
        "lazy indent plus underline must stay intact, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );
}
