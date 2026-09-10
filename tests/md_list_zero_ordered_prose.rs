use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// snapper-9dc1: `* 0. A.` hangs after `0.`; pulldown invents a nested
/// ordered list. Format and oracle agree on the sembr hang, not a nested
/// `<ol>`.
fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn star_zero_a_hangs_without_nested_ordered_list() {
    let input = "* 0. A.";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(out, "* 0.\n  A.", "sembr hang after 0., got:\n{out}");
    assert!(
        !out.contains("\n    "),
        "must not invent a nested ordered list, got:\n{out}"
    );
    assert!(
        !out.contains("0. A."),
        "must not keep 0. A. on one line, got:\n{out}"
    );
    let twice = format_text(&out, &md_cfg()).unwrap();
    assert_eq!(out, twice, "hung item must be identity, got:\n{twice}");
    assert!(
        snapper_fmt::oracle::matches(Format::Markdown, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );

    let guarded = FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must keep the hang, got:\n{guarded_out}"
    );
    let guarded_nl = format_text("* 0. A.\n", &guarded).unwrap();
    assert_eq!(
        guarded_nl, "* 0.\n  A.\n",
        "oracle-on path must keep the hung newline, got:\n{guarded_nl}"
    );
}

#[test]
fn dash_and_plus_zero_a_hang_match_oracle() {
    let cfg = md_cfg();
    for (input, expect) in [
        ("- 0. A.", "- 0.\n  A."),
        ("+ 0. A.", "+ 0.\n  A."),
        ("* 1. Hello.", "* 1.\n  Hello."),
    ] {
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(out, expect, "hang {input:?}, got:\n{out}");
        assert!(
            snapper_fmt::oracle::matches(Format::Markdown, input, &out),
            "oracle mismatch\n in={input:?}\n out={out:?}"
        );
    }
}
