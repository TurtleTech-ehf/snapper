//! Docutils compound bibliographic fields nested-parse as leftover body.

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
fn leftover_rfc2822_author_at_bos_hangs_and_splits() {
    let input = concat!("Author: fig. 1 is here. After.\n", "After. Next.\n",);
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("Author:"))),
        "RFC2822 Author: must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("fig. 1 is here.") && p.contains("After.")
        )),
        "RFC2822 value must be leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        !out.contains("Author: fig. 1 is here. After."),
        "RFC2822 value must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn leftover_dedication_same_line_hangs_and_splits() {
    let input = concat!(
        ":Dedication: fig. 1 is here. After.\n",
        "After. Next.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        !out.contains(":Dedication: fig. 1 is here. After."),
        "Dedication value must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
}

#[test]
fn leftover_abstract_same_line_hangs_and_splits() {
    let input = concat!(
        ":Abstract: fig. 1 is here. After.\n",
        "After. Next.\n",
    );
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(":Abstract:"))),
        "Abstract marker must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("fig. 1 is here.") && p.contains("After.")
        )),
        "Abstract value must be leftover Prose, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        !out.contains(":Abstract: fig. 1 is here. After."),
        "Abstract value must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}
