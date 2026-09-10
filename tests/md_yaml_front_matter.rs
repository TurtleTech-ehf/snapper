use snapper_fmt::format::Format;
use snapper_fmt::parser::markdown::MarkdownParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// snapper-4why: pulldown `scan_metadata_block` / `scan_closing_metadata_block`.
/// `---` opens only when the next line is neither blank nor the closer.
/// YAML closer is `---` or `...`. Blank after `---` is a thematic break.

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
        "---\n",
        "title: Hello. World. This is a long title that would reflow.\n",
        "...\n",
        "\n",
        "Body after yaml. Second sentence.\n",
    )
}

#[test]
fn yaml_ellipsis_closes_front_matter() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "---\n"
        )),
        "YAML opener must be Structure, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s == "title: Hello. World. This is a long title that would reflow.\n"
        )),
        "YAML title must stay Structure, got: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == "...\n")),
        "... must close YAML, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Hello. World.") || p.contains("long title")
        )),
        "YAML title must not be Prose, got: {regions:?}"
    );
}

#[test]
fn yaml_body_after_ellipsis_still_splits() {
    let input = ticket_fixture();
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Body after yaml.") && p.contains("Second sentence.")
        )),
        "body after YAML must stay Prose, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Body after yaml.")
        )),
        "body after YAML must not be Structure, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("title: Hello. World. This is a long title that would reflow."),
        "YAML title must not reflow, got:\n{out}"
    );
    assert!(
        !out.contains("Hello.\nWorld") && !out.contains("Hello.\n World"),
        "must not split the YAML title, got:\n{out}"
    );
    assert!(
        out.contains("Body after yaml.\nSecond sentence."),
        "body Prose must still split, got:\n{out}"
    );
    assert!(
        !out.contains("Body after yaml. Second sentence."),
        "fused body must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn yaml_blank_after_dashes_is_thematic_break_not_front_matter() {
    let input = "---\n\nBody after yaml. Second sentence.\n";
    let regions = MarkdownParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == "---")),
        "--- then blank must be a thematic break, got: {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("Body after yaml.")
        )),
        "body must not be unclosed front matter, got: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("Body after yaml.") && p.contains("Second sentence.")
        )),
        "body after thematic --- must stay Prose, got: {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("---\n\nBody after yaml.\nSecond sentence."),
        "--- then blank is a break; body must split, got:\n{out}"
    );
    assert!(
        !out.contains("Body after yaml. Second sentence."),
        "fused body must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn yaml_dash_closer_still_works() {
    let input = concat!(
        "---\n",
        "title: Hello. World.\n",
        "---\n",
        "\n",
        "Body after yaml. Second sentence.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s == "title: Hello. World.\n"
        )),
        "--- closer must keep YAML as Structure, got: {regions:?}"
    );
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("title: Hello. World."),
        "YAML title must not reflow, got:\n{out}"
    );
    assert!(
        out.contains("Body after yaml.\nSecond sentence."),
        "body must still split, got:\n{out}"
    );
}
