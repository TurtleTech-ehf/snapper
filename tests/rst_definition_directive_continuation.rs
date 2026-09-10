use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #135 / snapper-v1ze: a definition paragraph after a nested
/// `.. pull-quote::` must keep three-space indent on the second sentence.
#[test]
fn definition_after_nested_directive_keeps_three_space_indent() {
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
        "   First sentence.\n",
        "   Second sentence.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "second definition sentence must keep three-space indent, got:\n{out}"
    );
    assert!(
        out.contains("\n   Second sentence."),
        "second sentence must stay inside the definition, got:\n{out}"
    );
    assert!(
        !out.contains("\nSecond sentence."),
        "second sentence must not outdent to column 0, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung definition after nested directive must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );

    let compact = concat!(
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
    let compact_out = format_text(compact, &cfg).unwrap();
    assert_eq!(
        compact_out, input,
        "compact definition after nested directive must hang the second sentence, got:\n{compact_out}"
    );
}
