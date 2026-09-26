//! An indented `csv-table` / `table` title continuation stays Structure
//! until a blank line or a table border. Same-line title text still
//! splits. The table body stays opaque. Prose after the table splits.

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

fn csv_title_continuation() -> &'static str {
    concat!(
        ".. csv-table:: fig. 1 is here. After.\n",
        "   Still title. More.\n",
        "\n",
        "   \"a. b\", \"c. d\"\n",
        "After. Next.\n",
    )
}

fn table_title_to_border() -> &'static str {
    concat!(
        ".. table:: fig. 1 is here. After.\n",
        "   Still title. More.\n",
        "   =====  =====\n",
        "   A. B   C. D\n",
        "   =====  =====\n",
        "After. Next.\n",
    )
}

#[test]
fn csv_table_title_continuation_stays_structure() {
    let input = csv_title_continuation();
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Still title. More.")
        )),
        "csv-table title continuation must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title")
        )),
        "csv-table title continuation must not be Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("fig. 1 is here.") && p.contains("After.")
        )),
        "same-line csv-table title must still be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("a. b") || p.contains("c. d")
        )),
        "csv-table body must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After.") && p.contains("Next.")
        )),
        "prose after the table must still be Prose, got {regions:?}"
    );

    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("   Still title. More.\n"),
        "title continuation must not sentence-split, got:\n{out}"
    );
    assert!(
        !out.contains(".. csv-table:: fig. 1 is here. After."),
        "same-line title must still split, got:\n{out}"
    );
    assert!(
        out.contains("\"a. b\"") && !out.contains("a.\n"),
        "csv-table body must stay opaque, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after the table must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn table_title_continuation_stays_structure_until_border() {
    let input = table_title_to_border();
    let regions = RstParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Still title. More.")
        )),
        "table title continuation must be Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("A. B") || p.contains("C. D"))),
        "table body must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert!(
        out.contains("   Still title. More.\n"),
        "title continuation must not sentence-split, got:\n{out}"
    );
    assert!(
        out.contains("A. B") && !out.contains("A.\n"),
        "table body must stay opaque, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "prose after the table must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}
