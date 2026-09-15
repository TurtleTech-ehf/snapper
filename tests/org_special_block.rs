//! GitHub #179 / snapper-0rny: Org special-block interiors are prose.
//! `#+BEGIN_NOTE` fences stay Structure; the body splits. After the note
//! still reflows.

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
fn leftover_begin_quote_same_line_hangs_and_splits() {
    let input = concat!(
        "#+BEGIN_QUOTE leftover. Next.\n",
        "Quoted one. Quoted two.\n",
        "#+END_QUOTE\n",
        "After. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_QUOTE"))),
        "quote opener must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("leftover.") && p.contains("Next.")
        )),
        "same-line leftover after BEGIN_QUOTE must be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        !out.contains("#+BEGIN_QUOTE leftover. Next."),
        "same-line leftover must still split, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_special_block_names_reflow_like_note() {
    for name in ["TIP", "WARNING", "ABSTRACT", "PROOF"] {
        let input = format!(
            "#+BEGIN_{name}\nQuoted one. Quoted two.\n#+END_{name}\nAfter the note. More.\n"
        );
        let out = format_text(&input, &org_cfg()).unwrap();
        assert_eq!(
            out,
            format!(
                "#+BEGIN_{name}\nQuoted one.\nQuoted two.\n#+END_{name}\nAfter the note.\nMore.\n"
            ),
            "{name} is the same special-block class as NOTE, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }
}
