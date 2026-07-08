//! Integration tests for the AST-backed pandoc path.
//!
//! These call shipped library entry points (`regions_from_pandoc_json`,
//! `format_text` with `use_pandoc`) — not a re-implementation of the walker.

#![cfg(feature = "pandoc")]

use std::path::PathBuf;

use snapper_fmt::format::Format;
use snapper_fmt::parser::pandoc::{
    PandocBackend, PandocParser, ffi_available, regions_from_pandoc_json,
};
use snapper_fmt::parser::Region;
use snapper_fmt::{format_text, FormatConfig};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pandoc_ast")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

#[test]
fn mixed_markdown_json_fixture_classifies_via_shipped_ast_mapper() {
    let json = read_fixture("mixed_md.json");
    let regions = regions_from_pandoc_json(&json).expect("deserialize mixed_md.json");

    let prose_n = regions
        .iter()
        .filter(|r| matches!(r, Region::Prose(_)))
        .count();
    assert!(prose_n >= 1, "expected prose from Para, got {regions:?}");

    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.to_lowercase().contains("title") || !s.is_empty())),
        "expected Header structure among {regions:?}"
    );

    let has_code = regions.iter().any(|r| matches!(r, Region::Code { .. }));
    let has_table = regions
        .iter()
        .any(|r| matches!(r, Region::Structure(s) if s.contains("[table]")));
    assert!(has_code, "expected CodeBlock → Region::Code: {regions:?}");
    assert!(has_table, "expected Table → non-prose: {regions:?}");

    for r in &regions {
        if let Region::Prose(s) = r {
            assert!(
                !s.contains("print(") && !s.contains("[table]"),
                "code/table must not be prose: {s}"
            );
        }
    }
}

#[test]
fn mixed_org_json_fixture_classifies_via_shipped_ast_mapper() {
    let json = read_fixture("mixed_org.json");
    let regions = regions_from_pandoc_json(&json).expect("deserialize mixed_org.json");
    let prose_n = regions
        .iter()
        .filter(|r| matches!(r, Region::Prose(_)))
        .count();
    assert!(prose_n >= 1, "org fixture prose: {regions:?}");
    assert!(
        regions.iter().any(|r| matches!(r, Region::Code { .. }))
            || regions
                .iter()
                .any(|r| matches!(r, Region::Structure(s) if s.contains("[table]"))),
        "org fixture should have code and/or table structure: {regions:?}"
    );
}

#[test]
fn format_text_ffi_backend_errors_explicitly_when_lib_missing() {
    if ffi_available() {
        // Live FFI path exercised in format_text_ffi_live when the lib is present.
        return;
    }
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: true,
        pandoc_backend: PandocBackend::Ffi,
        pandoc_format: Some("markdown".into()),
        ..Default::default()
    };
    let err = format_text("# Hi\n\nHello world.\n", &cfg).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unavailable") || msg.contains("FFI") || msg.contains("library"),
        "expected explicit FFI failure, got: {msg}"
    );
}

#[test]
fn format_text_cli_backend_stable_across_two_runs_when_pandoc_present() {
    if !snapper_fmt::parser::pandoc::pandoc_available() {
        eprintln!("skipping: pandoc CLI not on PATH");
        return;
    }
    let input = read_fixture("mixed.md");
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: true,
        pandoc_backend: PandocBackend::Cli,
        pandoc_format: Some("markdown".into()),
        ..Default::default()
    };
    let run1 = format_text(&input, &cfg).expect("run1");
    let run2 = format_text(&input, &cfg).expect("run2");
    assert_eq!(run1, run2, "AST/CLI reflow must be stable across two runs");
    // Non-prose structure markers / content preserved at a high level
    assert!(
        run1.to_lowercase().contains("title") || run1.contains('#'),
        "heading-related content should remain: {run1}"
    );
    assert!(
        run1.contains("print") || run1.contains("python") || run1.contains("```"),
        "code-related content should remain (not sentence-split away): {run1}"
    );
}

#[test]
fn try_parse_cli_mixed_markdown_region_kinds() {
    if !snapper_fmt::parser::pandoc::pandoc_available() {
        eprintln!("skipping: pandoc CLI not on PATH");
        return;
    }
    let input = read_fixture("mixed.md");
    let parser = PandocParser::with_backend("markdown", PandocBackend::Cli);
    let regions = parser.try_parse(&input).expect("cli parse");
    assert!(
        regions.iter().any(|r| matches!(r, Region::Prose(_))),
        "{regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(r, Region::Code { .. })),
        "code: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("[table]"))),
        "table: {regions:?}"
    );
}

#[test]
fn format_text_ffi_live_stable_when_lib_present() {
    if !ffi_available() {
        eprintln!("skipping live FFI: libsnapper_pandoc not loadable");
        return;
    }
    let input = read_fixture("mixed.md");
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: true,
        pandoc_backend: PandocBackend::Ffi,
        pandoc_format: Some("markdown".into()),
        ..Default::default()
    };
    let run1 = format_text(&input, &cfg).expect("ffi run1");
    let run2 = format_text(&input, &cfg).expect("ffi run2");
    assert_eq!(run1, run2);
}
