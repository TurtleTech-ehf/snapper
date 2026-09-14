//! org-element latex-fragment leftover `$` rejects a space after the
//! opener. `$ x. Next $` is prose; `$a. b$` stays a fragment.

use snapper_fmt::format::Format;
use snapper_fmt::{format_text, FormatConfig};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn dollar_space_after_opener_is_prose_and_splits() {
    let input = "See $ x. Next $ today. After.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("See $ x.\nNext $ today."),
        "space after $ must not be a fragment, got:\n{out}"
    );
    assert!(
        out.contains("After."),
        "After. must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn dollar_closer_then_letter_is_not_a_fragment() {
    let input = "See $a. B$today. After.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("See $a.\nB$today."),
        "closer then letter must not be a fragment, got:\n{out}"
    );
    assert!(
        out.contains("After."),
        "After. must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn dollar_closer_then_comma_is_a_fragment() {
    let input = "See $a. b$, today. After.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("$a. b$"),
        "$a. b$ must stay one token before comma, got:\n{out}"
    );
    assert!(
        !out.contains("$a.\n"),
        "must not split inside $a. b$, got:\n{out}"
    );
    assert!(
        out.contains("today.\nAfter."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn dollar_closer_then_bracket_is_a_fragment() {
    let input = "See $a. b$[1]. After.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("$a. b$"),
        "$a. b$ must stay one token before bracket, got:\n{out}"
    );
    assert!(
        !out.contains("$a.\n"),
        "must not split inside $a. b$, got:\n{out}"
    );
    assert!(
        out.contains("After."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn tight_dollar_fragment_stays_atomic() {
    let input = "See $a. b$ today. After.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("$a. b$"),
        "$a. b$ must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("$a.\n"),
        "must not split inside $a. b$, got:\n{out}"
    );
    assert!(
        out.contains("today.\nAfter."),
        "following sentence must still split, got:\n{out}"
    );
}
