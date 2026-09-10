use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #90 / snapper-g2od: RST doctest blocks must not reflow as prose.
fn doctest_fixture() -> &'static str {
    concat!(
        ">>> print('Python-specific usage examples; begun with \">>> \"')\n",
        "Python-specific usage examples; begun with \">>> \"\n",
        ">>> print('(cut and pasted from interactive Python sessions)')\n",
        "(cut and pasted from interactive Python sessions)\n",
    )
}

#[test]
fn doctest_block_is_identity_under_sembr() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = doctest_fixture();
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out, input,
        "doctest prompt, output, and next >>> must stay separate lines, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(out, twice, "doctest block must be identity, got:\n{twice}");
}

/// Wrap must not glue the prompt onto the output or the next `>>>`.
#[test]
fn wrap_does_not_join_doctest_prompt_and_output() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 40,
        ..Default::default()
    };
    let input = doctest_fixture();
    let out = format_text(input, &cfg).unwrap();
    assert!(
        out.contains(">>> print('Python-specific usage examples; begun with \">>> \"')"),
        "first prompt must stay intact, got:\n{out}"
    );
    assert!(
        out.contains(">>> print('(cut and pasted from interactive Python sessions)')"),
        "second prompt must stay intact, got:\n{out}"
    );
    assert!(
        !out.contains(">>> \"') Python-specific"),
        "wrap must not join prompt onto output, got:\n{out}"
    );
    assert!(
        !out.contains(">>> \" >>> print"),
        "wrap must not join output onto the next >>>, got:\n{out}"
    );
}
