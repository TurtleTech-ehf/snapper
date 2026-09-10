//! GitHub #215 / snapper-e8g6: CommonMark 0.31.2 §6.3 full/collapsed
//! reference links must stay one inline token. Interior punctuation
//! is not a sentence boundary. The use stays Prose; the LRD stays
//! Structure.

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

fn ticket_fixture() -> &'static str {
    concat!(
        "See [the Fourier. transform][wiki] for details. Next sentence.\n",
        "\n",
        "[wiki]: https://example.org/fourier\n",
    )
}

#[test]
fn ticket_lrd_is_structure_not_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[wiki]: https://example.org/fourier")
        )),
        "LRD must be Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("[wiki]:"))),
        "LRD must not be Prose, got {regions:?}"
    );
}

#[test]
fn ticket_reference_use_stays_prose() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("[the Fourier. transform][wiki]")
        )),
        "reference use must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_reference_span_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("[the Fourier. transform][wiki]"),
        "reference span must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("[the Fourier.\n") && !out.contains("[the Fourier.\ntransform]"),
        "must not split inside the reference text, got:\n{out}"
    );
    assert!(
        out.contains("for details.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("for details. Next sentence."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert!(
        out.contains("[wiki]: https://example.org/fourier"),
        "LRD must stay its own Structure line, got:\n{out}"
    );
    assert!(
        !out.contains("Next sentence. [wiki]:"),
        "must not glue dest onto the previous sentence, got:\n{out}"
    );
    assert!(
        out.contains("Next sentence.\n\n[wiki]: https://example.org/fourier"),
        "blank before the LRD must stay, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn collapsed_reference_link_stays_one_span() {
    let input = concat!(
        "See [the Fourier. transform][] for details. Next sentence.\n",
        "\n",
        "[the Fourier. transform]: https://example.org/fourier\n",
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("[the Fourier. transform][]"),
        "collapsed span must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("for details.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
