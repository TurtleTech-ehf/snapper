use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #134 / snapper-7rgg: compact list-like lines after a paragraph
/// are still that paragraph. Sentence split must not hang a continuation
/// and invent a list / Unexpected indentation.
#[test]
fn compact_listlike_prose_stays_one_paragraph() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "Topics include:\n",
        "* Product requirements - what pages exist? What functionality lives on them?\n",
        "* Technical requirements\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        concat!(
            "Topics include: * Product requirements - what pages exist?\n",
            "What functionality lives on them?\n",
            "* Technical requirements\n",
        ),
        "compact list-like prose must stay one paragraph, got:\n{out}"
    );
    assert!(
        !out.contains("\n  What functionality"),
        "must not hang a continuation and invent Unexpected indentation, got:\n{out}"
    );
    assert!(
        !out.contains("\n* Product"),
        "must not start a list at Product requirements, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "compact list-like paragraph must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}
