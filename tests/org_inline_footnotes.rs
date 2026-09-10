//! GitHub #231 / snapper-gjkj: org-element footnote references
//! (`[fn:: …]` / `[fn:note: …]`) stay one inline token. Interior
//! punctuation is not a sentence boundary. The use stays Prose.
//! Sibling of snapper-s5la (column-0 `[fn:LABEL]` definitions).

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

fn ticket_fixture() -> &'static str {
    "See [fn:: the Fourier. transform] in the notes. Next sentence.\n"
}

#[test]
fn ticket_inline_footnote_stays_prose() {
    let note = "[fn:: the Fourier. transform]";
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(note) && s.contains("Next sentence.")
        )),
        "inline footnote must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_footnote_span_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("[fn:: the Fourier. transform]"),
        "inline footnote must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("[fn:: the Fourier.\n") && !out.contains("Fourier.\ntransform]"),
        "must not split inside the inline footnote, got:\n{out}"
    );
    assert!(
        out.contains("in the notes.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("in the notes. Next sentence."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Org,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );

    // Bracket-depth merge can glue a UAX split at width 0. Wrap still
    // cuts on `Fourier.` unless [fn::…] is an atomic inline token.
    let wrap_cfg = FormatConfig {
        format: Format::Org,
        max_width: 28,
        ..Default::default()
    }
    .without_safety_backstops();
    let wrapped = format_text(input, &wrap_cfg).unwrap();
    assert!(
        wrapped
            .lines()
            .any(|l| l.contains("[fn:: the Fourier. transform]")),
        "wrap must not cut inside the inline footnote, got:\n{wrapped}"
    );
    assert!(
        !wrapped.contains("Fourier.\ntransform"),
        "must not wrap on the interior period, got:\n{wrapped}"
    );
    assert!(
        wrapped.contains("Next sentence."),
        "sentence after the footnote must still reflow, got:\n{wrapped}"
    );
    assert_eq!(format_text(&wrapped, &wrap_cfg).unwrap(), wrapped);
}

#[test]
fn named_inline_footnote_stays_one_span() {
    let input = "See [fn:note: the Fourier. transform] in the notes. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("[fn:note: the Fourier. transform]"),
        "named inline footnote must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("Fourier.\ntransform"),
        "must not split inside the named inline footnote, got:\n{out}"
    );
    assert!(
        out.contains("in the notes.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
