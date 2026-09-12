//! GitHub #355 / snapper-1zw3: unmatched org-element openers are paragraphs.
//! `\begin{equation}` / `#+BEGIN_SRC` / `#+BEGIN_EXPORT` without a closer
//! must not swallow following prose to EOF. `After the env.` / `Next.`
//! stay Prose and still split. Closed environments / BEGIN_EXPORT /
//! BEGIN_SRC stay Structure (or Code) through the closer.

use snapper_fmt::format::Format;
use snapper_fmt::parser::org::OrgParser;
use snapper_fmt::parser::{FormatParser, Region};
use snapper_fmt::{FormatConfig, format_text};

fn org_cfg() -> FormatConfig {
    FormatConfig {
        format: Format::Org,
        max_width: 0,
        ..Default::default()
    }
    .without_safety_backstops()
}

/// Ticket fixture (Format::Org / GitHub #355).
fn ticket_fixture() -> &'static str {
    concat!("\\begin{equation}\n", "x = 1.\n", "After the env. Next.\n",)
}

fn export_fixture() -> &'static str {
    concat!(
        "#+BEGIN_EXPORT html\n",
        "<p>Hello. World.</p>\n",
        "After the env. Next.\n",
    )
}

fn src_fixture() -> &'static str {
    concat!(
        "#+BEGIN_SRC python\n",
        "print(1)\n",
        "After the env. Next.\n",
    )
}

fn assert_after_next_prose_and_split(input: &str, out: &str, regions: &[Region]) {
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s) if s.contains("After the env.") && s.contains("Next.")
        )),
        "After the env. / Next. must stay Prose, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("After the env."))),
        "After the env. must not freeze as Structure, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Code { body, .. } if body.contains("After the env.")
        )),
        "After the env. must not freeze as Code, got {regions:?}"
    );
    assert!(
        out.contains("After the env.\nNext."),
        "After the env. / Next. must still split, got:\n{out}"
    );
    assert!(
        !out.contains("After the env. Next."),
        "fused trailing prose must not survive, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);

    let guarded = FormatConfig {
        format: Format::Org,
        ..Default::default()
    };
    let guarded_out = format_text(input, &guarded).unwrap();
    assert_eq!(
        guarded_out, out,
        "oracle-on path must match, got:\n{guarded_out}"
    );
}

#[test]
fn unmatched_latex_begin_is_paragraph() {
    let input = ticket_fixture();
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("\\begin{equation}")
                    && s.contains("x = 1.")
                    && s.contains("After the env.")
                    && s.contains("Next.")
        )),
        "unmatched \\begin{{equation}} is a paragraph, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("\\begin{equation}"))),
        "unmatched \\begin must not be Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_after_next_prose_and_split(input, &out, &regions);
}

#[test]
fn unmatched_begin_export_is_paragraph() {
    let input = export_fixture();
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("#+BEGIN_EXPORT html")
                    && s.contains("After the env.")
                    && s.contains("Next.")
        )),
        "unmatched #+BEGIN_EXPORT is a paragraph, got {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_EXPORT"))),
        "unmatched #+BEGIN_EXPORT must not be Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_after_next_prose_and_split(input, &out, &regions);
}

#[test]
fn unmatched_begin_src_is_paragraph() {
    let input = src_fixture();
    let regions = OrgParser.parse(input);
    assert!(
        regions.iter().any(|r| matches!(
            r,
            Region::Prose(s)
                if s.contains("#+BEGIN_SRC python")
                    && s.contains("After the env.")
                    && s.contains("Next.")
        )),
        "unmatched #+BEGIN_SRC is a paragraph, got {regions:?}"
    );
    assert!(
        !regions.iter().any(|r| matches!(
            r,
            Region::Code { header, .. } if header.contains("#+BEGIN_SRC")
        )),
        "unmatched #+BEGIN_SRC must not be Code, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert_after_next_prose_and_split(input, &out, &regions);
}

#[test]
fn closed_latex_env_still_structure() {
    let input = concat!(
        "\\begin{equation}\n",
        "x = 1.\n",
        "\\end{equation}\n",
        "After the env. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("\\begin{equation}"))),
        "closed \\begin{{equation}} stays Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("x = 1."))),
        "closed equation body stays Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("\\begin{equation}\nx = 1.\n\\end{equation}\nAfter the env.\nNext."),
        "closed env frozen; After. / Next. still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn closed_begin_export_still_structure() {
    let input = concat!(
        "#+BEGIN_EXPORT html\n",
        "<p>Hello. World.</p>\n",
        "#+END_EXPORT\n",
        "After the env. Next.\n",
    );
    let regions = OrgParser.parse(input);
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("#+BEGIN_EXPORT"))),
        "closed #+BEGIN_EXPORT stays Structure, got {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("<p>Hello. World.</p>"))),
        "closed export body stays Structure, got {regions:?}"
    );
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains(
            "#+BEGIN_EXPORT html\n<p>Hello. World.</p>\n#+END_EXPORT\nAfter the env.\nNext."
        ),
        "closed EXPORT frozen; After. / Next. still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}

#[test]
fn unmatched_lowercase_src_and_export_are_paragraphs() {
    for (label, input) in [
        (
            "begin_src",
            "#+begin_src python\nprint(1)\nAfter the env. Next.\n",
        ),
        (
            "begin_export",
            "#+begin_export html\n<p>Hello. World.</p>\nAfter the env. Next.\n",
        ),
    ] {
        let regions = OrgParser.parse(input);
        let out = format_text(input, &org_cfg()).unwrap();
        assert_after_next_prose_and_split(input, &out, &regions);
        assert!(
            !regions.iter().any(|r| matches!(
                r,
                Region::Structure(s) if s.to_ascii_uppercase().contains("BEGIN_SRC")
                    || s.to_ascii_uppercase().contains("BEGIN_EXPORT")
            )),
            "unmatched lowercase {label} must not be Structure, got {regions:?}"
        );
        assert!(
            !regions.iter().any(|r| matches!(r, Region::Code { .. })),
            "unmatched lowercase {label} must not be Code, got {regions:?}"
        );
    }
}

#[test]
fn closed_begin_src_still_code() {
    let input = concat!(
        "#+BEGIN_SRC python\n",
        "print(1)\n",
        "#+END_SRC\n",
        "After the env. Next.\n",
    );
    let regions = OrgParser.parse(input);
    match regions.iter().find(|r| matches!(r, Region::Code { .. })) {
        Some(Region::Code {
            lang,
            header,
            body,
            footer,
        }) => {
            assert_eq!(lang.as_deref(), Some("python"));
            assert_eq!(header, "#+BEGIN_SRC python\n");
            assert_eq!(body, "print(1)\n");
            assert_eq!(footer, "#+END_SRC\n");
        }
        other => panic!("closed #+BEGIN_SRC must stay Code, got {other:?}"),
    }
    let out = format_text(input, &org_cfg()).unwrap();
    assert!(
        out.contains("#+BEGIN_SRC python\nprint(1)\n#+END_SRC\nAfter the env.\nNext."),
        "closed SRC frozen; After. / Next. still split, got:\n{out}"
    );
    assert_eq!(format_text(&out, &org_cfg()).unwrap(), out);
}
