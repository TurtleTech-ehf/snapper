//! snapper-p4xk / GitHub #426: leftover body.py `parsed-literal`
//! same-line body is leftover Prose. Marker stays Structure; After.
//! still splits and hangs. Flush After. / Next. still split.
//! header / footer / replace:: / epigraph unchanged.

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

/// Ticket fixture (Format::Rst / GitHub #426).
fn ticket_fixture() -> &'static str {
    concat!(
        ".. parsed-literal:: fig. 1 is here. After.\n",
        "After. Next.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        ".. parsed-literal:: fig. 1 is here.\n",
        "                    After.\n",
        "After.\n",
        "Next.\n",
    )
}

#[test]
fn parsed_literal_opener_is_structure_body_is_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ".. parsed-literal:: ")),
        "opener .. parsed-literal:: must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("fig. 1 is here.") && s.contains("After.")
        )),
        "same-line parsed-literal body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("fig. 1 is here")
        )),
        "same-line parsed-literal body must not stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_hangs_and_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert!(
        !out.contains(".. parsed-literal:: fig. 1 is here. After."),
        "same-line parsed-literal body must not stay one line, got:\n{out}"
    );
    assert!(
        out.contains("After.\nNext.\n"),
        "flush After. / Next. must stay split, got:\n{out}"
    );
    assert!(
        !out.contains("\n                    After.\n                    After."),
        "flush After. must not inherit the parsed-literal hang, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "split parsed-literal body must be identity, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn indented_parsed_literal_body_still_hangs() {
    let input = concat!(
        ".. parsed-literal::\n",
        "\n",
        "   fig. 1 is here. After.\n",
        "After. Next.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. parsed-literal::\n",
            "\n",
            "   fig. 1 is here.\n",
            "   After.\n",
            "After.\n",
            "Next.\n",
        ),
        "indented parsed-literal body must hang and split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn header_footer_replace_epigraph_unchanged() {
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

    let epigraph = concat!(
        ".. epigraph::\n",
        "\n",
        "   This is a long note sentence that must reflow. Second sentence.\n",
        "After. Next.\n",
    );
    let epigraph_out = format_text(epigraph, &rst_cfg()).unwrap();
    assert_eq!(
        epigraph_out,
        concat!(
            ".. epigraph::\n",
            "\n",
            "   This is a long note sentence that must reflow.\n",
            "   Second sentence.\n",
            "After.\n",
            "Next.\n",
        ),
        "epigraph leftover from #351 must stay, got:\n{epigraph_out}"
    );

    let replace = concat!(
        "See |v|. Next.\n",
        "\n",
        ".. |v| replace:: fig. 1 is here. After.\n",
    );
    let replace_out = format_text(replace, &rst_cfg()).unwrap();
    assert_eq!(
        replace_out,
        concat!(
            "See |v|.\n",
            "Next.\n",
            "\n",
            ".. |v| replace:: fig. 1 is here.\n",
            "                 After.\n",
        ),
        "same-line replace leftover from #417 must stay, got:\n{replace_out}"
    );
}
