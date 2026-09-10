//! GitHub #170 / snapper-62xn: sentence punct inside a markdown `](url)` tail.
//!
//! `See [the docs.](http://x.com)\niCloud` must stay two lines under
//! `format_text` `Format::Markdown` `max_width=0`.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
}

#[test]
fn markdown_link_tail_keeps_break_before_icloud() {
    let input = "See [the docs.](http://x.com)\niCloud\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, input,
        "period inside [text](url) must keep the break, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
