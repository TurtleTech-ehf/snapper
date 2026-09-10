//! GitHub #205 / snapper-4zt5: GFM alert type markers stay Structure.
//! `> [!NOTE]` plus the next `>` line must not join as one quote Prose.
//! Body hangs and splits; following prose still reflows.

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
fn alert_note_fixture() -> &'static str {
    concat!(
        "> [!NOTE]\n",
        "> This is a long alert sentence that must reflow. Second sentence.\n",
        "\n",
        "After the alert. Next.\n",
    )
}

#[test]
fn note_alert_type_stays_structure_body_hangs() {
    let input = alert_note_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("[!NOTE]"))),
        "[!NOTE] must stay Structure, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("[!NOTE]"))),
        "[!NOTE] must not join quote Prose, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("This is a long alert sentence that must reflow.")
                    && p.contains("Second sentence.")
        )),
        "alert body must be hung Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a long alert sentence")
        )),
        "alert body must not freeze as Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "> ")),
        "alert body hang `>` must be Structure, got: {regions:?}"
    );

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
        "alert type stays; body hangs and splits; following prose reflows, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn leftover_alert_types_reflow_like_note() {
    for name in ["TIP", "WARNING", "CAUTION", "IMPORTANT"] {
        let input = format!(
            "> [!{name}]\n> This is a long alert sentence that must reflow. Second sentence.\n\nAfter the alert. Next.\n"
        );
        let out = format_text(&input, &md_cfg()).unwrap();
        assert_eq!(
            out,
            format!(
                "> [!{name}]\n> This is a long alert sentence that must reflow.\n> Second sentence.\n\nAfter the alert.\nNext.\n"
            ),
            "{name} is the same GFM alert class as NOTE, got:\n{out}"
        );
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
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
            "[!{name}] must not join quote Prose, got: {regions:?}"
        );
    }
}
