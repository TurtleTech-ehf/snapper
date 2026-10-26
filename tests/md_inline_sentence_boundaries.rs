//! GitHub #530: Org emphasis must not hide Markdown prose boundaries.

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
fn markdown_inline_characters_do_not_hide_sentence_boundaries() {
    for (input, expected) in [
        (
            "A/B works. See https://example.com.\n",
            "A/B works.\nSee https://example.com.\n",
        ),
        ("A/B works. C/D works.\n", "A/B works.\nC/D works.\n"),
        (
            "foo_bar works. Baz_qux works.\n",
            "foo_bar works.\nBaz_qux works.\n",
        ),
        ("a+b works. C+d works.\n", "a+b works.\nC+d works.\n"),
        (
            "A/B (\"Quoted C/D `code.` Still quoted\") ends. Next sentence.\n",
            "A/B (\"Quoted C/D `code.` Still quoted\") ends.\nNext sentence.\n",
        ),
        (
            "été_bar works. Baz_qux works.\n",
            "été_bar works.\nBaz_qux works.\n",
        ),
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
fn markdown_inline_characters_do_not_prevent_width_wrapping() {
    let config = FormatConfig {
        max_width: 16,
        ..md_cfg()
    };
    for (input, expected) in [
        (
            "A/B alpha beta gamma delta https://example.com.\n",
            "A/B alpha beta\ngamma delta\nhttps://example.com.\n",
        ),
        (
            "a+b alpha beta gamma delta c+d.\n",
            "a+b alpha beta\ngamma delta c+d.\n",
        ),
        (
            "a_b alpha beta gamma delta c_d.\n",
            "a_b alpha beta\ngamma delta c_d.\n",
        ),
    ] {
        let out = format_text(input, &config).unwrap();
        assert_eq!(out, expected);
        assert!(oracle::matches(Format::Markdown, input, &out));
        assert_eq!(format_text(&out, &config).unwrap(), out);
    }
}

#[test]
fn format_aware_inline_protection_preserves_org_emphasis_and_markdown_literals() {
    let org = FormatConfig {
        format: Format::Org,
        ..Default::default()
    }
    .without_safety_backstops();
    for marker in ['/', '+', '_'] {
        let span = format!("{marker}First sentence. Second sentence{marker}");
        let input = format!("{span} remains emphasized. Next sentence.\n");
        let out = format_text(&input, &org).unwrap();
        assert_eq!(out, format!("{span} remains emphasized.\nNext sentence.\n"));
        assert!(oracle::matches(Format::Org, &input, &out));
        assert_eq!(format_text(&out, &org).unwrap(), out);
    }

    for input in [
        "A/B works.\nSee https://example.com.\n",
        "`A/B works. C/D works.` stays literal.\n",
        "*Two sentences. Inside emphasis* remain emphasized.\n",
        "_Two sentences. Inside emphasis_ remain emphasized.\n",
        "__Two sentences. Inside strong emphasis__ remain emphasized.\n",
        "***Two sentences. Inside emphasis*** remain emphasized.\n",
        "___Two sentences. Inside emphasis___ remain emphasized.\n",
        "(_Two sentences. Inside emphasis_) remain emphasized.\n",
        "_First span. Still first_ and _Second span. Still second_ stay intact.\n",
        "See [the docs. More information](https://example.com/path).\n",
        "```text\nA/B works. C/D works.\n```\n",
    ] {
        let out = format_text(input, &md_cfg()).unwrap();
        assert_eq!(out, input);
    }
}

#[test]
fn markdown_inline_characters_respect_lowercase_sentence_start_policy() {
    let config = FormatConfig {
        lowercase_starts: true,
        ..md_cfg()
    };
    for (input, expected) in [
        (
            "foo_bar works. baz_qux works.\n",
            "foo_bar works.\nbaz_qux works.\n",
        ),
        ("a+b works. c+d works.\n", "a+b works.\nc+d works.\n"),
    ] {
        assert_eq!(format_text(input, &md_cfg()).unwrap(), input);
        let out = format_text(input, &config).unwrap();
        assert_eq!(out, expected);
        assert!(oracle::matches(Format::Markdown, input, &out));
        assert_eq!(format_text(&out, &config).unwrap(), out);
    }
}

#[test]
fn non_org_slash_and_plus_pairs_do_not_hide_sentence_boundaries() {
    for format in [Format::Plaintext, Format::Latex] {
        let config = FormatConfig { format, ..md_cfg() };
        for (input, expected) in [
            ("A/B works. C/D works.\n", "A/B works.\nC/D works.\n"),
            ("a+b works. C+d works.\n", "a+b works.\nC+d works.\n"),
        ] {
            let out = format_text(input, &config).unwrap();
            assert_eq!(out, expected, "{format:?}");
            assert_eq!(format_text(&out, &config).unwrap(), out);
        }
    }
}
