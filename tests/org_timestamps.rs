//! GitHub #248 / snapper-c0fn: org-element timestamps stay one inline
//! token. Active `<...>`, inactive `[...]`, ranges `--`, and diary
//! `<%%(...)>` are atomic. Interior punctuation is not a sentence
//! boundary. The use stays Prose. Radio / macro / fn / src_ / call_
//! stay their own tokens.

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
        "timestamp must stay Prose, got {regions:?}"
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
        !out.contains("<2024-01-01 Mon 10:00>\n")
            && !out.contains("10:00>--\n")
            && !out.contains("\n--<2024-01-02"),
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
}

#[test]
fn inactive_timestamp_stays_one_span() {
    let input = "Logged [2024-01-01 Mon 10:00] then leave. Next sentence.\n";
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
    let input = "Meet at <%%(equal (calendar-day-of-week date) 1.)> then leave. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("<%%(equal (calendar-day-of-week date) 1.)>"),
        "diary sexp must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("date) 1.\n") && !out.contains("1.\n)>"),
        "must not split inside the diary sexp, got:\n{out}"
    );
    assert!(
        out.contains("then leave.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn repeater_interior_punct_stays_one_span() {
    let input = "Meet at <2024-01-01 Mon .+1w> then leave. Next sentence.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("<2024-01-01 Mon .+1w>"),
        "repeater timestamp must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("Mon .\n") && !out.contains("Mon.\n+1w"),
        "must not split on the repeater period, got:\n{out}"
    );
    assert!(
        out.contains("then leave.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn radio_macro_fn_src_call_stay_their_tokens_beside_timestamps() {
    let input = concat!(
        "See <<<the Fourier. transform>>> at <2024-01-01 Mon .+1w> ",
        "and {{{cite(Smith. 2020)}}} plus [fn:: see fig. 1] ",
        "via src_python{print(1. 2)} or call_name(1. 2) today. Next sentence.\n"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("<<<the Fourier. transform>>>"),
        "radio must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("<2024-01-01 Mon .+1w>"),
        "timestamp must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("{{{cite(Smith. 2020)}}}"),
        "macro must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("[fn:: see fig. 1]"),
        "inline fn must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("src_python{print(1. 2)}"),
        "inline src must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("call_name(1. 2)"),
        "inline call must stay one token, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext sentence."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn wrap_does_not_cut_timestamp_range() {
    let input = ticket_fixture();
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
            .any(|l| l.contains("<2024-01-01 Mon 10:00>--<2024-01-02 Tue 12:00>")),
        "wrap must not cut inside the timestamp range, got:\n{wrapped}"
    );
    assert!(
        !wrapped.contains("10:00>--\n") && !wrapped.contains("Mon 10:00>\n--"),
        "must not wrap on the range dashes, got:\n{wrapped}"
    );
    assert!(
        wrapped.contains("Next sentence."),
        "sentence after the timestamp must still reflow, got:\n{wrapped}"
    );
}
