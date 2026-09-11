//! GitHub #248 / snapper-c0fn: org-element timestamps (active `<...>`,
//! inactive `[...]`, ranges `--`, diary `<%%(...)>`) stay one inline
//! token. Interior punctuation is not a sentence or wrap boundary.
//! The use stays Prose. Radio / macro / fn / src_ / call_ stay tokens.

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
    "Meet at <2024-01-01 Mon 10:00>--<2024-01-02 Tue 12:00> then leave. Next sentence.\n"
}

#[test]
fn ticket_timestamp_use_stays_prose() {
    let ts = "<2024-01-01 Mon 10:00>--<2024-01-02 Tue 12:00>";
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(ts) && s.contains("Next sentence.")
        )),
        "timestamp range must stay Prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_timestamp_span_and_splits_next() {
    let input = ticket_fixture();
    let ts = "<2024-01-01 Mon 10:00>--<2024-01-02 Tue 12:00>";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains(ts),
        "timestamp range must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("Mon\n") && !out.contains("10:00>--<2024-01-02\n"),
        "must not split inside the timestamp range, got:\n{out}"
    );
    assert!(
        out.contains("then leave.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("then leave. Next sentence."),
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

    let wrap_cfg = FormatConfig {
        format: Format::Org,
        max_width: 32,
        ..Default::default()
    }
    .without_safety_backstops();
    let wrapped = format_text(input, &wrap_cfg).unwrap();
    assert!(
        wrapped.lines().any(|l| l.contains(ts)),
        "wrap must not cut inside the timestamp range, got:\n{wrapped}"
    );
    assert!(
        !wrapped.contains("Mon\n10:00") && !wrapped.contains("Tue\n12:00"),
        "must not wrap inside the timestamp range, got:\n{wrapped}"
    );
    assert!(
        wrapped.contains("Next sentence."),
        "sentence after the timestamp must still reflow, got:\n{wrapped}"
    );
    assert_eq!(format_text(&wrapped, &wrap_cfg).unwrap(), wrapped);
}

#[test]
fn inactive_timestamp_stays_one_span() {
    let input = "Meet at [2024-01-01 Mon 10:00] then leave. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("[2024-01-01 Mon 10:00]"),
        "inactive timestamp must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("then leave.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn diary_sexp_stays_one_span() {
    let input = "Meet at <%%(diary-float t 4 2)> then leave. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("<%%(diary-float t 4 2)>"),
        "diary sexp must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("then leave.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn radio_macro_fn_src_call_stay_tokens_beside_timestamp() {
    let input = concat!(
        "See <<<the Fourier. transform>>> and {{{cite(Smith. 2020)}}} ",
        "and [fn:: the Fourier. transform] and src_python{print(1. 2)} ",
        "and call_name(1. 2) at <2024-01-01 Mon 10:00>--<2024-01-02 Tue 12:00>. ",
        "Next sentence.\n"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    for token in [
        "<<<the Fourier. transform>>>",
        "{{{cite(Smith. 2020)}}}",
        "[fn:: the Fourier. transform]",
        "src_python{print(1. 2)}",
        "call_name(1. 2)",
        "<2024-01-01 Mon 10:00>--<2024-01-02 Tue 12:00>",
    ] {
        assert!(
            out.contains(token),
            "{token} must stay one token, got:\n{out}"
        );
    }
    assert!(
        out.contains(".\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
