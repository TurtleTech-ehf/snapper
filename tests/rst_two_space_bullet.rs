use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #133 / snapper-7yj2: two spaces after an RST bullet are the
/// Docutils hang. Shrinking only the marker to `* ` turns a hang-aligned
/// body and nested list into a block quote.
#[test]
fn two_space_bullet_keeps_marker_and_nested_content() {
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
        out.starts_with("*  Candidate:"),
        "must keep two spaces after the marker, got:\n{out}"
    );
    assert!(
        out.contains("\n   Search API."),
        "item body must keep three-space hang, got:\n{out}"
    );
    assert!(
        out.contains("\n   * Child item."),
        "nested child must keep three-space indent, got:\n{out}"
    );
    assert!(
        !out.starts_with("* Candidate:"),
        "must not shrink only the marker, got:\n{out}"
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

    let split = "*  Candidate: First. Second.\n";
    let split_out = format_text(split, &cfg).unwrap();
    assert_eq!(
        split_out, "*  Candidate: First.\n   Second.\n",
        "sentence split must hang at two-space marker width, got:\n{split_out}"
    );
    let split_twice = format_text(&split_out, &cfg).unwrap();
    assert_eq!(
        split_out, split_twice,
        "hung two-space bullet must be identity, got:\n{split_twice}"
    );
}
