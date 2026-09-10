//! snapper-62xn / GitHub #170: join gap misses sentence punct inside
//! markdown link tails.
//!
//! `See [the docs.](http://x.com)\niCloud` must stay two lines under
//! `format_text` `Format::Markdown` `max_width=0`.

use snapper_fmt::format::Format;
use snapper_fmt::{format_text, FormatConfig};

fn md_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        max_width: 0,
        ..Default::default()
    }
}

#[test]
fn markdown_link_tail_icloud_stays_two_lines() {
    let input = "See [the docs.](http://x.com)\niCloud\n";
    let out = format_text(input, &md_cfg()).unwrap();
    assert_eq!(
        out, input,
        "must keep break after link-tail sentence, got:\n{out}"
    );
    assert_eq!(format_text(&out, &md_cfg()).unwrap(), out);
}
