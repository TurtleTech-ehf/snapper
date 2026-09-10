//! snapper-62f5 / GitHub #204: Org `#+CAPTION:` values reflow; NAME/ATTR do not.
//!
//! `org-element-parsed-keywords` is CAPTION only. The `#+CAPTION:` opener
//! (optional `[short]`) is Structure; the value hangs and splits.
//! `#+NAME:` and `#+ATTR_*` stay whole-line Structure.

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

/// Ticket fixture: affiliated CAPTION, figure link, following prose.
fn ticket_fixture() -> &'static str {
    concat!(
        "#+CAPTION: This is a long figure caption that must reflow as prose. Second sentence.\n",
        "[[file:plot.png]]\n",
        "After the figure. More.\n",
    )
}

#[test]
fn caption_opener_is_structure_value_is_hung_prose() {
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "#+CAPTION: ")),
        "#+CAPTION: must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long figure caption that must reflow as prose.")
                    && s.contains("Second sentence.")
        )),
        "CAPTION value must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("This is a long figure caption")
        )),
        "CAPTION value must not stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[[file:plot.png]]")
        )),
        "figure link line must stay Structure, got {regions:?}"
    );
}

#[test]
fn caption_fixture_value_splits_and_hangs() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#+CAPTION: This is a long figure caption that must reflow as prose.\n",
            "           Second sentence.\n",
            "[[file:plot.png]]\n",
            "After the figure.\n",
            "More.\n",
        ),
        "CAPTION opener stays; value hangs and splits, got:\n{out}"
    );
    assert!(
        !out.lines().any(|l| l == "Second sentence."),
        "must not emit a column-0 second caption sentence, got:\n{out}"
    );
    assert!(
        !out.contains("[[file:plot.png]] After the figure."),
        "link line must not join following prose, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &org_cfg()).unwrap(),
        out,
        "hung CAPTION must be identity, got:\n{out}"
    );
}

#[test]
fn caption_dual_short_title_stays_structure() {
    let input = "#+CAPTION[Short. Title.]: Long caption. Second.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "#+CAPTION[Short. Title.]: "
        )),
        "dual [short] must stay in the Structure opener, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Short. Title.")
        )),
        "short title must not be Prose, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#+CAPTION[Short. Title.]: Long caption.\n",
            "                          Second.\n",
        ),
        "dual short stays; long value hangs and splits, got:\n{out}"
    );
    assert!(
        !out.contains("Short.\n"),
        "Short. Title. must not split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn name_and_attr_stay_whole_line_structure() {
    let input = concat!(
        "#+NAME: First sentence. Second sentence.\n",
        "#+ATTR_HTML: :alt First sentence. Second.\n",
        "#+ATTR_LATEX: :width 0.9 :alt First sentence. Second sentence.\n",
        "#+CAPTION: Long caption. Second.\n",
        "[[file:fig.png]]\n",
        "\n",
        "After the figure. Next.\n",
    );
    let regions = OrgParser.parse(input);
    for needle in [
        "#+NAME: First sentence. Second sentence.",
        "#+ATTR_HTML: :alt First sentence. Second.",
        "#+ATTR_LATEX: :width 0.9 :alt First sentence. Second sentence.",
    ] {
        assert!(
            regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains(needle))),
            "{needle} must stay Structure, got {regions:?}"
        );
        assert!(
            !regions
                .iter()
                .any(|r| matches!(r, Region::Prose(s) if s.contains(needle))),
            "{needle} must not be Prose, got {regions:?}"
        );
    }
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("#+NAME: First sentence. Second sentence.\n"),
        "#+NAME: must stay whole-line, got:\n{out}"
    );
    assert!(
        out.contains("#+ATTR_HTML: :alt First sentence. Second.\n"),
        "#+ATTR_HTML: must stay whole-line, got:\n{out}"
    );
    assert!(
        out.contains("#+ATTR_LATEX: :width 0.9 :alt First sentence. Second sentence.\n"),
        "#+ATTR_LATEX: must stay whole-line, got:\n{out}"
    );
    assert!(
        out.contains("#+CAPTION: Long caption.\n           Second.\n"),
        "CAPTION value must still hang, got:\n{out}"
    );
    assert!(
        out.contains("After the figure.\nNext.\n"),
        "following prose must still reflow, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
