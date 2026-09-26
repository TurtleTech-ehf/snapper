use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #93 / snapper-lhat: RST anonymous hyperlink targets must not reflow.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    concat!(
        "__ https://www.python.org/some/very/long/path\n",
        "\n",
        "See the target. Next sentence.\n",
    )
}

#[test]
fn anonymous_target_is_structure_not_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("__ https://www.python.org/some/very/long/path")
        )),
        "anonymous target line must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("__") || s.contains("python.org")
        )),
        "anonymous target must not be Prose, got {regions:?}"
    );
}

#[test]
fn anonymous_target_fixture_is_identity_under_format() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.lines()
            .any(|l| l == "__ https://www.python.org/some/very/long/path"),
        "URI must stay one line, got:\n{out}"
    );
    assert_eq!(
        out,
        concat!(
            "__ https://www.python.org/some/very/long/path\n",
            "\n",
            "See the target.\n",
            "Next sentence.\n",
        ),
        "target stays; following prose still splits, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung fixture must be identity, got:\n{out}"
    );
}

#[test]
fn wrap_does_not_break_anonymous_target_uri() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 20,
        ..Default::default()
    }
    .without_safety_backstops();
    let out = format_text(ticket_fixture(), &cfg).unwrap();
    assert!(
        out.lines()
            .any(|l| l == "__ https://www.python.org/some/very/long/path"),
        "narrow wrap must not break the URI, got:\n{out}"
    );
    assert!(
        !out.contains("__\nhttps://"),
        "must not split `__` onto its own line, got:\n{out}"
    );
}
