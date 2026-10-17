//! GitHub #530: Org /italic/ protection must not pair ordinary Markdown slashes.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text, oracle};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn markdown_slash_pairs_do_not_hide_sentence_boundaries() {
    for (input, expected) in [
        (
            "A/B works. See https://example.com.\n",
            "A/B works.\nSee https://example.com.\n",
        ),
        ("A/B works. C/D works.\n", "A/B works.\nC/D works.\n"),
        (
            "The loader/WASM are unchanged copies of the retained minimal pair. FFmpeg's upstream license is https://github.com/FFmpeg/FFmpeg/blob/n5.1.4/LICENSE.md.\n",
            "The loader/WASM are unchanged copies of the retained minimal pair.\nFFmpeg's upstream license is https://github.com/FFmpeg/FFmpeg/blob/n5.1.4/LICENSE.md.\n",
        ),
    ] {
        let out = format_text(input, &md_cfg()).unwrap();
        assert_eq!(out, expected);
        assert!(oracle::matches(Format::Markdown, input, &out));
        assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
    }
}

#[test]
fn markdown_slash_protection_does_not_prevent_width_wrapping() {
    let config = FormatConfig {
        max_width: 16,
        ..md_cfg()
    };
    let input = "A/B alpha beta gamma delta https://example.com.\n";
    let expected = "A/B alpha beta\ngamma delta\nhttps://example.com.\n";
    let out = format_text(input, &config).unwrap();
    assert_eq!(out, expected);
    assert!(oracle::matches(Format::Markdown, input, &out));
    assert_eq!(format_text(&out, &config).unwrap(), out);
}

#[test]
fn markdown_slash_fix_preserves_org_emphasis_and_markdown_literals() {
    let org = FormatConfig {
        format: Format::Org,
        ..Default::default()
    }
    .without_safety_backstops();
    let italic = "/First sentence. Second sentence/ remains italic. Next sentence.\n";
    let out = format_text(italic, &org).unwrap();
    assert!(out.contains("/First sentence. Second sentence/"));
    assert!(oracle::matches(Format::Org, italic, &out));
    assert_eq!(format_text(&out, &org).unwrap(), out);

    for input in [
        "A/B works.\nSee https://example.com.\n",
        "`A/B works. C/D works.` stays literal.\n",
        "*Two sentences. Inside emphasis* remain emphasized.\n",
        "See [the docs. More information](https://example.com/path).\n",
        "```text\nA/B works. C/D works.\n```\n",
    ] {
        let out = format_text(input, &md_cfg()).unwrap();
        assert_eq!(out, input);
    }
}
