use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #100 / snapper-j0ig: Docutils section adornment is any
/// non-alphanumeric printable 7-bit ASCII (`pats.nonalphanum7bit`).
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        "Title\n",
        ":::::\n",
        "\n",
        "Next paragraph. Second sentence.\n",
    )
}

#[test]
fn colon_underline_is_structure_not_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("Title"))),
        "title must be Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":::::"))),
        "colon underline must be Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("Title") || s.contains(":::::"))),
        "colon section must not be Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Next paragraph.") && s.contains("Second sentence.")
        )),
        "following paragraph must stay Prose, got {regions:?}"
    );
}

#[test]
fn colon_section_fixture_is_identity_under_format() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, "Title\n:::::\n\nNext paragraph.\nSecond sentence.\n",
        "colon adornment stays; following prose still splits, got:\n{out}"
    );
    assert!(
        !out.contains("Title :::::"),
        "must not glue ::::: onto the title, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "colon section fixture must be identity, got:\n{out}"
    );

    // Single-token Title cannot expose a Prose title; two sentences must stay one line.
    let titled = "Title one. Title two.\n:::::\n\nNext paragraph. Second sentence.\n";
    let titled_out = format_text(titled, &rst_cfg()).unwrap();
    assert_eq!(
        titled_out, "Title one. Title two.\n:::::\n\nNext paragraph.\nSecond sentence.\n",
        "multi-sentence title above ::::: must stay one line, got:\n{titled_out}"
    );
}

#[test]
fn percent_and_at_adornments_are_structure() {
    for rule in ["%%%%%", "@@@@@"] {
        let input = format!("Title\n{rule}\n");
        let regions = RstParser.parse(&input);
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Title"))),
            "title above {rule} must be Structure, got {regions:?}"
        );
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(rule))),
            "{rule} must be Structure, got {regions:?}"
        );
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert_eq!(
            out, input,
            "{rule} adornment must stay two lines, got:\n{out}"
        );
        assert!(
            !out.contains(&format!("Title {rule}")),
            "must not glue {rule} onto the title, got:\n{out}"
        );
    }
}
