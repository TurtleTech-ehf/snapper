//! GitHub #318 / snapper-p6ng: Org drawer names follow Emacs
//! `org-element-drawer-re` NAME = `(any ?- ?_ word)`.
//! Digits and unicode letters open a drawer; `:See also:` does not.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Org / GitHub #318).
fn word_drawer_fixture(name: &str) -> String {
    format!(":{name}:\npayload text. More text.\n:END:\nAfter drawer. Next.\n")
}

#[test]
fn digit_and_unicode_drawer_names_freeze_body() {
    for name in ["LOG1", "föö", "1", "ID2"] {
        let input = word_drawer_fixture(name);
        let out = format_text(&input, &org_cfg()).unwrap();
        assert_eq!(
            out,
            format!(":{name}:\npayload text. More text.\n:END:\nAfter drawer.\nNext.\n"),
            ":{name}: must open a drawer; body frozen including :END:, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }
}

#[test]
fn ascii_logbook_stays_structure() {
    let input = ":LOGBOOK:\nCLOCK: [2026-01-01] First. Second.\n:END:\nAfter drawer. Next.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out, ":LOGBOOK:\nCLOCK: [2026-01-01] First. Second.\n:END:\nAfter drawer.\nNext.\n",
        "ASCII :LOGBOOK: must stay a drawer, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn see_also_is_not_a_drawer() {
    let input = ":See also:\nThis is a note. Second sentence.\nAfter the note. More.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("This is a note.\nSecond sentence."),
        ":See also: is not a NAME; notes must reflow, got:\n{out}"
    );
    assert!(
        out.contains("After the note.\nMore."),
        ":See also: must not swallow following prose, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn fixed_width_colon_space_unchanged() {
    let input = ": First sentence. Second sentence.\nAfter fixed. More after.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out, ": First sentence. Second sentence.\nAfter fixed.\nMore after.\n",
        "fixed-width : text must stay frozen, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
