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

/// snapper-32gc / GitHub #261: CommonMark 5.2. Setext underline must sit
/// at the list hang. Hang of `1. ` is 3; two spaces is below hang.
#[test]
fn ordered_list_underline_below_hang_is_not_setext() {
    let input = concat!(
        "1. Foo is a list item. Still item.\n",
        "  =======\n",
        "\n",
        "Body after list. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("list item") && p.contains("Still item")
        )),
        "two-space ======= must not promote 1. Foo, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Foo is a list item")
        )),
        "1. Foo must stay Prose, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("list item.\n   Still item."),
        "Foo must still split at hang 3, got:\n{out}"
    );
    assert!(
        !out.contains("list item. Still item."),
        "fused Foo must not survive, got:\n{out}"
    );
    assert!(
        out.contains("Body after list.\nSecond body."),
        "body after the list must still split, got:\n{out}"
    );
}

/// snapper-32gc: hang of `- ` is 2, so two-space `=======` stays a heading.
#[test]
fn bullet_list_underline_at_hang_is_setext() {
    let input = concat!(
        "- Foo is a list item. Still item.\n",
        "  =======\n",
        "\n",
        "Body after list. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("list item") || p.contains("Still item")
        )),
        "two-space ======= is at hang 2 and must stay a heading, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("list item.\n"),
        "must not sentence-split a hang-2 list setext, got:\n{out}"
    );
    assert!(
        out.contains("Body after list.\nSecond body."),
        "body after the heading must still split, got:\n{out}"
    );
}

/// snapper-32gc: after quote markers, one space is below hang 2; two
/// spaces sit at hang. Multi-line quoted list is the same hole.
#[test]
fn quoted_list_underline_below_hang_is_not_setext() {
    let below = concat!(
        "> - Foo is the first title line. Still title.\n",
        ">  =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(below);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "quoted list plus >  ======= must stay Prose, got: {regions:?}"
    );
    let out = format_text(below, &md_cfg()).unwrap();
    assert!(
        out.contains("first title line.\n"),
        "Foo must still split, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );

    let multi = concat!(
        "> - Foo is the first title line. Still title.\n",
        ">   Bar is the second title line.\n",
        ">  =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let multi_regions = MarkdownParser.parse(multi);
    assert!(
        multi_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "multi-line quoted list plus >  ======= must stay Prose, got: {multi_regions:?}"
    );

    let at_hang = concat!(
        "> - Foo is the first title line. Still title.\n",
        ">   =======\n",
    );
    let at_hang_regions = MarkdownParser.parse(at_hang);
    assert!(
        !at_hang_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "quoted list plus >   ======= must stay a heading, got: {at_hang_regions:?}"
    );
}

/// snapper-34pr / GitHub #259: `>  >` interrupts like `>>`. Foo stays
/// Prose and still splits; Bar plus the underline are Structure. Same
/// for three spaces and a tab. Five spaces after `>` is not a second
/// marker.
#[test]
fn two_or_three_space_nested_quote_leaves_outer_quote_prose() {
    for inner in [">  >", ">   >", ">\t>"] {
        let input = format!(
            concat!(
                "> Foo is the first title line. Still title.\n",
                "{inner} Bar is the second title line.\n",
                "{inner} =======\n",
                "\n",
                "Body after setext. Second body.\n",
            ),
            inner = inner
        );
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
            )),
            "{inner:?} nested quote interrupts: Foo must stay Prose, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Bar is the second")
            )),
            "{inner:?} inner setext title must not be Prose: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Bar is the second title line.")
            )),
            "{inner:?} Bar plus underline must be Structure, got: {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert!(
            out.contains("first title line.\n"),
            "{inner:?} Foo must still split, got:\n{out}"
        );
        assert!(
            out.contains("Body after setext.\nSecond body."),
            "{inner:?} body Prose must still split, got:\n{out}"
        );
    }

    let five = concat!(
        "> Foo is the first title line. Still title.\n",
        ">     > Bar is the second title line.\n",
        ">     > =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let five_regions = MarkdownParser.parse(five);
    assert!(
        five_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "five spaces after > must stay quote Prose, got: {five_regions:?}"
    );
    assert!(
        five_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Bar is the second")
        )),
        "five-space leftover > is not a nested opener, got: {five_regions:?}"
    );
    let five_out = format_text(five, &md_cfg()).unwrap();
    assert!(
        five_out.contains("first title line.\n"),
        "five-space Foo must still split, got:\n{five_out}"
    );
    assert!(
        five_out.contains("Body after setext.\nSecond body."),
        "five-space body Prose must still split, got:\n{five_out}"
    );
}

