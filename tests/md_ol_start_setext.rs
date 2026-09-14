//! Ordered start != 1 plus underline is a list item, not a setext heading.
//! CommonMark 4.3: a setext title must not be a list item. After blank/BOF,
//! `=======` is lazy item text and `---` is a thematic break.
//! `1. Foo` / `- Foo` already take the list path.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn equals_fixture() -> &'static str {
    concat!(
        "2. Foo is an item. Still item.\n",
        "=======\n",
        "\n",
        "After the list. Next.\n",
    )
}

fn dash_fixture() -> &'static str {
    concat!(
        "2. Foo is an item. Still item.\n",
        "---\n",
        "\n",
        "After the break. Next.\n",
    )
}

#[test]
fn start_not_one_equals_is_list_not_setext() {
    let regions = MarkdownParser.parse(equals_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "2. ")),
        "2. must be a list marker, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Foo is an item.") && p.contains("Still item.")
        )),
        "item text plus lazy ======= must stay Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Foo is an item.")
        )),
        "2. Foo must not be a setext title, got {regions:?}"
    );
}

#[test]
fn start_not_one_dash_is_list_then_thematic_break() {
    let regions = MarkdownParser.parse(dash_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "2. ")),
        "2. must be a list marker, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.trim() == "---"
        )),
        "--- after a list item must be a thematic break, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Foo is an item.") && !p.contains("---")
        )),
        "item prose must not swallow the break, got {regions:?}"
    );
}

#[test]
fn start_not_one_fixtures_reflow_and_do_not_regress_start_one() {
    let eq_out = format_text(equals_fixture(), &md_cfg()).unwrap();
    assert!(
        eq_out.contains("2. Foo is an item.\n   Still item."),
        "2. item must still split, got:\n{eq_out}"
    );
    assert!(
        !eq_out.contains("Foo is an item. Still item.\n======="),
        "must not keep a setext pair, got:\n{eq_out}"
    );
    assert!(
        eq_out.contains("After the list.\nNext."),
        "following prose must still split, got:\n{eq_out}"
    );
    assert_eq!(format_text(&eq_out, &md_cfg()).unwrap(), eq_out);

    let dash_out = format_text(dash_fixture(), &md_cfg()).unwrap();
    assert!(
        dash_out.contains("2. Foo is an item.\n   Still item."),
        "2. item before --- must still split, got:\n{dash_out}"
    );
    assert!(
        dash_out.contains("---\n"),
        "thematic break must stay, got:\n{dash_out}"
    );
    assert!(
        dash_out.contains("After the break.\nNext."),
        "prose after --- must still split, got:\n{dash_out}"
    );
    assert_eq!(format_text(&dash_out, &md_cfg()).unwrap(), dash_out);

    for opener in [
        "1. Foo is an item. Still item.\n=======\n",
        "- Foo is an item. Still item.\n=======\n",
    ] {
        let regions = MarkdownParser.parse(opener);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s == "1. " || s == "- "
            )),
            "start-1 / bullet must stay a list, got {regions:?} for {opener:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Foo is an item.")
            )),
            "start-1 / bullet must not become setext, got {regions:?} for {opener:?}"
        );
    }
}
