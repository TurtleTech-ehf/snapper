//! GitHub #327 / snapper-owq3: CommonMark 0.31.2 sec 4.2 ATX headings
//! are 1–6 `#` then space/tab or EOL. `HEADING_RE` required `\s+`, so a
//! lone `#` / `##` joined the next paragraph.

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

/// Ticket fixture (Format::Markdown / GitHub #327).
fn ticket_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "#\n",
        "After heading. Next sentence.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "#\n",
        "After heading.\n",
        "Next sentence.\n",
    )
}

fn wrapped(marker: &str) -> String {
    format!(
        "Intro sentence here. Another intro sentence.\n{marker}\nAfter heading. Next sentence.\n"
    )
}

fn expected_wrapped(marker: &str) -> String {
    format!(
        "Intro sentence here.\nAnother intro sentence.\n{marker}\nAfter heading.\nNext sentence.\n"
    )
}

/// Probe fixture from the ticket body (blank before the empty ATX).
fn ticket_probe_fixture() -> &'static str {
    concat!(
        "Intro sentence here. Another intro sentence.\n",
        "\n",
        "#\n",
        "After heading. Next sentence.\n",
    )
}

fn expected_ticket_probe() -> &'static str {
    concat!(
        "Intro sentence here.\n",
        "Another intro sentence.\n",
        "\n",
        "#\n",
        "After heading.\n",
        "Next sentence.\n",
    )
}

#[test]
fn empty_hash_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "#\n" || s == "#")),
        "lone # at EOL must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains('#') && p.contains("After heading.")
        )),
        "empty # must not join the following prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After heading.") && p.contains("Next sentence.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_empty_hash_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn empty_atx_levels_one_to_six_are_structure() {
    for n in 1..=6 {
        let marks = "#".repeat(n);
        let input = wrapped(&marks);
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.trim() == marks
            )),
            "lone {marks} at EOL must be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains(&marks) && p.contains("After heading.")
            )),
            "empty {marks} must not join the following prose, got {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert_eq!(out, expected_wrapped(&marks), "level {n} got:\n{out}");
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}

#[test]
fn titled_atx_with_space_stays_structure() {
    let input = "# Title\n\nBody sentence one. Body sentence two.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "# Title\n")),
        "# Title with a following space must stay Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("Title"))),
        "titled ATX must not become Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.starts_with("# Title\n"),
        "titled ATX must stay one Structure line, got:\n{out}"
    );
    assert!(
        out.contains("Body sentence one.\nBody sentence two."),
        "body Prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn four_space_indented_hash_is_code() {
    let four = "    #\n";
    let regions = MarkdownParser.parse(four);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Code { body, .. } if body.contains('#'))),
        "four-space # is indented code, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains('#'))),
        "four-space # must not be an ATX heading, got: {regions:?}"
    );
    let out = format_text(four, &md_cfg()).unwrap();
    assert!(
        out.contains("    #"),
        "four-space # must stay indented code, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn ticket_probe_blank_before_empty_hash() {
    let input = ticket_probe_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "#")),
        "lone # after a blank must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains('#') && p.contains("After heading.")
        )),
        "empty # must not join After heading., got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, expected_ticket_probe(), "got:\n{out}");
    assert!(
        !out.contains("# After heading."),
        "must not glue # onto the next sentence, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn hash_without_space_is_not_atx() {
    let input = "#foo\nAfter heading. Next sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("#foo"))),
        "#foo is prose (CM ex. 69–70), got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("#foo"))),
        "#foo must stay Prose, got {regions:?}"
    );
}

#[test]
fn hash_then_tab_is_atx() {
    let input = "#\tTitle\n\nAfter heading. Next sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("#\tTitle"))),
        "hash then tab is ATX (CM 4.2), got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("Title"))),
        "tab-separated title must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("#\tTitle"),
        "tab ATX must stay Structure, got:\n{out}"
    );
    assert!(
        out.contains("After heading.\nNext sentence."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn three_space_empty_atx_is_structure() {
    let input = "   #\n\nAfter heading. Next sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "#")),
        "0–3 space empty ATX must be Structure, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.starts_with("   #\n"),
        "indented empty ATX must stay, got:\n{out}"
    );
    assert!(
        out.contains("After heading.\nNext sentence."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
