//! snapper-awlr / GitHub #430: leftover body.py `line-block` same-line
//! body is leftover Prose. Marker stays Structure; After. still splits
//! and hangs. Flush After. / Next. still split. `| ` line-blocks already
//! end at flush. parsed-literal / header / footer unchanged.

use snapper_fmt::format::Format;
use snapper_fmt::parser::rst::RstParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn rst_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Rst,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Rst / GitHub #430).
fn ticket_fixture() -> &'static str {
    concat!(".. line-block:: fig. 1 is here. After.\n", "After. Next.\n",)
}

fn expected_ticket() -> &'static str {
    concat!(
        ".. line-block:: fig. 1 is here.\n",
        "                After.\n",
        "After.\n",
        "Next.\n",
    )
}

#[test]
fn line_block_directive_opener_is_structure_body_is_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ".. line-block:: ")),
        "opener .. line-block:: must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("fig. 1 is here.") && s.contains("After.")
        )),
        "same-line line-block body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("fig. 1 is here")
        )),
        "same-line line-block body must not stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_hangs_and_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert!(
        !out.contains(".. line-block:: fig. 1 is here. After."),
        "same-line line-block body must not stay one line, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext.\n"),
        "flush After. / Next. must stay split, got:\n{out}"
    );
    assert!(
        !out.contains("\n                After.\n                After."),
        "flush After. must not inherit the line-block hang, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "split line-block body must be identity, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn pipe_line_blocks_still_end_at_flush() {
    let input = concat!("| First line. Second line.\n", "After. Next.\n",);
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!("| First line.\n", "  Second line.\n", "After.\n", "Next.\n",),
        "| line-blocks already end at flush; After. / Next. still split, got:\n{out}"
    );
    assert!(
        !out.contains("\n  After."),
        "After. must not inherit the | hang, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn parsed_literal_header_footer_unchanged() {
    let parsed = concat!(
        ".. parsed-literal:: fig. 1 is here. After.\n",
        "After. Next.\n",
    );
    let parsed_out = format_text(parsed, &rst_cfg()).unwrap();
    assert_eq!(
        parsed_out,
        concat!(
            ".. parsed-literal:: fig. 1 is here.\n",
            "                    After.\n",
            "After.\n",
            "Next.\n",
        ),
        "parsed-literal leftover from #426 must stay, got:\n{parsed_out}"
    );

    for name in ["header", "footer"] {
        let opener = format!(".. {name}:: ");
        let hang = " ".repeat(opener.len());
        let input = format!(".. {name}:: fig. 1 is here. After.\nAfter. Next.\n");
        let out = format_text(&input, &rst_cfg()).unwrap();
        assert_eq!(
            out,
            format!("{opener}fig. 1 is here.\n{hang}After.\nAfter.\nNext.\n"),
            "{name} same-line leftover must stay, got:\n{out}"
        );
        assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
    }
}
