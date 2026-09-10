use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #133 / snapper-7yj2: two spaces after an RST bullet stay in
/// the hang. Shrinking `*  Candidate:` to `* Candidate:` leaves the
/// three-space body and child, which Docutils treats as a block quote.
#[test]
fn two_space_bullet_keeps_marker_and_nested_indent() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "*  Candidate:\n",
        "\n",
        "   Search API.\n",
        "\n",
        "   * Child item.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "two-space bullet must stay identity, got:\n{out}"
    );
    assert!(
        out.contains("*  Candidate:"),
        "marker must keep two spaces after *, got:\n{out}"
    );
    assert!(
        !out.contains("* Candidate:"),
        "must not shrink the marker to one space, got:\n{out}"
    );
    assert!(
        out.contains("\n   Search API."),
        "body must keep three-space indent, got:\n{out}"
    );
    assert!(
        out.contains("\n   * Child item."),
        "child must keep three-space indent, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "two-space bullet must be identity, got:\n{twice}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}
