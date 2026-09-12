//! GitHub #354 / snapper-sxte: org-element-export-snippet-parser.
//! A line that starts with `@@` is not whole-line Structure when prose
//! follows the closer. Backend is `[-A-Za-z0-9]+` so `html5` and hyphen
//! names stay one wrap token. Trailing `After the snippet.` / `Next.`
//! stay Prose and still split. Letter-backend `@@latex:1. 2@@` stays
//! one token. latex-fragment and brace-subscript wrap stay atomic.

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

fn wrap_cfg(width: usize) -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: width,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    "@@latex:\\newpage@@ After the snippet. Next.\n"
}

#[test]
fn ticket_trailing_prose_stays_prose() {
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("@@latex:\\newpage@@")
                    && s.contains("After the snippet.")
                    && s.contains("Next.")
        )),
        "only the snippet is the object; trailing prose stays Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("After the snippet.")
        )),
        "trailing prose must not freeze as Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_identity_splits_trailing_prose() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("@@latex:\\newpage@@ After the snippet.\nNext."),
        "After the snippet. / Next. must still split, got:\n{out}"
    );
    assert!(
        !out.contains("After the snippet. Next."),
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
fn ticket_html5_wrap_keeps_snippet_and_splits_next() {
    let input = "See @@html5:1. 2@@ today. Next.\n";
    let token = "@@html5:1. 2@@";
    let out = format_text(input, &wrap_cfg(16)).unwrap();
    assert!(
        out.lines().any(|l| l.contains(token)),
        "html5 snippet must stay one wrap token, got:\n{out}"
    );
    assert!(
        !out.contains("html5:1.\n") && !out.contains("1.\n2@@"),
        "must not wrap-split inside the html5 snippet, got:\n{out}"
    );
    assert!(
        !out.lines().any(|l| l.trim_start().starts_with("@@")),
        "wrap must not park the snippet at BOL (that line is Structure), got:\n{out}"
    );
    assert!(
        out.contains("See @@html5:1. 2@@"),
        "skip-cut keeps the snippet with See, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert!(
        !out.contains("today. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg(16)).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Org,
        max_width: 16,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn ticket_hyphen_backend_stays_one_wrap_token() {
    let input = "See @@html-5:1. 2@@ today. Next.\n";
    let token = "@@html-5:1. 2@@";
    let out = format_text(input, &wrap_cfg(16)).unwrap();
    assert!(
        out.lines().any(|l| l.contains(token)),
        "hyphen backend must stay one wrap token, got:\n{out}"
    );
    assert!(
        !out.contains("html-5:1.\n") && !out.contains("1.\n2@@"),
        "must not wrap-split inside the hyphen snippet, got:\n{out}"
    );
    assert!(
        !out.lines().any(|l| l.trim_start().starts_with("@@")),
        "wrap must not park the hyphen snippet at BOL, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &wrap_cfg(16)).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Org,
        max_width: 16,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn ticket_letter_backend_stays_one_token() {
    let input = "See @@latex:1. 2@@ today. Next.\n";
    let token = "@@latex:1. 2@@";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains(token),
        "letter-backend snippet must stay one token, got:\n{out}"
    );
    assert!(
        !out.contains("latex:1.\n") && !out.contains("1.\n2@@"),
        "must not split inside @@latex:1. 2@@, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext."),
        "following sentence must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn whole_line_snippet_stays_structure() {
    let input = "Text before.\n@@latex:\\newpage@@\nText after.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("@@latex:\\newpage@@"))),
        "a line that is only a snippet stays Structure, got {regions:?}"
    );
}

#[test]
fn latex_fragment_and_brace_subscript_stay_atomic() {
    let frag = format_text(
        "The root is \\(\\alpha. \\beta\\) today. Next.\n",
        &wrap_cfg(12),
    )
    .unwrap();
    assert!(
        frag.contains("\\(\\alpha. \\beta\\)"),
        "latex-fragment wrap must stay atomic, got:\n{frag}"
    );
    assert!(
        !frag.contains("\\(\\alpha.\n") && !frag.contains("alpha.\n\\beta"),
        "must not wrap inside the fragment, got:\n{frag}"
    );
    assert!(
        frag.contains("today.\nNext."),
        "fragment trailing sentence must still split, got:\n{frag}"
    );

    let sub = format_text("See H_{2. 0} today. Next.\n", &wrap_cfg(10)).unwrap();
    assert!(
        sub.lines().any(|l| l.contains("H_{2. 0}")),
        "brace-subscript wrap must stay atomic, got:\n{sub}"
    );
    assert!(
        !sub.contains("2.\n0") && !sub.contains("H_{2.\n"),
        "must not wrap on the interior period, got:\n{sub}"
    );
    assert!(
        sub.contains("today.\nNext."),
        "subscript trailing sentence must still split, got:\n{sub}"
    );
}
