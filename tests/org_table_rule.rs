//! GitHub #310 / snapper-oac5: Org table-rule lines stay unjoined.
//! Emacs 30.2 `org-element-paragraph-separate` matches `+` followed by
//! one or more `-+` groups (`+---+`). That line ends a paragraph.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Org).
fn table_rule_fixture() -> &'static str {
    concat!(
        "+-----+\n",
        "First line. Second line.\n",
        "+-----+\n",
        "After the block. Next.\n",
    )
}

#[test]
fn table_rule_lines_are_structure() {
    let regions = OrgParser.parse(table_rule_fixture());
    assert_eq!(
        regions
            .iter()
            .filter(|r| matches!(r, Region::Structure(s) if s.trim() == "+-----+"))
            .count(),
        2,
        "each +---+ line must be Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("+-----+"))),
        "table-rule must not join following prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_rules_and_splits_flanking_prose() {
    let input = table_rule_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "+-----+\n",
            "First line.\n",
            "Second line.\n",
            "+-----+\n",
            "After the block.\n",
            "Next.\n",
        ),
        "each +---+ stays unjoined; First line. / Second line. and After the block. / Next. still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    assert!(
        snapper_fmt::oracle::matches(Format::Org, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn pipe_org_tables_and_horizontal_rules_unchanged() {
    let input = concat!(
        "| Name | Age |\n",
        "|------+-----|\n",
        "| Alice | 30 |\n",
        "-----\n",
        "After the rule. Next.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "| Name | Age |\n",
            "|------+-----|\n",
            "| Alice | 30 |\n",
            "-----\n",
            "After the rule.\n",
            "Next.\n",
        ),
        "pipe tables and ----- stay; following prose still splits, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn indented_and_multi_cell_table_rules_are_structure() {
    let input = concat!(
        "  +---+---+\n",
        "Between. More.\n",
        "+-+\n",
        "After. Next.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "  +---+---+\n",
            "Between.\n",
            "More.\n",
            "+-+\n",
            "After.\n",
            "Next.\n",
        ),
        "indented / short table-rules stay unjoined, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn incomplete_plus_dash_line_is_still_prose() {
    let input = "+-----\nFirst line. Second line.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out, "+----- First line.\nSecond line.\n",
        "+----- without a closing + stays Prose and joins the next line, got:\n{out}"
    );
}
