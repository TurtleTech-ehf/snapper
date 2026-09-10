//! snapper-79b2 / GitHub #196: RST Jinja statements stay unjoined.
//! Two consecutive `{% set ... %}` lines must stay two lines; the
//! following `.. code-block::` stays a Code region (not Prose).

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

/// Reporter fixture (GitHub #196 / sphinx-jinja RST).
fn ticket_fixture() -> &'static str {
    concat!(
        "         {% set foo = \"foo\" %}\n",
        "         {% set bar = \"bar\" %}\n",
        "\n",
        "         .. code-block:: python\n",
        "\n",
        "            pass\n",
    )
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
}

#[test]
fn following_code_block_stays_code_not_prose() {
    let regions = RstParser.parse(ticket_fixture());
    let code = regions.iter().find_map(|r| match r {
        Region::Code {
            lang, header, body, ..
        } => Some((lang.clone(), header.clone(), body.clone())),
        _ => None,
    });
    let (lang, header, body) = code.expect("expected one Region::Code");
    assert_eq!(lang.as_deref(), Some("python"));
    assert!(
        header.contains(".. code-block:: python"),
        "code-block header must stay on the Code region, got {regions:?}"
    );
    assert!(
        body.contains("pass"),
        "code-block body must stay on the Code region, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("code-block") || s.contains("pass")
        )),
        "following code-block must not be Prose, got {regions:?}"
    );
}

#[test]
fn consecutive_jinja_statements_stay_two_lines() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "consecutive Jinja statements must stay unjoined; code-block must stay, got:\n{out}"
    );
    assert!(
        out.contains("{% set foo = \"foo\" %}\n         {% set bar = \"bar\" %}\n"),
        "the two set lines must remain two lines, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung Jinja fixture must be identity, got:\n{out}"
    );
}

/// Flush `{% ... %}` lines (no indent) are also statements, not prose.
#[test]
fn flush_jinja_statements_stay_unjoined() {
    let input = concat!(
        "{% set foo = \"foo\" %}\n",
        "{% set bar = \"bar\" %}\n",
        "\n",
        "After. More after.\n",
    );
    let regions = RstParser.parse(input);
    for needle in [r#"{% set foo = "foo" %}"#, r#"{% set bar = "bar" %}"#] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "flush Jinja statement {needle:?} must be Structure, got {regions:?}"
        );
    }
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("{% set foo = \"foo\" %}\n{% set bar = \"bar\" %}\n"),
        "flush Jinja statements must stay two lines, got:\n{out}"
    );
    assert!(
        out.contains("After.\nMore after."),
        "prose after flush Jinja must still reflow, got:\n{out}"
    );
}
