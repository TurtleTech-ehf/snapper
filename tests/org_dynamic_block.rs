//! GitHub #178 / snapper-57v0: Org dynamic-block bodies stay Structure.
//! `#+BEGIN: NAME` … `#+END:` is not a one-line keyword plus Prose.

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

/// Ticket fixture (Format::Org / GitHub #178).
fn clocktable_fixture() -> &'static str {
    concat!(
        "#+BEGIN: clocktable :scope file\n",
        "This is a long sentence inside a dynamic block that must stay frozen. Second sentence.\n",
        "#+END:\n",
        "Following paragraph. Another sentence.\n",
    )
}

#[test]
fn dynamic_block_body_stays_frozen_following_prose_splits() {
    let input = clocktable_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#+BEGIN: clocktable :scope file\n",
            "This is a long sentence inside a dynamic block that must stay frozen. Second sentence.\n",
            "#+END:\n",
            "Following paragraph.\n",
            "Another sentence.\n",
        ),
        "BEGIN through END stays frozen; following prose splits, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn lowercase_dynamic_block_is_also_structure() {
    let input = concat!(
        "#+begin: clocktable :scope file\n",
        "Frozen one. Frozen two.\n",
        "#+end:\n",
        "After. Next.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#+begin: clocktable :scope file\n",
            "Frozen one. Frozen two.\n",
            "#+end:\n",
            "After.\n",
            "Next.\n",
        ),
        "case-fold BEGIN:/END: must freeze the body, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
