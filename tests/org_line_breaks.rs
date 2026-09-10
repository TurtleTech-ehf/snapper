//! snapper-2xhr / GitHub #232: Org `\\` line breaks stay Structure.
//!
//! org-element-line-break-parser `\\[ \t]*$`. The next physical line is
//! its own sentence; `Next claim.` still splits.

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
fn line_break_fixture() -> &'static str {
    "This is a long first sentence.\\\\\nThis is the second sentence after the break. Next claim.\n"
}

#[test]
fn line_break_plus_eol_is_structure() {
    let regions = OrgParser.parse(line_break_fixture());
    assert!(
        regions.iter().any(|r| match r {
            Region::Structure(s) => s == "\\\\\n" || s.starts_with("\\\\"),
            _ => false,
        }),
        "\\\\ plus EOL must stay Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("This is a long first sentence.")
                    && s.contains("This is the second sentence after the break.")
        )),
        "line break must not fuse the next physical line into one prose buffer, got {regions:?}"
    );
}

#[test]
fn line_break_fixture_next_line_is_own_sentence() {
    let input = line_break_fixture();
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
        !out.contains("sentence. This is the second"),
        "must not join the next line onto \\\\, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
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
fn line_break_with_trailing_spaces_stays_structure() {
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
        out.contains("First sentence.\\\\  \nSecond sentence.\n"),
        "trailing spaces on the break must survive, got:\n{out}"
    );
    assert!(
        out.contains("Next claim."),
        "Next claim. must still split, got:\n{out}"
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
fn list_item_line_break_does_not_join() {
    let input = "- First sentence.\\\\\n  Second sentence. Next claim.\n";
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
