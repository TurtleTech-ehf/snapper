//! GitHub #211 / snapper-gz10: org-element radio targets (`<<<...>>>`)
//! and angle targets (`<<...>>`) stay one inline token. Interior
//! punctuation is not a sentence boundary. The use stays Prose.

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
    "See <<<the Fourier. transform>>> in the text. Next sentence.\n"
}

#[test]
fn ticket_radio_use_stays_prose() {
    let radio = "<<<the Fourier. transform>>>";
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(radio) && s.contains("Next sentence.")
        )),
        "radio target must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_radio_span_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("<<<the Fourier. transform>>>"),
        "radio target must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("<<<the Fourier.\n") && !out.contains("Fourier.\ntransform>>>"),
        "must not split inside the radio target, got:\n{out}"
    );
    assert!(
        out.contains("in the text.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("in the text. Next sentence."),
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
}

#[test]
fn angle_target_stays_one_span() {
    let input = "See <<sec. intro>> in the text. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("<<sec. intro>>"),
        "angle target must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("sec.\nintro"),
        "must not split inside the angle target, got:\n{out}"
    );
    assert!(
        out.contains("in the text.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
