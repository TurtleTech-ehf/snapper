//! snapper-2xhr / GitHub #232: org-element-line-break-parser
//! `\\\\[ \t]*$` is Structure. The next physical line is not joined
//! onto `\\`. `Next claim.` still splits.

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
    concat!(
        "This is a long first sentence.\\\\\n",
        "This is the second sentence after the break. Next claim.\n",
    )
}

#[test]
fn line_break_backslash_pair_is_structure() {
    let regions = OrgParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "\\\\\n")),
        "\\\\ plus EOL must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("This is a long first sentence.")
                && !s.contains("second sentence")
        )),
        "first sentence must not join the next line, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is the second sentence after the break.")
                    && s.contains("Next claim.")
        )),
        "line after \\\\ must be its own prose, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_break_and_splits_next() {
    let input = ticket_fixture();
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "This is a long first sentence.\\\\\n",
            "This is the second sentence after the break.\n",
            "Next claim.\n",
        ),
        "\\\\ plus EOL stays; next line is its own sentence; Next claim. still splits, got:\n{out}"
    );
    assert!(
        !out.contains("sentence.\\\\ This") && !out.contains("sentence.\\\\This"),
        "must not join the next line onto \\\\, got:\n{out}"
    );
    assert!(
        !out.contains("the break. Next claim."),
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
fn trailing_spaces_after_line_break_stay_structure() {
    let input = "First sentence.\\\\  \nSecond sentence. Next claim.\n";
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "\\\\  \n")),
        "\\\\ plus trailing spaces plus EOL must stay Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("First sentence.\\\\  \nSecond sentence.\nNext claim."),
        "trailing spaces after \\\\ must survive and Next claim. split, got:\n{out}"
    );
}

#[test]
fn latex_linebreak_skip_is_not_org_line_break() {
    let input = "Keep this as one line\\\\[2ex] still prose. Next claim.\n";
    let regions = OrgParser.parse(input);
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.starts_with("\\\\"))),
        "\\\\[2ex] must not be a line-break Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("\\\\[2ex]"),
        "\\\\[2ex] must stay in prose, got:\n{out}"
    );
    assert!(
        out.contains("still prose.\nNext claim."),
        "Next claim. must still split, got:\n{out}"
    );
}

#[test]
fn mid_line_backslash_backslash_is_not_a_break() {
    let input = "Words with \\\\ inside. Next claim.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out, "Words with \\\\ inside.\nNext claim.\n",
        "mid-line \\\\ is not a line-break, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn same_line_math_then_line_break_still_splits() {
    let input = "The root is \\[ x = a.b \\] today.\\\\\nAfter the break. Next claim.\n";
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("\\[ x = a.b \\]"),
        "same-line math must stay intact, got:\n{out}"
    );
    assert!(
        out.contains("today.\\\\\nAfter the break.\nNext claim.\n"),
        "\\\\ after same-line math must stay a break, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn list_item_line_break_hangs_continuation() {
    let input = concat!(
        "- First sentence.\\\\\n",
        "  Second sentence. Next claim.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "- First sentence.\\\\\n",
            "  Second sentence.\n",
            "  Next claim.\n",
        ),
        "list item \\\\ stays; next line hangs and Next claim. still splits, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
