use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #126 / snapper-y8mf: a sentence split after an RST role on a
/// list continuation must keep the marker hang. `:class:`Name`` is
/// interpreted text, not a field list.
#[test]
fn role_continuation_sentence_split_keeps_list_hang() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "* TooManyRequests is returned when a\n",
        "  :class:`CloudDatabase` exceeds a configured request rate\n",
        "  limit. Set requests_per_second_limit to 0 for every request.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        concat!(
            "* TooManyRequests is returned when a :class:`CloudDatabase` exceeds a configured request rate limit.\n",
            "  Set requests_per_second_limit to 0 for every request.\n",
        ),
        "second sentence must hang as a list continuation, got:\n{out}"
    );
    assert!(
        out.contains("\n  Set requests_per_second_limit to 0 for every request."),
        "second sentence must stay indented, got:\n{out}"
    );
    assert!(
        !out.contains("\nSet requests_per_second_limit"),
        "second sentence must not outdent to column 0, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung role continuation must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );

    let already_hung = concat!(
        "* TooManyRequests is returned when a :class:`CloudDatabase` exceeds a configured request rate limit.\n",
        "  Set requests_per_second_limit to 0 for every request.\n",
    );
    let hung_out = format_text(already_hung, &cfg).unwrap();
    assert_eq!(
        hung_out, already_hung,
        "already-hung role item must stay identity, got:\n{hung_out}"
    );
}
