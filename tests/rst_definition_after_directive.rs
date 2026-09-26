use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #135 / snapper-v1ze: after a nested `.. pull-quote::` the
/// second definition/quote sentence must keep the three-space hang.
/// Docutils keeps both sentences inside the same block; outdenting the
/// last line moves it out of that container.
fn ticket_fixture() -> &'static str {
    concat!(
        "Loading-state race:\n",
        "\n",
        "   Ask this question:\n",
        "\n",
        "   .. pull-quote::\n",
        "\n",
        "      What happens?\n",
        "\n",
        "   First sentence.\n",
        "   Second sentence.\n",
    )
}

#[test]
fn definition_paragraph_after_nested_directive_keeps_hang() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = ticket_fixture();
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "second sentence must stay inside the definition, got:\n{out}"
    );
    assert!(
        out.contains("\n   Second sentence."),
        "second sentence must keep three-space indent, got:\n{out}"
    );
    assert!(
        !out.contains("\nSecond sentence."),
        "second sentence must not outdent to column 0, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung definition paragraph must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn compact_sentences_after_nested_directive_hang() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "Loading-state race:\n",
        "\n",
        "   Ask this question:\n",
        "\n",
        "   .. pull-quote::\n",
        "\n",
        "      What happens?\n",
        "\n",
        "   First sentence. Second sentence.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        concat!(
            "Loading-state race:\n",
            "\n",
            "   Ask this question:\n",
            "\n",
            "   .. pull-quote::\n",
            "\n",
            "      What happens?\n",
            "\n",
            "   First sentence.\n",
            "   Second sentence.\n",
        ),
        "split second sentence must keep three-space hang, got:\n{out}"
    );
    assert!(
        !out.contains("\nSecond sentence."),
        "split second sentence must not outdent, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(out, twice, "hung split must be identity, got:\n{twice}");
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}
