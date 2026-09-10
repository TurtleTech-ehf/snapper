//! GitHub #205 / snapper-4zt5: GFM alert type markers stay Structure.
//! `> [!NOTE]` is not quote Prose. The body hangs and splits. After the
//! alert still reflows.

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

/// Ticket fixture (Format::Markdown).
fn ticket_fixture() -> &'static str {
    concat!(
        "> [!NOTE]\n",
        "> This is a long alert sentence that must reflow. Second sentence.\n",
        "\n",
        "After the alert. Next.\n",
    )
}

#[test]
fn note_alert_type_marker_is_structure() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[!NOTE]")
        )),
        "[!NOTE] must stay Structure, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("[!NOTE]")
        )),
        "[!NOTE] must not join the body as Prose, got: {regions:?}"
    );
}

#[test]
fn note_alert_body_hangs_and_following_prose_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "> [!NOTE]\n",
            "> This is a long alert sentence that must reflow.\n",
            "> Second sentence.\n",
            "\n",
            "After the alert.\n",
            "Next.\n",
        ),
        "NOTE stays; body hangs and splits; following prose reflows, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn note_alert_survives_safety_backstops() {
    let input = ticket_fixture();
    let cfg = FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    };
    let out = format_text(input, &cfg).unwrap();
    assert!(
        out.contains("> This is a long alert sentence that must reflow.\n> Second sentence."),
        "CLI backstops must not revert the split, got:\n{out}"
    );
    assert!(
        out.contains("After the alert.\nNext."),
        "following prose must still split under backstops, got:\n{out}"
    );
    assert!(
        !out.contains("[!NOTE] This is a long"),
        "type marker must not join the body, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle must accept the reflow\n in={input:?}\n out={out:?}"
    );
    assert_eq!(format_text(&out, &cfg).unwrap(), out);
}

#[test]
fn leftover_alert_types_reflow_like_note() {
    for name in ["TIP", "WARNING", "CAUTION", "IMPORTANT"] {
        let input = format!(
            "> [!{name}]\n> This is a long alert sentence that must reflow. Second sentence.\n\nAfter the alert. Next.\n"
        );
        let regions = MarkdownParser.parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(&format!("[!{name}]"))
            )),
            "[!{name}] must stay Structure, got: {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains(&format!("[!{name}]"))
            )),
            "[!{name}] must not join the body as Prose, got: {regions:?}"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert_eq!(
            out,
            format!(
                "> [!{name}]\n> This is a long alert sentence that must reflow.\n> Second sentence.\n\nAfter the alert.\nNext.\n"
            ),
            "{name} is the same alert class as NOTE, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}
