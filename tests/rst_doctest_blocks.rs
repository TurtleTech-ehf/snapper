use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

/// GitHub #90 / snapper-g2od: RST doctest blocks must not reflow as prose.
fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
}

fn ticket_fixture() -> &'static str {
    concat!(
        ">>> print('Python-specific usage examples; begun with \">>> \"')\n",
        "Python-specific usage examples; begun with \">>> \"\n",
        ">>> print('(cut and pasted from interactive Python sessions)')\n",
        "(cut and pasted from interactive Python sessions)\n",
    )
}

#[test]
fn doctest_block_is_structure_not_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains(">>> print('Python-specific usage examples")
                    && s.contains("Python-specific usage examples; begun with")
                    && s.contains(">>> print('(cut and pasted")
                    && s.contains("(cut and pasted from interactive Python sessions)")
        )),
        "doctest block must be Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains(">>>") || s.contains("Python-specific")
        )),
        "doctest block must not be Prose, got {regions:?}"
    );
}

#[test]
fn doctest_block_is_identity_under_format() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "doctest prompt, output, and next >>> must stay unjoined, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "hung doctest must be identity, got:\n{out}"
    );
}

/// GitHub #90 leftover / snapper-ijxt: prompt-only `>>>` is Body.doctest,
/// not a `>` section underline. Fails on origin/main (opener stolen,
/// output reflows as Prose) then passes.
fn prompt_only_fixture() -> &'static str {
    concat!(
        ">>>\n",
        "Python-specific usage examples; begun with \">>> \"\n",
        ">>> print('(cut and pasted from interactive Python sessions)')\n",
        "(cut and pasted from interactive Python sessions)\n",
    )
}

#[test]
fn prompt_only_doctest_is_structure_not_underline() {
    let regions = RstParser.parse(prompt_only_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.trim() == ">>>")),
        "prompt-only >>> at EOL must be Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("Python-specific usage examples; begun with")
        )),
        "text after empty >>> must stay Prose, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Structure(s)
                if s.contains(">>> print('(cut and pasted")
                    && s.contains("(cut and pasted from interactive Python sessions)")
        )),
        "same-line >>> print must still open a Structure block, got {regions:?}"
    );
}

#[test]
fn prompt_only_doctest_is_identity_under_format() {
    let input = prompt_only_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out, input,
        "prompt-only >>> must stay identity under format, got:\n{out}"
    );
    assert!(
        !out.contains(">>> Python-specific"),
        "must not join prompt-only >>> onto the output line, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "prompt-only doctest identity must survive a second pass"
    );
}
