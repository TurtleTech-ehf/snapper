use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #126 / snapper-y8mf: a `:role:`text`` continuation is not a
/// field list, so the next sentence keeps the list-item hang.
#[test]
fn role_continuation_second_sentence_hangs_at_marker_width() {
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
        "second sentence must hang at `* ` width, got:\n{out}"
    );
    assert!(
        out.contains("\n  Set requests_per_second_limit"),
        "continuation must be two spaces, not column 0, got:\n{out}"
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

    let reporter = concat!(
        "* ``TooManyRequests`` is returned when a\n",
        "  :class:`mock_vws.database.CloudDatabase` exceeds a configured request rate\n",
        "  limit. Set ``requests_per_second_limit`` to ``0`` for every request.\n",
    );
    let reporter_out = format_text(reporter, &cfg).unwrap();
    assert!(
        reporter_out.contains("\n  Set ``requests_per_second_limit``"),
        "reporter second sentence must hang, got:\n{reporter_out}"
    );
    assert!(
        !reporter_out.contains("\nSet ``requests_per_second_limit``"),
        "reporter second sentence must not outdent, got:\n{reporter_out}"
    );
    let reporter_twice = format_text(&reporter_out, &cfg).unwrap();
    assert_eq!(
        reporter_out, reporter_twice,
        "hung reporter fixture must be identity, got:\n{reporter_twice}"
    );
}
