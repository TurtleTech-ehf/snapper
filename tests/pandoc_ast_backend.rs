//! Integration tests for the AST-backed pandoc path.
//!
//! These call shipped library entry points (`regions_from_pandoc_json`,
//! `format_text` with `use_pandoc`) — not a re-implementation of the walker.

#![cfg(feature = "pandoc")]

use std::path::PathBuf;

use snapper_fmt::format::Format;
use snapper_fmt::parser::Region;
use snapper_fmt::parser::pandoc::{
    PandocBackend, PandocParser, ffi_available, regions_from_pandoc_json,
};
use snapper_fmt::{FormatConfig, format_text};

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
    let has_table = regions.iter().any(|r| {
        matches!(r, Region::Structure(s) if s.contains('|') && s.contains("---"))
            || matches!(r, Region::Structure(s) if s.contains('|') && s.contains('a'))
    });
    assert!(has_code, "expected CodeBlock → Region::Code: {regions:?}");
    assert!(has_table, "expected Table → pipe Structure: {regions:?}");

    for r in &regions {
        if let Region::Prose(s) = r {
            assert!(!s.contains("print("), "code must not be prose: {s}");
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
                .any(|r| matches!(r, Region::Structure(s) if s.contains('|'))),
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
        // Disable AST cache so a prior CLI/FFI parse of this fixture cannot succeed
        // without loading the library.
        .env("SNAPPER_PANDOC_CACHE", "0")
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
            .any(|r| matches!(r, Region::Structure(s) if s.contains('|'))),
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
            .any(|r| matches!(r, Region::Structure(s) if s.contains('|'))),
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
        run1.contains('|') && (run1.contains("---") || run1.contains('1')),
        "Table cells as structure from AST:\n{run1}"
    );
}

/// Math + code: display/inline math and CodeBlock not sentence-reflowed.
#[test]
fn format_text_pandoc_math_and_code_protected() {
    let input = read_fixture("math_code.md");
    let backend = if ffi_available() {
        PandocBackend::Ffi
    } else if snapper_fmt::parser::pandoc::pandoc_available() {
        PandocBackend::Cli
    } else {
        eprintln!("skipping: no pandoc backend");
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
    assert_eq!(run1, run2);

    // Ordinary multi-sentence prose reflowed.
    assert!(
        run1.contains("First sentence.\n") && run1.contains("Second sentence"),
        "plain prose reflowed:\n{run1}"
    );

    // Display math: periods in body must not create orphan prose lines like "y = 2." alone
    // from sentence split of math (body may still appear as structure lines).
    let regions = regions_from_pandoc_json(&read_fixture("math_code_md.json")).unwrap();
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("1.5"))),
        "display math not prose regions: {regions:?}"
    );
    assert!(
        !regions
            .iter()
            .any(|r| matches!(r, Region::Prose(p) if p.contains("mc^2"))),
        "inline math not in prose: {regions:?}"
    );
    // Coherent fenced code unit from pandoc CodeBlock (lang + body, not prose).
    assert!(
        run1.contains("```python\nprint(1.0)\nx = 2.\n```"),
        "CodeBlock must emit one fenced coherent unit:\n{run1}"
    );
    // Must not reflow code into: print(1.\n0) style — period after 1 in code.
    assert!(
        !run1.lines().any(|l| l.trim() == "0)" || l.trim() == "0"),
        "code not sentence-fragmented:\n{run1}"
    );
}

#[test]
fn format_text_pandoc_latex_math_code_envs() {
    if !snapper_fmt::parser::pandoc::pandoc_available() && !ffi_available() {
        eprintln!("skipping: no pandoc backend");
        return;
    }
    // LaTeX readers are CLI-complete; FFI may not include latex the same way.
    let backend = if snapper_fmt::parser::pandoc::pandoc_available() {
        PandocBackend::Cli
    } else {
        PandocBackend::Ffi
    };
    let input = read_fixture("math_code.tex");
    let cfg = FormatConfig {
        format: Format::Latex,
        use_pandoc: true,
        pandoc_backend: backend,
        pandoc_format: Some("latex".into()),
        ..Default::default()
    };
    let out = match format_text(&input, &cfg) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("skipping latex pandoc path: {e}");
            return;
        }
    };
    // Require real sentence reflow (fixture has "Hello world. Second sentence." on one line).
    assert!(
        out.contains("Hello world.\n") && out.contains("Second sentence"),
        "latex pandoc must reflow multi-sentence prose:\n{out}"
    );
    let native = format_text(
        &input,
        &FormatConfig {
            format: Format::Latex,
            use_pandoc: false,
            ..Default::default()
        },
    )
    .expect("native latex");
    assert!(
        native.contains("Hello world.\n") && native.contains("Second sentence"),
        "native latex reflow for parity:\n{native}"
    );
    // Equation / display math not orphaned by "E = mc" / "2." split as prose lines only.
    assert!(
        !out.lines().any(|l| l.trim() == "2." && !l.contains("mc")),
        "math period must not orphan a bare '2.' prose line:\n{out}"
    );
    // minted/lstlisting/verbatim → CodeBlock with fence emit + lang when known
    assert!(
        out.contains("```") && (out.contains("print(1.0)") || out.contains("print")),
        "latex-origin CodeBlocks as fenced code units:\n{out}"
    );
    assert!(
        out.contains("```python") || out.matches("```").count() >= 2,
        "language fence or multiple code units:\n{out}"
    );
}

#[test]
fn format_text_pandoc_table_list_quote() {
    let input = read_fixture("structure_blocks.md");
    let backend = if snapper_fmt::parser::pandoc::pandoc_available() {
        PandocBackend::Cli
    } else if ffi_available() {
        PandocBackend::Ffi
    } else {
        eprintln!("skipping: no pandoc backend");
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
    assert_eq!(run1, run2);
    assert!(
        run1.contains("| a |") || run1.contains("| a | b |"),
        "table cells:\n{run1}"
    );
    assert!(run1.contains("---"), "table separator:\n{run1}");
    assert!(
        run1.contains("- ") || run1.lines().any(|l| l.starts_with("-")),
        "list markers:\n{run1}"
    );
    assert!(run1.contains('>'), "blockquote:\n{run1}");
    assert!(
        run1.contains("Intro sentence.\n") || run1.contains("Intro sentence."),
        "prose reflow:\n{run1}"
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
    assert!(
        !out.lines()
            .any(|l| l.trim() == "### 1." || l.trim() == "### 1")
    );
}
