use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #91 / snapper-rvl2: RST alpha, roman, and (n) enumerators hang.
#[test]
fn alpha_roman_paren_enumerators_hang_at_marker_width() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "a. Alpha item. Second sentence.\n",
        "(1) Paren arabic. Second sentence.\n",
        "i. Roman item. Second sentence.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        concat!(
            "a. Alpha item.\n",
            "   Second sentence.\n",
            "(1) Paren arabic.\n",
            "    Second sentence.\n",
            "i. Roman item.\n",
            "   Second sentence.\n",
        ),
        "alpha/roman/paren enumerators must hang like 1. items, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung enumerators must be identity, got:\n{twice}"
    );
}
