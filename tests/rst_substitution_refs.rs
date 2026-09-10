use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// RST Inliner substitution_ref must not be stolen as a table row.
/// Docutils line_block is `| ` / `|` at EOL; grid_table_top is `+---+`.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn start_fixture() -> &'static str {
    "|version| is the current release. Second sentence.\n"
}

fn mid_fixture() -> &'static str {
    concat!(
        "The current release is\n",
        "|version|. Next sentence.\n",
    )
}

#[test]
fn substitution_ref_start_is_prose_not_structure() {
    let regions = RstParser.parse(start_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("|version| is the current release.")
                    && s.contains("Second sentence.")
        )),
        "start |version| line must be Prose that still splits, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("|version|") || s.contains("current release")
        )),
        "start |version| must not be leftover Structure, got {regions:?}"
    );
}

#[test]
fn substitution_ref_mid_paragraph_is_prose_not_structure() {
    let regions = RstParser.parse(mid_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("|version|") && s.contains("Next sentence.")
        )),
        "mid-paragraph |version| must stay Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("|version|") || s.contains("Next sentence")
        )),
        "mid-paragraph |version| must not be leftover Structure, got {regions:?}"
    );
}

#[test]
fn substitution_ref_start_splits_and_keeps_ref_in_sentence() {
    let out = format_text(start_fixture(), &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "|version| is the current release.\n",
            "Second sentence.\n",
        ),
        "start |version| must split as Prose, got:\n{out}"
    );
    let twice = format_text(&out, &rst_cfg()).unwrap();
    assert_eq!(
        out, twice,
        "split substitution_ref must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, start_fixture(), &out),
        "oracle mismatch\n in={:?}\n out={out:?}",
        start_fixture()
    );
}

#[test]
fn substitution_ref_mid_paragraph_splits_and_keeps_ref_in_sentence() {
    let out = format_text(mid_fixture(), &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "The current release is |version|.\n",
            "Next sentence.\n",
        ),
        "mid-paragraph |version| must stay in the sentence and split, got:\n{out}"
    );
    let twice = format_text(&out, &rst_cfg()).unwrap();
    assert_eq!(
        out, twice,
        "split mid-paragraph substitution_ref must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, mid_fixture(), &out),
        "oracle mismatch\n in={:?}\n out={out:?}",
        mid_fixture()
    );
}

#[test]
fn grid_table_still_full_line_structure() {
    let input = concat!("+---+---+\n", "| a | b |\n", "+---+---+\n",);
    let regions = RstParser.parse(input);
    for needle in ["+---+---+", "| a | b |"] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "grid line {needle:?} must stay Structure, got {regions:?}"
        );
    }
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, input, "grid table must stay identity, got:\n{out}");
}
