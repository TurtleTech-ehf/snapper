//! GitHub #309 / snapper-b3qs: a column-0 diary-sexp line is a
//! diary-sexp element, not a paragraph. `%%(` at BOL stays unjoined;
//! following prose still splits. CLOCK / DEADLINE / `<%%(...)>` stay.

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

/// Ticket fixture (Format::Org / GitHub #309).
fn ticket_fixture() -> &'static str {
    concat!(
        "%%(or (diary-date 9 11 2026) (eq 1. 2))\n",
        "After the block. Next.\n",
    )
}

#[test]
fn diary_sexp_line_is_structure() {
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains("%%(or (diary-date 9 11 2026) (eq 1. 2))")
        )),
        "column-0 diary-sexp must be Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("%%("))),
        "diary-sexp must not be Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After the block.") && p.contains("Next.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_diary_sexp_unjoined_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "%%(or (diary-date 9 11 2026) (eq 1. 2))\n",
            "After the block.\n",
            "Next.\n",
        ),
        "%%( line stays unjoined; After the block. / Next. still split, got:\n{out}"
    );
    assert!(
        !out.contains("(eq 1.\n") && !out.contains("1.\n 2)"),
        "must not split inside the diary-sexp, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Org,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn clock_deadline_and_inline_diary_timestamp_unchanged() {
    let input = concat!(
        "* TODO Task\n",
        "DEADLINE: <2026-01-01 Wed>\n",
        "CLOCK: [2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00] =>  1:00\n",
        "Meet at <%%(equal (calendar-day-of-week date) 1.)> then leave. Next sentence.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("* TODO Task\nDEADLINE: <2026-01-01 Wed>\nCLOCK:"),
        "DEADLINE: must stay its own line, got:\n{out}"
    );
    assert!(
        !out.contains("DEADLINE: <2026-01-01 Wed> CLOCK:"),
        "DEADLINE: must not glue onto CLOCK:, got:\n{out}"
    );
    assert!(
        out.contains("CLOCK: [2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00] =>  1:00\nMeet at "),
        "CLOCK: must stay its own line, got:\n{out}"
    );
    assert!(
        !out.contains("=>  1:00 Meet at"),
        "CLOCK: must not glue onto the following prose, got:\n{out}"
    );
    assert!(
        out.contains("<%%(equal (calendar-day-of-week date) 1.)>"),
        "inline <%%(...)> timestamp must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("date) 1.\n") && !out.contains("1.\n)>"),
        "must not split inside <%%(...)>, got:\n{out}"
    );
    assert!(
        out.contains("then leave.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn indented_diary_sexp_is_not_the_element() {
    let input = "  %%(or (diary-date 9 11 2026) (eq 1. 2))\nAfter the block. Next.\n";
    let regions = OrgParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("%%(")
        )),
        "indented %%( is not org-element diary-sexp (column-0 only), got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("After the block.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