/// snapper-48gu / GitHub #263: four spaces is not a `>` marker (CM 5.1)
/// and indented code cannot interrupt a paragraph (4.4). `    > - Bar`
/// is title text. Three-space `   > - Bar` is a real quoted list opener
/// and must still interrupt.
#[test]
fn four_space_quoted_list_lookalike_stays_quote_setext_title() {
    let input = concat!(
        "> Foo is the first title line. Still title.\n",
        "    > - Bar is the second title line.\n",
        "> =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Still title")
                    || p.contains("Foo is the first")
                    || p.contains("Bar is the second")
        )),
        "4-space > - lookalike must stay quote setext, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Foo is the first title line.")
        )),
        "Foo must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bar is the second title line.")
        )),
        "Bar lookalike must be Structure, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        !out.contains("first title line.\n"),
        "must not sentence-split the quote setext, got:\n{out}"
    );
    assert!(
        out.contains("Body after setext.\nSecond body."),
        "body Prose must still split, got:\n{out}"
    );

    let tab = concat!(
        "> Foo is the first title line. Still title.\n",
        "\t> - Bar is the second title line.\n",
        "> =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let tab_regions = MarkdownParser.parse(tab);
    assert!(
        !tab_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Still title")
                    || p.contains("Foo is the first")
                    || p.contains("Bar is the second")
        )),
        "tab > - lookalike must stay quote setext, got: {tab_regions:?}"
    );

    let three = concat!(
        "> Foo is the first title line. Still title.\n",
        "   > - Bar is the second title line.\n",
        "   >   =======\n",
    );
    let three_regions = MarkdownParser.parse(three);
    assert!(
        three_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "3-space > - must interrupt, got: {three_regions:?}"
    );
    assert!(
        three_regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Bar is the second title line.")
        )),
        "3-space quoted list must be the setext title, got: {three_regions:?}"
    );
}

/// snapper-do12 / GitHub #262: matching-depth `>  >` setext. Both title
/// lines plus the underline are Structure; body still splits. Same for
/// one-line and three-space forms. `>>` / `> >` still hold.
#[test]
fn matching_two_space_nested_quote_setext_promotes_both_title_lines() {
    for inner in [">  >", ">   >", ">>", "> >"] {
        let input = format!(
            concat!(
                "{inner} Foo is the first title line. Still title.\n",
                "{inner} Bar is the second title line.\n",
                "{inner} =======\n",
                "\n",
                "Body after setext. Second body.\n",
            ),
            inner = inner
        );
        let regions = MarkdownParser.parse(&input);
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p)
                    if p.contains("Still title")
                        || p.contains("Foo is the first")
                        || p.contains("Bar is the second")
            )),
            "{inner:?} matching nested setext must not leave title Prose, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Foo is the first title line.")
            )),
            "{inner:?} Foo must be Structure, got: {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Bar is the second title line.")
            )),
            "{inner:?} Bar must be Structure, got: {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert!(
            !out.contains("first title line.\n"),
            "{inner:?} must not sentence-split the nested quote setext, got:\n{out}"
        );
        assert!(
            out.contains("Body after setext.\nSecond body."),
            "{inner:?} body Prose must still split, got:\n{out}"
        );
    }

    let one = concat!(
        ">  > Foo is the first title line. Still title.\n",
        ">  > =======\n",
        "\n",
        "Body after setext. Second body.\n",
    );
    let one_regions = MarkdownParser.parse(one);
    assert!(
        !one_regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Foo is the first")
        )),
        "one-line >  > setext must not leave Foo Prose, got: {one_regions:?}"
    );
}
