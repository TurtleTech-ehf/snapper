//! GitHub #179 / snapper-0rny: Org special-block interiors are prose.
//! `#+BEGIN_NOTE` fences stay Structure; the body splits like a quote.

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
fn special_block_note_fixture() -> &'static str {
    concat!(
        "#+BEGIN_NOTE\n",
        "Quoted one. Quoted two.\n",
        "#+END_NOTE\n",
        "After the note. More.\n",
    )
}

#[test]
fn note_special_block_inner_prose_splits() {
    let input = special_block_note_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#+BEGIN_NOTE\n",
            "Quoted one.\n",
            "Quoted two.\n",
            "#+END_NOTE\n",
            "After the note.\n",
            "More.\n",
        ),
        "NOTE fences stay; inner and following prose must split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn warning_special_block_same_class_as_note() {
    let out = format_text(
        "#+BEGIN_WARNING\nQuoted one. Quoted two.\n#+END_WARNING\nAfter the note. More.\n",
        &org_cfg(),
    )
    .unwrap();
    assert_eq!(
        out,
        concat!(
            "#+BEGIN_WARNING\n",
            "Quoted one.\n",
            "Quoted two.\n",
            "#+END_WARNING\n",
            "After the note.\n",
            "More.\n",
        ),
        "WARNING is the same special-block class as NOTE, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
