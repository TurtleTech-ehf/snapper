//! `$…$` whose closer is preceded by `.` stays one span, as does
//! `$x = 3.14$`. A sentence outside the dollars still splits.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn dollar_closed_by_period_stays_whole_and_outside_splits() {
    let input = "See $See this. Then that.$ today. Next sentence.\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("$See this. Then that.$"),
        "period-closed math must stay one span, got:\n{out}"
    );
    assert!(
        !out.contains("$See this.\n"),
        "must not split inside the dollars, got:\n{out}"
    );
    assert!(
        out.contains("today.\nNext sentence.\n"),
        "a sentence outside the dollars must still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}

#[test]
fn decimal_dollar_math_stays_whole() {
    let input = "The value $x = 3.14$ matters. Next sentence.\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert!(
        out.contains("$x = 3.14$"),
        "decimal math must stay one span, got:\n{out}"
    );
    assert!(
        out.contains("matters.\nNext sentence.\n"),
        "a sentence outside the dollars must still split, got:\n{out}"
    );
}
