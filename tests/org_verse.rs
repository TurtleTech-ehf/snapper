//! snapper-hw7m / GitHub #281: an org verse line with two sentences
//! stays one physical line. org-element verse-line lineation is the
//! object. Inner stays Prose, not Structure. After the block still
//! splits. Quote / center / special-block reflow is unchanged.

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
fn ticket_fixture() -> &'static str {
    concat!(
        "#+BEGIN_VERSE\n",
        "First line. Second line.\n",
        "#+END_VERSE\n",
        "After the block. Next.\n",
    )
}

#[test]
fn verse_line_is_prose_not_structure() {
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_VERSE"))),
        "verse opener must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("First line.") && p.contains("Second line.")
        )),
        "verse line must be Prose, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("First line."))),
        "verse inner must not flip to Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_verse_line_and_splits_after() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#+BEGIN_VERSE\n",
            "First line. Second line.\n",
            "#+END_VERSE\n",
            "After the block.\n",
            "Next.\n",
        ),
        "verse line stays one physical line; After the block. / Next. still splits, got:\n{out}"
    );
    assert!(
        !out.contains("First line.\nSecond line."),
        "must not invent a verse line break, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
    assert!(
        snapper_fmt::oracle::matches(Format::Org, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn period_free_verse_lines_stay_unjoined() {
    let input = concat!(
        "#+BEGIN_VERSE\n",
        "Great clouds overhead\n",
        "Tiny black birds rise and fall\n",
        "#+END_VERSE\n",
        "After. Next.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("Great clouds overhead\nTiny black birds rise and fall"),
        "period-free verse lines must stay separate, got:\n{out}"
    );
    assert!(
        !out.contains("Great clouds overhead Tiny black birds"),
        "verse must not join lines, got:\n{out}"
    );
    assert!(
        out.contains("#+END_VERSE\nAfter.\nNext."),
        "prose after verse must still split, got:\n{out}"
    );
}

#[test]
fn quote_center_special_block_still_split() {
    let quote = "#+BEGIN_QUOTE\nQuoted one. Quoted two.\n#+END_QUOTE\n";
    let center = "#+BEGIN_CENTER\nCentered one. Centered two.\n#+END_CENTER\n";
    let note = "#+BEGIN_NOTE\nQuoted one. Quoted two.\n#+END_NOTE\n";
    let q = format_text(quote, &org_cfg()).unwrap();
    let c = format_text(center, &org_cfg()).unwrap();
    let n = format_text(note, &org_cfg()).unwrap();
    assert!(
        q.contains("Quoted one.\nQuoted two."),
        "quote inner must still split, got:\n{q}"
    );
    assert!(
        c.contains("Centered one.\nCentered two."),
        "center inner must still split, got:\n{c}"
    );
    assert!(
        n.contains("Quoted one.\nQuoted two."),
        "special-block inner must still split, got:\n{n}"
    );
}
