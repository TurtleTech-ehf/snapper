//! GitHub #357 / snapper-98o9: CommonMark 0.31.2 sec 4.7.
//! An LRD does not interrupt a paragraph. Without a blank,
//! `[foo]: /url/a.b` stays in the paragraph. After a blank,
//! dest stays Structure and After. / Next. still split.

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

/// Ticket fixture (Format::Markdown / GitHub #357).
fn ticket_fixture() -> &'static str {
    concat!(
        "Foo is a sentence. Bar is another.\n",
        "[foo]: /url/a.b\n",
        "After. Next.\n",
    )
}

#[test]
fn lrd_without_blank_stays_in_paragraph() {
    let regions = MarkdownParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Foo is a sentence.")
                    && p.contains("Bar is another.")
                    && p.contains("[foo]: /url/a.b")
                    && p.contains("After.")
                    && p.contains("Next.")
        )),
        "LRD must stay in the paragraph, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]:")
        )),
        "mid-paragraph LRD must not become Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_keeps_lrd_in_paragraph() {
    let input = ticket_fixture();
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Foo is a sentence.\nBar is another."),
        "Foo / Bar must still split, got:\n{out}"
    );
    assert!(
        out.contains("[foo]: /url/a.b"),
        "LRD dest must remain, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "After. / Next. must still split, got:\n{out}"
    );
    assert!(
        !out.contains("After. Next."),
        "fused After. Next. must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn lrd_after_blank_is_structure_and_next_splits() {
    let input = concat!(
        "Foo is a sentence. Bar is another.\n",
        "\n",
        "[foo]: /url/a.b\n",
        "After. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]: /url/a.b")
        )),
        "LRD after a blank must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("[foo]:")
        )),
        "LRD after a blank must not be Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p) if p.contains("After.") && p.contains("Next.")
        )),
        "After. / Next. must stay Prose, got {regions:?}"
    );

    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("Foo is a sentence.\nBar is another."),
        "first paragraph must still split, got:\n{out}"
    );
    assert!(
        out.contains("[foo]: /url/a.b"),
        "dest must stay its own Structure line, got:\n{out}"
    );
    assert!(
        !out.contains("[foo]: /url/a.b After"),
        "must not glue After. onto the dest, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext."),
        "After. / Next. must still split, got:\n{out}"
    );
    assert!(
        !out.contains("After. Next."),
        "fused following prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn dest_next_line_without_blank_stays_in_paragraph() {
    let input = concat!(
        "Foo is a sentence. Bar is another.\n",
        "[foo]:\n",
        "/url/a.b\n",
        "After. Next.\n",
    );
    let regions = MarkdownParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(p)
                if p.contains("Foo is a sentence.")
                    && p.contains("[foo]:")
                    && p.contains("/url/a.b")
                    && p.contains("After.")
        )),
        "dest-next-line LRD must stay in the paragraph, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("[foo]:") || s.contains("/url/a.b")
        )),
        "mid-paragraph dest-next-line must not become Structure, got {regions:?}"
    );
}
