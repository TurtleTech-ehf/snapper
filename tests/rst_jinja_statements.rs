//! snapper-79b2 / GitHub #196: consecutive Jinja statements stay two lines.
//! sphinx-jinja / Jinja2 `{% ... %}` block delimiters, not a pandoc parse.

use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// GitHub #196 fixture (Format::Rst).
fn ticket_fixture() -> &'static str {
    include_str!("fixtures/rst_jinja_statements.rst")
}

#[test]
fn jinja_statements_are_structure_not_prose() {
    let regions = RstParser.parse(ticket_fixture());
    for needle in [r#"{% set foo = "foo" %}"#, r#"{% set bar = "bar" %}"#] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "Jinja statement {needle:?} must be Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains(needle))),
            "Jinja statement {needle:?} must not be Prose, got {regions:?}"
        );
    }
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("set foo") && s.contains("set bar")
        )),
        "consecutive Jinja statements must not join into one Prose, got {regions:?}"
    );
}

#[test]
fn following_code_block_stays_structure() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| match r {
            Region::Code { header, .. } => header.contains(".. code-block:: python"),
            Region::Structure(s) => s.contains(".. code-block:: python"),
            _ => false,
        }),
        "following .. code-block:: must stay Structure/Code, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(".. code-block::") || s.contains("pass")
        )),
        "following .. code-block:: must not be Prose, got {regions:?}"
    );
}

#[test]
fn consecutive_jinja_statements_stay_two_lines() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    let joined = r#"{% set foo = "foo" %} {% set bar = "bar" %}"#;
    assert!(
        !out.contains(joined),
        "must not join consecutive Jinja statements, got:\n{out}"
    );
    let foo_lines = out
        .lines()
        .filter(|l| l.contains(r#"{% set foo = "foo" %}"#))
        .count();
    let bar_lines = out
        .lines()
        .filter(|l| l.contains(r#"{% set bar = "bar" %}"#))
        .count();
    assert_eq!(
        foo_lines, 1,
        "foo statement must stay one line, got:\n{out}"
    );
    assert_eq!(
        bar_lines, 1,
        "bar statement must stay one line, got:\n{out}"
    );
    assert_eq!(
        out, input,
        "Jinja statements and following code-block must stay identity, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung Jinja fixture must be identity, got:\n{out}"
    );
}

#[test]
fn flush_left_jinja_statements_stay_two_lines() {
    let input = concat!("{% set foo = \"foo\" %}\n", "{% set bar = \"bar\" %}\n",);
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "flush-left Jinja statements must stay two lines, got:\n{out}"
    );
}
