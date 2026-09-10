//! snapper-17lc / GitHub #175: RST footnotes and citations are not comments.
//! `.. [1]` is Structure; the body is hung Prose that still splits.

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

/// Ticket fixture: footnote reference paragraph plus `.. [1]` definition.
fn ticket_fixture() -> &'static str {
    concat!(
        "See [1]_. Next sentence.\n",
        "\n",
        ".. [1] Footnote text. Second sentence.\n",
    )
}

#[test]
fn footnote_opener_is_structure_body_is_hung_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ".. [1] ")),
        ".. [1] must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Footnote text.") && s.contains("Second sentence.")
        )),
        "footnote body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Footnote text")
        )),
        "footnote body must not stay Structure, got {regions:?}"
    );
}

#[test]
fn footnote_fixture_body_splits_and_hangs() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "See [1]_.\n",
            "Next sentence.\n",
            "\n",
            ".. [1] Footnote text.\n",
            "       Second sentence.\n",
        ),
        "footnote opener stays; body hangs and splits, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung footnote must be identity, got:\n{out}"
    );
}

#[test]
fn auto_symbol_and_citation_are_structure_plus_hung_prose() {
    for (marker, hang) in [
        (".. [#] ", "       "),
        (".. [*] ", "       "),
        (".. [CIT2002] ", "             "),
    ] {
        let input =
            format!("See the note. Next sentence.\n\n{marker}Footnote text. Second sentence.\n");
        let regions = RstParser.parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s == marker)),
            "{marker:?} must be Structure, got {regions:?}"
        );
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(s) if s.contains("Footnote text.") && s.contains("Second sentence.")
            )),
            "{marker:?} body must be Prose, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains("Footnote text")
            )),
            "{marker:?} body must not stay Structure, got {regions:?}"
        );
        let out = format_text(&input, &rst_cfg()).unwrap();
        let expected = format!(
            "See the note.\nNext sentence.\n\n{marker}Footnote text.\n{hang}Second sentence.\n"
        );
        assert_eq!(
            out, expected,
            "{marker:?} body must hang and split, got:\n{out}"
        );
        assert_eq!(
            format_text(&out, &rst_cfg()).unwrap(),
            out,
            "{marker:?} hung fixture must be identity, got:\n{out}"
        );
    }
}
