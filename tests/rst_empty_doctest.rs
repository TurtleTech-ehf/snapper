//! GitHub #331 / snapper-mpww: Docutils `Body.patterns` doctest is
//! `>>>( +|$)`. A lone `>>>` at EOL is Structure; following prose still
//! splits. Same-line `>>> print(1.2)` still takes the doctest block.

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

/// Ticket fixture (Format::Rst / GitHub #331).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        ">>>\n",
        "After empty. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        ">>>\n",
        "After empty.\n",
        "Next sentence.\n",
    )
}

fn wrapped(opener: &str) -> String {
    format!("Intro sentence here. Another intro sentence.\n{opener}\nAfter empty. Next sentence.\n")
}

fn expected_wrapped(opener: &str) -> String {
    format!(
        "Intro sentence here.\nAnother intro sentence.\n{opener}\nAfter empty.\nNext sentence.\n"
    )
}

#[test]
fn empty_doctest_is_structure() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ">>>" || s == ">>>\n")),
        "lone >>> at EOL must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(">>>") && s.contains("After empty.")
        )),
        "empty >>> must not swallow following prose as Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After empty.") && p.contains("Next sentence.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_empty_doctest_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn trailing_space_empty_doctest_is_structure() {
    let input = wrapped(">>> ");
    let regions = RstParser.parse(&input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == ">>>")),
        ">>> plus trailing space at EOL must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("After empty.")
        )),
        ">>>  must not swallow following prose, got {regions:?}"
    );
    let out = format_text(&input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_wrapped(">>> "), "got:\n{out}");
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn same_line_doctest_command_unchanged() {
    let input = ">>> print(1.2)\n1.2\n";
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains(">>> print(1.2)") && s.contains("1.2")
        )),
        ">>> print(1.2) must stay one Structure block, got {regions:?}"
    );
    assert!(
        !regions.iter().any(
            |r| matches!(r, Region::Prose(p) if p.contains("print(1.2)") || p.contains("1.2"))
        ),
        "same-line doctest must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        ">>> print(1.2) must stay unchanged, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn same_line_doctest_still_swallows_following_output() {
    let input = concat!(
        "Intro sentence here. Another intro sentence.\n",
        ">>> print(1.2)\n",
        "After empty. Next sentence.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains(">>> print(1.2)") && s.contains("After empty. Next sentence.")
        )),
        ">>> print(1.2) must still take the following output line, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After empty.")
        )),
        "command doctest output must not become Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("Intro sentence here.\nAnother intro sentence.\n"),
        "leading prose must still split, got:\n{out}"
    );
    assert!(
        out.contains(">>> print(1.2)\nAfter empty. Next sentence.\n"),
        "same-line doctest block must stay unjoined, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}
