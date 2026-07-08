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

/// Real CLI entry: selected FFI backend with a bad library path must error
/// (not print reflowed all-prose success). Uses a subprocess so process-global
/// FFI OnceLock state from other tests cannot mask the failure.
#[test]
fn snapper_cli_ffi_bad_lib_is_explicit_error() {
    let bin = env!("CARGO_BIN_EXE_snapper");
    let input = fixture("numbered_heading.md");
    let out = std::process::Command::new(bin)
        .args([
            "--use-pandoc",
            "--pandoc-backend",
            "ffi",
            "--format",
            "markdown",
        ])
        .arg(&input)
        .env("SNAPPER_PANDOC_LIB", "/nonexistent/libsnapper_pandoc.so")
        .env_remove("SNAPPER_PANDOC_LIB_DIR")
        .output()
        .expect("spawn snapper");
    assert!(
        !out.status.success(),
        "FFI with missing lib must fail, stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("unavailable") || err.contains("FFI") || err.contains("library"),
        "expected explicit FFI error on stderr, got: {err}"
    );
    // Must not look like successful reflow of the whole file as prose.
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty()
            || (!stdout.contains("Hello world.") && !stdout.contains("cargo binstall")),
        "must not emit all-prose success output on FFI failure: {stdout}"
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
    // Header title from AST (no requirement for invented ### markers).
    assert!(
        run1.to_lowercase().contains("title"),
        "Header title text should remain after pandoc+snapper: {run1}"
    );
    assert!(
        run1.contains("print") || run1.contains("python"),
        "CodeBlock body should remain (not sentence-reflowed away): {run1}"
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

/// Pandoc parse first: Header node → Structure; title never Prose (no reflow).
#[test]
fn numbered_heading_after_pandoc_parse_not_prose() {
    let json = read_fixture("numbered_heading.json");
    let regions = regions_from_pandoc_json(&json).expect("json");
    assert!(
        regions.iter().any(|r| {
            matches!(
                r,
                Region::Structure(s) if s.contains("1.") && s.contains("cargo binstall")
            )
        }),
        "Header must be Structure from AST, got: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("cargo binstall"))),
        "title must not be Prose: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(r, Region::Prose(_))),
        "body Para remain prose for snapper: {regions:?}"
    );
    assert!(
        regions.iter().any(|r| matches!(r, Region::Code { .. })),
        "CodeBlock non-prose: {regions:?}"
    );
    assert!(
        regions
            .iter()
            .any(|r| matches!(r, Region::Structure(s) if s.contains("[table]"))),
        "Table non-prose: {regions:?}"
    );
}

/// End-to-end: pandoc parse → snapper reflow on prose only.
#[test]
fn format_text_pandoc_then_snapper_numbered_header_stable() {
    let input = read_fixture("numbered_heading.md");
    let backend = if ffi_available() {
        PandocBackend::Ffi
    } else if snapper_fmt::parser::pandoc::pandoc_available() {
        PandocBackend::Cli
    } else {
        eprintln!("skipping: neither FFI lib nor pandoc CLI available");
        return;
    };
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: true,
        pandoc_backend: backend,
        pandoc_format: Some("markdown".into()),
        ..Default::default()
    };
    let run1 = format_text(&input, &cfg).expect("run1");
    let run2 = format_text(&input, &cfg).expect("run2");
    assert_eq!(run1, run2, "stable across two runs");

    let heading_line = run1
        .lines()
        .find(|l| l.contains("cargo binstall"))
        .expect("heading title present after pandoc+snapper");
    assert!(
        heading_line.contains("1.") && heading_line.contains("cargo binstall"),
        "Header title not sentence-split: {heading_line:?}\nfull:\n{run1}"
    );
    // Success is node-kind based — do not require invented ATX `###` markers.
    assert!(
        !heading_line.trim_start().starts_with("###"),
        "pandoc path must not invent ATX markers as structure proof: {heading_line:?}"
    );
    assert!(
        !run1.lines().any(|l| {
            let t = l.trim();
            t == "1." || t == "`cargo binstall` (preferred binary install)"
        }),
        "title must not be split by snapper reflow:\n{run1}"
    );

    // Body Para was reflowed: multi-sentence prose becomes separate lines.
    assert!(
        run1.contains("Hello world.\n") && run1.contains("Second sentence"),
        "prose Para must be reflowed by snapper:\n{run1}"
    );
    assert!(
        run1.contains("print") || run1.contains("python"),
        "code preserved: {run1}"
    );
    assert!(
        run1.contains("[table]"),
        "Table stays non-prose marker from AST: {run1}"
    );
}

/// Native path (no pandoc) still has its own ATX source-line contract.
#[test]
fn default_path_numbered_atx_source_line_not_split() {
    let input = read_fixture("numbered_heading.md");
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: false,
        ..Default::default()
    };
    let out = format_text(&input, &cfg).expect("default format");
    assert!(
        out.lines()
            .next()
            .is_some_and(|l| l == "### 1. `cargo binstall` (preferred binary install)"),
        "native path keeps full ATX source line, got:\n{out}"
    );
    assert!(!out.lines().any(|l| l.trim() == "### 1." || l.trim() == "### 1"));
}
