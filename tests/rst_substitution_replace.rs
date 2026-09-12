//! snapper-nt0s / GitHub #417: RST SubstitutionDef
//! `.. |name| replace::` same-line body is leftover Prose.
//! Marker stays Structure; After. still splits. See |v|. / Next.
//! unchanged. Isolate onto origin/main.

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

/// Ticket fixture (Format::Rst / GitHub #417).
fn ticket_fixture() -> &'static str {
    concat!(
        "See |v|. Next.\n",
        "\n",
        ".. |v| replace:: fig. 1 is here. After.\n",
    )
}

fn expected_ticket() -> &'static str {
    concat!(
        "See |v|.\n",
        "Next.\n",
        "\n",
        ".. |v| replace:: fig. 1 is here.\n",
        "                 After.\n",
    )
}

#[test]
fn substitution_replace_opener_is_structure_body_is_prose() {
    let regions = RstParser.parse(ticket_fixture());
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s == ".. |v| replace:: ")),
        "opener .. |v| replace:: must stay Structure, got {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("fig. 1 is here.") && s.contains("After.")
        )),
        "same-line replace body must be Prose, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Structure(s) if s.contains("fig. 1 is here")
        )),
        "same-line replace body must not stay whole-line Structure, got {regions:?}"
    );
}

#[test]
fn ticket_fixture_hangs_and_splits() {
    let input = ticket_fixture();
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(out, expected_ticket(), "got:\n{out}");
    assert!(
        !out.contains(".. |v| replace:: fig. 1 is here. After."),
        "same-line replace body must not stay one line, got:\n{out}"
    );
    assert!(
        out.contains("See |v|.\nNext.\n"),
        "See |v|. / Next. must stay split, got:\n{out}"
    );
    assert_eq!(
        format_text(&out, &rst_cfg()).unwrap(),
        out,
        "split replace body must be identity, got:\n{out}"
    );
    assert!(
        snapper_fmt::oracle::matches(Format::Rst, input, &out),
        "oracle mismatch\n in={input:?}\n out={out:?}"
    );
}

#[test]
fn image_substitution_stays_opaque() {
    let input = ".. |v| image:: fig.png\n";
    let regions = RstParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains(".. |v| image:: fig.png"))),
        "image substitution must stay Structure, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(s) if s.contains("fig.png"))),
        "image URI must not become Prose, got {regions:?}"
    );
    assert_eq!(format_text(input, &rst_cfg()).unwrap(), input);
}

#[test]
fn indented_replace_body_still_hangs() {
    let input = concat!(
        ".. |v| replace::\n",
        "\n",
        "   fig. 1 is here. After.\n",
        "\n",
        "After markup. Next.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. |v| replace::\n",
            "\n",
            "   fig. 1 is here.\n",
            "   After.\n",
            "\n",
            "After markup.\n",
            "Next.\n",
        ),
        "indented replace body must hang and split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &rst_cfg()).unwrap(), out);
}

#[test]
fn substitution_refs_still_split_as_prose() {
    let input = concat!(
        "|version| is the current release. Second sentence.\n",
        "\n",
        "The current release is\n",
        "|version|. Next sentence.\n",
    );
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            "|version| is the current release.\n",
            "Second sentence.\n",
            "\n",
            "The current release is |version|.\n",
            "Next sentence.\n",
        ),
        "substitution-ref prose from #233 must stay, got:\n{out}"
    );
}

#[test]
fn same_line_note_still_hangs() {
    let input = ".. note:: This is a long note sentence that must reflow. Second sentence.\n";
    let out = format_text(input, &rst_cfg()).unwrap();
    assert_eq!(
        out,
        concat!(
            ".. note:: This is a long note sentence that must reflow.\n",
            "          Second sentence.\n",
        ),
        "same-line note leftover from #349 must stay, got:\n{out}"
    );
}
