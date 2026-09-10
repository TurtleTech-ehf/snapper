use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{format_text, FormatConfig};

/// GitHub #93 / snapper-lhat: RST anonymous hyperlink targets must not
/// reflow as prose. Fails on origin/main (`__ url` is Prose; wrap splits
/// the URI off the `__` line) then passes.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
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
            Region::Prose(s) if s.contains("__") || s.contains("https://www.python.org")
        )),
        "anonymous target line must not be Prose, got {regions:?}"
    );
}

#[test]
fn anonymous_target_is_identity_under_format() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("__ https://www.python.org/some/very/long/path\n"),
        "anonymous target URI must stay on the __ line, got:\n{out}"
    );
    assert!(
        out.contains("See the target.\nNext sentence.\n"),
        "surrounding prose must still reflow, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "anonymous target identity must survive a second pass"
    );
}

#[test]
fn wrap_does_not_split_anonymous_target_uri() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 40,
        ..Default::default()
    };
    let input = ticket_fixture();
    let out = format_text(input, &cfg).unwrap();
    assert!(
        out.contains("__ https://www.python.org/some/very/long/path\n"),
        "wrap must keep __ and the URI on one line, got:\n{out}"
    );
    assert!(
        !out.contains("__\nhttps://"),
        "wrap must not break the anonymous target onto two lines, got:\n{out}"
    );
}
