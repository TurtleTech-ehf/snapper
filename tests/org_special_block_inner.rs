//! snapper-0rny / GitHub #179: Org special-block interiors are prose.
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
fn note_special_block_fixture() -> &'static str {
    concat!(
        "#+BEGIN_NOTE\n",
        "Quoted one. Quoted two.\n",
        "#+END_NOTE\n",
        "After the note. More.\n",
    )
}

#[test]
fn note_special_block_fences_structure_inner_prose_splits() {
    let input = note_special_block_fixture();
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_NOTE"))),
        "NOTE opener must stay Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("#+END_NOTE"))),
        "NOTE closer must stay Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Quoted one.") && p.contains("Quoted two.")
        )),
        "NOTE body must be Prose, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("Quoted one."))),
        "NOTE body must not freeze as Structure, got: {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("#+BEGIN_NOTE\nQuoted one.\nQuoted two.\n#+END_NOTE"),
        "NOTE sentences must reflow inside the fence, got:\n{out}"
    );
    assert!(
        !out.contains("Quoted one. Quoted two."),
        "NOTE sentences must not stay fused, got:\n{out}"
    );
    assert!(
        out.contains("#+END_NOTE\nAfter the note.\nMore."),
        "prose after NOTE must still reflow, got:\n{out}"
    );
    assert!(
        !out.contains("After the note. More."),
        "fused prose after NOTE must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn leftover_special_block_names_reflow_like_note() {
    for name in ["TIP", "WARNING", "ABSTRACT", "PROOF"] {
        let input = format!(
            "#+BEGIN_{name}\nQuoted one. Quoted two.\n#+END_{name}\nAfter the note. More.\n"
        );
        let regions = OrgParser.parse(&input);
        assert!(
            regions.iter().any(|r| matches!(
                r,
                Region::Prose(p) if p.contains("Quoted one.") && p.contains("Quoted two.")
            )),
            "{name} body must be Prose, got: {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("Quoted one."))),
            "{name} body must not freeze as Structure, got: {regions:?}"
        );
        let out = format_text(&input, &org_cfg()).unwrap();
        assert!(
            out.contains(&format!(
                "#+BEGIN_{name}\nQuoted one.\nQuoted two.\n#+END_{name}"
            )),
            "{name} sentences must reflow inside the fence, got:\n{out}"
        );
        assert!(
            out.contains("After the note.\nMore."),
            "prose after {name} must still reflow, got:\n{out}"
        );
        assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    }
}
