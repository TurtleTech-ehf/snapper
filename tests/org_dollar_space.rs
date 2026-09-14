//! org-element latex-fragment leftover `$` rejects a space after the
//! opener. `$ x. Next $` is prose; `$a. b$` stays a fragment.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

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
