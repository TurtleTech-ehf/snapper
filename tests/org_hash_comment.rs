//! GitHub #177 / snapper-stms: Org hash comments require a space or EOL.
//! `#not-a-comment` is prose and must reflow; `# comment` stays Structure.

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

/// Ticket fixture (Format::Org).
fn hash_comment_space_or_eol_fixture() -> &'static str {
    concat!(
        "#not-a-comment This is a long sentence that must reflow as prose. Second sentence.\n",
        "# This is a real comment and must stay frozen.\n",
    )
}

#[test]
fn hash_without_space_is_prose_and_splits() {
    let input = hash_comment_space_or_eol_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#not-a-comment This is a long sentence that must reflow as prose.\n",
            "Second sentence.\n",
            "# This is a real comment and must stay frozen.\n",
        ),
        "#not-a-comment must split; real comment stays frozen, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn bare_hash_eol_stays_comment() {
    let out = format_text("#\nAfter. Next.\n", &org_cfg()).unwrap();
    assert_eq!(out, "#\nAfter.\nNext.\n", "got:\n{out}");
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
