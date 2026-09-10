//! GitHub #215 / snapper-e8g6: CommonMark 0.31.2 §6.3 reference links.
//! INLINE_TOKEN_RE already protects `[text](url)` not `[text][ref]`.
//! The use stays Prose; the LRD stays Structure.

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
fn ticket_fixture_reference_link_stays_one_span() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "See [the Fourier. transform][wiki] for details.\n",
            "Next sentence.\n",
            "\n",
            "[wiki]: https://example.org/fourier\n",
        ),
        "reference span must stay one token and Next sentence. must split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn ticket_fixture_lrd_stays_structure() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[wiki]: https://example.org/fourier")
        )),
        "LRD must be Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("[wiki]:")
        )),
        "LRD must not be Prose, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("[the Fourier. transform][wiki]")
        )),
        "the use must stay Prose, got: {regions:?}"
    );
}

#[test]
fn ticket_fixture_does_not_reflow_lrd() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("[wiki]: https://example.org/fourier"),
        "LRD dest must stay its own line, got:\n{out}"
    );
    assert!(
        !out.contains("Next sentence. [wiki]:") && !out.contains("details. [wiki]:"),
        "must not glue LRD onto the previous sentence, got:\n{out}"
    );
    assert_eq!(
        out,
        concat!(
            "See [the Fourier. transform][wiki] for details.\n",
            "Next sentence.\n",
            "\n",
            "[wiki]: https://example.org/fourier\n",
        ),
        "ticket fixture shape, got:\n{out}"
    );

    let guarded = FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle must accept the reflow\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn collapsed_and_shortcut_reference_links_stay_one_span() {
    let collapsed = concat!(
        "See [the Fourier. transform][] for details. Next sentence.\n",
        "\n",
        "[the Fourier. transform]: https://example.org/fourier\n",
    );
    let out = format_text(collapsed, &md_cfg()).unwrap();
    assert!(
        out.contains("See [the Fourier. transform][] for details.\nNext sentence."),
        "collapsed span must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("[the Fourier. transform]: https://example.org/fourier"),
        "collapsed LRD must stay Structure, got:\n{out}"
    );

    let shortcut = concat!(
        "See [the Fourier. transform] for details. Next sentence.\n",
        "\n",
        "[the Fourier. transform]: https://example.org/fourier\n",
    );
    let out = format_text(shortcut, &md_cfg()).unwrap();
    assert!(
        out.contains("See [the Fourier. transform] for details.\nNext sentence."),
        "shortcut span must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("[the Fourier. transform]: https://example.org/fourier"),
        "shortcut LRD must stay Structure, got:\n{out}"
    );
}
