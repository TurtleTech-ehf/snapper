use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #91 / snapper-rvl2: RST alpha, roman, and paren enumerators hang.
#[test]
fn alpha_roman_paren_enumerators_hang_like_arabic() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let cases = [
        (
            "a. Alpha item. Second sentence.\n",
            "a. Alpha item.\n   Second sentence.\n",
        ),
        (
            "(1) Paren arabic. Second sentence.\n",
            "(1) Paren arabic.\n    Second sentence.\n",
        ),
        (
            "i. Roman item. Second sentence.\n",
            "i. Roman item.\n   Second sentence.\n",
        ),
    ];
    for (input, want) in cases {
        let out = format_text(input, &cfg).unwrap();
        assert_eq!(
            out, want,
            "enumerator must hang at marker width, got:\n{out}"
        );
        let twice = format_text(&out, &cfg).unwrap();
        assert_eq!(
            out, twice,
            "hung enumerator must be identity, got:\n{twice}"
        );
    }
}
