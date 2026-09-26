//! Title arguments of admonition, rubric, topic, sidebar, list-table,
//! and contents stay on the directive line. Body prose still reflows.
//! `note` / `warning` same-line text is a body paragraph and still splits.

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

#[test]
fn title_argument_stays_on_directive_line_body_reflows() {
    for name in [
        "admonition",
        "rubric",
        "topic",
        "sidebar",
        "list-table",
        "contents",
    ] {
        let input = format!(
            ".. {name}:: First title. Second title.\n\n   Body one. Body two.\n\nAfter. Next.\n"
        );
        let regions = RstParser.parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.contains(&format!(".. {name}:: First title. Second title."))
            )),
            "{name} title must stay Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("First title.")
            )),
            "{name} title must not be Prose, got {regions:?}"
        );
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert!(
            out.contains(&format!(".. {name}:: First title. Second title.")),
            "{name} title stays on the directive line, got:\n{out}"
        );
        assert!(
            !out.contains("First title.\n"),
            "{name} title must not split, got:\n{out}"
        );
        assert!(
            out.contains("   Body one.\n   Body two."),
            "{name} body must still reflow, got:\n{out}"
        );
        assert!(
            out.contains("After.\nNext."),
            "prose after {name} must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
}

#[test]
fn note_and_warning_same_line_body_still_reflows() {
    for name in ["note", "warning"] {
        let input = format!(".. {name}:: Body one. Body two.\n");
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert!(
            out.contains(&format!(".. {name}:: Body one.\n")),
            "{name} same-line body must still split, got:\n{out}"
        );
        assert!(
            out.contains("Body two."),
            "{name} second sentence must remain, got:\n{out}"
        );
        assert!(
            !out.contains(&format!(".. {name}:: Body one. Body two.")),
            "{name} same-line body must not stay one line, got:\n{out}"
        );
    }
}
