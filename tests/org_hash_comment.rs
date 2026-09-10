//! GitHub #177 / snapper-stms: Org hash comments require a space or EOL.
//! `#foo` is prose; `# comment` stays Structure.

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

#[test]
fn hash_comment_requires_space_or_eol() {
    let input = concat!(
        "#not-a-comment This is a long sentence that must reflow as prose. Second sentence.\n",
        "# This is a real comment and must stay frozen.\n",
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "#not-a-comment This is a long sentence that must reflow as prose.\n",
            "Second sentence.\n",
            "# This is a real comment and must stay frozen.\n",
        ),
        "#not-a-comment must split; real comment stays frozen, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
