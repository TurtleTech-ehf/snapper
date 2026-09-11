//! GitHub #319 / snapper-svbp: org-element planning and clock lines
//! match ignore ASCII case. Emacs 30.2 binds `case-fold-search` t
//! before `org-element-planning-line-re` and `org-element-clock-line-re`.
//! Lowercase `deadline:` / `clock:` stay unjoined; following prose still
//! splits. Uppercase `DEADLINE:` / `CLOCK:` and inline timestamps stay.

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

/// Ticket fixture (Format::Org / GitHub #319).
fn ticket_fixture() -> &'static str {
    concat!(
        "deadline: <2026-01-01 Wed>\n",
        "After planning. Next sentence.\n",
    )
}

#[test]
fn lowercase_deadline_is_structure() {
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("deadline: <2026-01-01 Wed>")
        )),
        "deadline: must be Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("deadline:"))),
        "deadline: must not be Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After planning.") && p.contains("Next sentence.")
        )),
        "following prose must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_lowercase_deadline_unjoined_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "deadline: <2026-01-01 Wed>\n",
            "After planning.\n",
            "Next sentence.\n",
        ),
        "deadline: stays unjoined; After planning. / Next sentence. still split, got:\n{out}"
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
fn lowercase_clock_stays_unjoined() {
    let input = "clock: [2026-01-01 Thu 10:00]\nAfter planning. Next sentence.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("clock: [2026-01-01 Thu 10:00]")
        )),
        "clock: must be Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("clock:"))),
        "clock: must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "clock: [2026-01-01 Thu 10:00]\n",
            "After planning.\n",
            "Next sentence.\n",
        ),
        "clock: stays unjoined; After planning. / Next sentence. still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn lowercase_scheduled_and_closed_stay_unjoined() {
    let input = concat!(
        "scheduled: <2026-01-02 Thu>\n",
        "closed: [2026-01-01 Wed 09:00]\n",
        "After planning. Next sentence.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "scheduled: <2026-01-02 Thu>\n",
            "closed: [2026-01-01 Wed 09:00]\n",
            "After planning.\n",
            "Next sentence.\n",
        ),
        "scheduled:/closed: stay unjoined, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn uppercase_deadline_clock_and_inline_timestamps_unchanged() {
    let input = concat!(
        "* TODO Task\n",
        "DEADLINE: <2026-01-01 Wed>\n",
        "CLOCK: [2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00] =>  1:00\n",
        "Meet at <2026-01-01 Wed> then leave. Next sentence.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("* TODO Task\nDEADLINE: <2026-01-01 Wed>\nCLOCK:"),
        "uppercase DEADLINE: must stay its own line, got:\n{out}"
    );
    assert!(
        !out.contains("DEADLINE: <2026-01-01 Wed> CLOCK:"),
        "uppercase DEADLINE: must not glue onto CLOCK:, got:\n{out}"
    );
    assert!(
        out.contains("CLOCK: [2026-01-01 Thu 10:00]--[2026-01-01 Thu 11:00] =>  1:00\nMeet at "),
        "uppercase CLOCK: must stay its own line, got:\n{out}"
    );
    assert!(
        !out.contains("=>  1:00 Meet at"),
        "uppercase CLOCK: must not glue onto following prose, got:\n{out}"
    );
    assert!(
        out.contains("<2026-01-01 Wed>"),
        "inline timestamp must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("then leave.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
