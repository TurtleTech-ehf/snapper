use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #89 / snapper-zqew: RST option-list gutter must survive SemBr.
#[test]
fn option_list_description_hangs_at_option_column() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    };
    let input = concat!(
        "-a            Output all. Keep this aligned.\n",
        "--long        Long option. Another sentence.\n",
    );
    let out = format_text(input, &cfg).unwrap();
    assert_eq!(
        out,
        concat!(
            "-a            Output all.\n",
            "              Keep this aligned.\n",
            "--long        Long option.\n",
            "              Another sentence.\n",
        ),
        "option descriptions must hang at the option column, got:\n{out}"
    );
    let twice = format_text(&out, &cfg).unwrap();
    assert_eq!(
        out, twice,
        "hung option list must be identity, got:\n{twice}"
    );
}

/// Wrap must not collapse the two-or-more spaces between option and description.
#[test]
fn wrap_does_not_eat_option_list_gutter() {
    let cfg = FormatConfig {
        format: Format::Rst,
        max_width: 36,
        ..Default::default()
    };
    let input = "-a            Output all extra words that force a wrap here.\n";
    let out = format_text(input, &cfg).unwrap();
    assert!(
        out.contains("-a            "),
        "wrap must keep the two-or-more option gutter, got:\n{out}"
    );
    assert!(
        !out.contains("-a Output"),
        "wrap must not collapse option pad to one space, got:\n{out}"
    );
}
