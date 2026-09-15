//! CM 0.31.2 §4.7 ex. 196: an LRD title may span physical lines.

use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

fn ticket_fixture() -> &'static str {
    concat!(
        "[foo]: /url\n",
        "\"Title with a period.\n",
        "Still title.\"\n",
        "\n",
        "After the definition. Next.\n",
    )
}

#[test]
fn spanning_lrd_title_is_structure() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]: /url")
        )),
        "dest must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Title with a period.")
        )),
        "title opener must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Still title.")
        )),
        "title closer must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Still title") || p.contains("Title with a period")
        )),
        "spanning title must not be Prose, got {regions:?}"
    );
}

#[test]
fn leftover_lrd_title_yields_to_fence() {
    let input = concat!(
        "[foo]: /url\n",
        "\"Title with a period.\n",
        "```\n",
        "code. yes\n",
        "```\n",
        "Still title.\"\n",
        "\n",
        "After the definition. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("code. yes")
        )),
        "fence inside a broken LRD title must be Code, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Still title")
        )),
        "interrupted title closer must not stay Structure, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("After the definition.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn leftover_lrd_title_yields_to_blockquote() {
    let input = concat!(
        "[foo]: /url\n",
        "\"Title with a period.\n",
        "> Still title. More.\n",
        "\n",
        "After the definition. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Still title")
        )),
        "blockquote must not stay LRD title Structure, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("After the definition.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn leftover_lrd_title_yields_to_setext_underline() {
    let input = concat!(
        "[foo]: /url\n",
        "\"Title with a period.\n",
        "=======\n",
        "Still title.\"\n",
        "\n",
        "After the definition. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]: /url")
        )),
        "dest-only LRD must stay Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Still title")
        )),
        "interrupted title closer must not stay Structure, got {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("After the definition.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn spanning_lrd_title_does_not_split_and_following_does() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("\"Title with a period.\nStill title.\""),
        "spanning title must stay one definition, got:\n{out}"
    );
    assert!(
        !out.contains("Title with a period.\nStill title.\"\nAfter")
            || out.contains("After the definition.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert!(
        out.contains("After the definition.\nNext."),
        "following prose must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
