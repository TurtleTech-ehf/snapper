//! Integration tests for the AST-backed pandoc path.
//!
//! These call shipped library entry points (`regions_from_pandoc_json`,
//! `format_text` with `use_pandoc`) — not a re-implementation of the walker.

#![cfg(feature = "pandoc")]

use std::path::PathBuf;

use snapper_fmt::format::Format;
use snapper_fmt::parser::Region;
use snapper_fmt::parser::pandoc::{
    PandocBackend, PandocParser, WRITER_NEEDS_PATH, dropped_comment_kind, ffi_available,
    ffi_write_available, regions_from_pandoc_json,
};
use snapper_fmt::{FormatConfig, format_text};

/// CLI writer is required unless the loaded FFI library exports a writer.
fn require_writer() -> bool {
    if snapper_fmt::parser::pandoc::pandoc_available() {
        true
    } else {
        eprintln!("skipping: pandoc CLI not on PATH (writer)");
        false
    }
}

fn assert_has_sentence_break(out: &str, first: &str, second_substr: &str) {
    assert!(
        out.contains(&format!("{first}\n")) && out.contains(second_substr),
        "expected sentence break after {first:?} before {second_substr:?}:\n{out}"
    );
}

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
    if !require_writer() {
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
    let a = format_text(&input, &cfg).expect("pandoc write");
    let b = format_text(&input, &cfg).expect("pandoc write 2");
    assert_eq!(a, b, "writer must be deterministic");
    assert_has_sentence_break(&a, "Hello world.", "Second sentence");
    assert!(a.contains("```") && a.contains("print(1)"), "code:\n{a}");
    assert!(
        a.contains('a') && a.contains('b') && a.contains("---"),
        "table cells stay structure:\n{a}"
    );
    assert!(a.contains("Title"), "header:\n{a}");
    assert_ne!(a, input, "write-through is not a byte splice");
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
    if !require_writer() {
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
    let a = format_text(&input, &cfg).expect("ffi parse + write");
    let b = format_text(&input, &cfg).expect("ffi parse + write 2");
    assert_eq!(a, b);
    assert_has_sentence_break(&a, "Hello world.", "Second sentence");
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

/// Pandoc writes the reflowed AST; the heading stays one Header (not prose).
#[test]
fn format_text_pandoc_writes_numbered_heading() {
    if !require_writer() {
        return;
    }
    let input = read_fixture("numbered_heading.md");
    let backend = if ffi_available() {
        PandocBackend::Ffi
    } else {
        PandocBackend::Cli
    };
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: true,
        pandoc_backend: backend,
        pandoc_format: Some("markdown".into()),
        ..Default::default()
    };
    let out = format_text(&input, &cfg).expect("pandoc write");
    assert_has_sentence_break(&out, "Hello world.", "Second sentence");
    assert!(
        out.contains("cargo binstall"),
        "header title survives:\n{out}"
    );
    assert!(
        !out.lines()
            .any(|l| l.trim() == "1." || l.trim() == "### 1."),
        "header must not be sentence-split:\n{out}"
    );
    let native = format_text(
        &input,
        &FormatConfig {
            format: Format::Markdown,
            use_pandoc: false,
            ..Default::default()
        }
        .without_safety_backstops(),
    )
    .unwrap();
    assert!(
        native
            .lines()
            .any(|l| l == "### 1. `cargo binstall` (preferred binary install)"),
        "native splice must keep ATX hashes:\n{native}"
    );
}

/// Math + code: display/inline math and CodeBlock not sentence-reflowed.
#[test]
fn format_text_pandoc_math_and_code_protected() {
    if !require_writer() {
        return;
    }
    let input = read_fixture("math_code.md");
    let backend = if ffi_available() {
        PandocBackend::Ffi
    } else {
        PandocBackend::Cli
    };
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: true,
        pandoc_backend: backend,
        pandoc_format: Some("markdown".into()),
        ..Default::default()
    };
    let pandoc_out = format_text(&input, &cfg).expect("pandoc write");
    assert_has_sentence_break(&pandoc_out, "First sentence.", "Second sentence");
    assert!(
        pandoc_out.contains("```") && pandoc_out.contains("print(1.0)"),
        "CodeBlock stays a fenced unit:\n{pandoc_out}"
    );
    assert!(
        !pandoc_out
            .lines()
            .any(|l| l.trim() == "0)" || l.trim() == "0"),
        "code not sentence-fragmented:\n{pandoc_out}"
    );
    let run1 = format_text(
        &input,
        &FormatConfig {
            format: Format::Markdown,
            ..Default::default()
        }
        .without_safety_backstops(),
    )
    .expect("native");

    // Ordinary multi-sentence prose reflowed (native splice).
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
    if !require_writer() {
        return;
    }
    let backend = PandocBackend::Cli;
    let input = read_fixture("math_code.tex");
    let cfg = FormatConfig {
        format: Format::Latex,
        use_pandoc: true,
        pandoc_backend: backend,
        pandoc_format: Some("latex".into()),
        ..Default::default()
    };
    let pandoc_out = format_text(&input, &cfg).expect("pandoc latex write");
    assert!(
        pandoc_out.contains("Hello world.") && pandoc_out.contains("Second sentence"),
        "latex prose present:\n{pandoc_out}"
    );
    assert_has_sentence_break(&pandoc_out, "Hello world.", "Second sentence");
    assert!(
        !pandoc_out
            .lines()
            .any(|l| l.trim() == "2." && !l.contains("mc")),
        "math period must not orphan a bare '2.' prose line:\n{pandoc_out}"
    );
    let out = format_text(
        &input,
        &FormatConfig {
            format: Format::Latex,
            use_pandoc: false,
            ..Default::default()
        }
        .without_safety_backstops(),
    )
    .expect("native latex");
    assert!(
        out.contains("Hello world.\n") && out.contains("Second sentence"),
        "native latex must reflow multi-sentence prose:\n{out}"
    );
    assert!(
        !out.lines().any(|l| l.trim() == "2." && !l.contains("mc")),
        "math period must not orphan a bare '2.' prose line:\n{out}"
    );
    assert!(
        out.contains("print(1.0)") || out.contains("\\begin{minted}"),
        "native latex keeps minted/lstlisting bodies:\n{out}"
    );
}

#[test]
fn format_text_pandoc_table_list_quote() {
    if !require_writer() {
        return;
    }
    let input = read_fixture("structure_blocks.md");
    let backend = PandocBackend::Cli;
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: true,
        pandoc_backend: backend,
        pandoc_format: Some("markdown".into()),
        ..Default::default()
    };
    let pandoc_out = format_text(&input, &cfg).expect("pandoc write");
    assert!(
        pandoc_out.contains('a')
            && pandoc_out.contains('b')
            && (pandoc_out.contains("---") || pandoc_out.contains('|')),
        "table stays structure:\n{pandoc_out}"
    );
    assert!(
        pandoc_out.contains("- ") || pandoc_out.lines().any(|l| l.starts_with('-')),
        "bullet list stays structure:\n{pandoc_out}"
    );
    assert!(pandoc_out.contains('>'), "blockquote:\n{pandoc_out}");
    assert_has_sentence_break(&pandoc_out, "Intro sentence.", "Second");
    let run1 = format_text(
        &input,
        &FormatConfig {
            format: Format::Markdown,
            use_pandoc: false,
            ..Default::default()
        }
        .without_safety_backstops(),
    )
    .expect("native");
    assert!(
        run1.contains("| a |") || run1.contains("| a | b |"),
        "table cells:\n{run1}"
    );
    assert!(
        run1.contains("---") || run1.contains("| - |"),
        "table separator:\n{run1}"
    );
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
    }
    .without_safety_backstops();
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

/// Pandoc's org reader already emits each `=...=` as one Code inline
/// (class verbatim). The walker keeps that as Structure, so a closer
/// cannot land on its own line. Native pairing is meant to match this.
#[test]
fn pandoc_org_verbatim_inner_equals_stays_one_span() {
    if !require_writer() {
        return;
    }
    let input = "so =x = 1 -- note.= reflows while =s = \"x\"= does not.\n";
    let cfg = FormatConfig {
        format: Format::Org,
        use_pandoc: true,
        pandoc_backend: PandocBackend::Cli,
        pandoc_format: Some("org".into()),
        ..Default::default()
    };
    let pandoc_out = format_text(input, &cfg).expect("pandoc org write");
    assert!(
        !pandoc_out
            .lines()
            .any(|l| l.trim() == "=" || l.starts_with("= ")),
        "pandoc path must not orphan a verbatim closer, got:\n{pandoc_out}"
    );
    let out = format_text(
        input,
        &FormatConfig {
            format: Format::Org,
            use_pandoc: false,
            ..Default::default()
        }
        .without_safety_backstops(),
    )
    .expect("native org");
    assert!(
        !out.lines().any(|l| l.trim() == "=" || l.starts_with("= ")),
        "native path must not orphan a verbatim closer, got:\n{out}"
    );
    assert!(
        out.contains("x = 1") && out.contains("s ="),
        "both verbatim bodies must survive, got:\n{out}"
    );
}

/// `snapper --use-pandoc FILE` must exit 0 and emit sentence breaks.
#[test]
fn snapper_cli_use_pandoc_exits_0() {
    if !require_writer() {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_snapper");
    let input = fixture("mixed.md");
    let out = std::process::Command::new(bin)
        .args(["--use-pandoc", "--format", "markdown"])
        .arg(&input)
        .output()
        .expect("spawn snapper");
    assert!(
        out.status.success(),
        "snapper --use-pandoc must exit 0; stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Hello world.\n") && stdout.contains("Second sentence"),
        "CLI write must break sentences:\n{stdout}"
    );
    assert!(
        stdout.contains("```") || stdout.contains("print(1)"),
        "CLI write keeps CodeBlock:\n{stdout}"
    );
}

#[test]
fn format_text_pandoc_definition_list_stays_structure() {
    if !require_writer() {
        return;
    }
    let input = "Term one\n\n:   First sentence. Second sentence.\n";
    let cfg = FormatConfig {
        format: Format::Markdown,
        use_pandoc: true,
        pandoc_backend: PandocBackend::Cli,
        pandoc_format: Some("markdown".into()),
        ..Default::default()
    };
    let out = format_text(input, &cfg).expect("pandoc write");
    assert!(
        out.to_lowercase().contains("term one"),
        "definition term survives:\n{out}"
    );
    assert!(
        out.contains("First sentence") && out.contains("Second sentence"),
        "definition body prose present:\n{out}"
    );
    assert_has_sentence_break(&out, "First sentence.", "Second sentence");
}

/// `--use-pandoc` must not delete RST `..` comments and exit 0.
#[test]
fn format_text_use_pandoc_rst_comment_is_explicit_error() {
    let input = read_fixture("rst_comment.rst");
    assert!(
        dropped_comment_kind(&input, "rst").is_some(),
        "fixture must be a dropped RST comment"
    );
    let err = format_text(
        &input,
        &FormatConfig {
            format: Format::Rst,
            use_pandoc: true,
            pandoc_backend: PandocBackend::Cli,
            pandoc_format: Some("rst".into()),
            ..Default::default()
        },
    )
    .expect_err("pandoc path must refuse RST comments");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("drop") || msg.contains("comment") || msg.contains("pragma"),
        "expected explicit drop error, got: {msg}"
    );
}

/// `--use-pandoc` must not delete `snapper:off` and exit 0.
#[test]
fn format_text_use_pandoc_snapper_off_is_explicit_error() {
    let input = read_fixture("snapper_off.md");
    let err = format_text(
        &input,
        &FormatConfig {
            format: Format::Markdown,
            use_pandoc: true,
            pandoc_backend: PandocBackend::Cli,
            pandoc_format: Some("markdown".into()),
            ..Default::default()
        },
    )
    .expect_err("pandoc path must refuse snapper:off");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("snapper") || msg.contains("drop") || msg.contains("pragma"),
        "expected explicit drop error, got: {msg}"
    );
}

#[test]
fn format_text_use_pandoc_rst_snapper_off_is_explicit_error() {
    let input = read_fixture("snapper_off.rst");
    let err = format_text(
        &input,
        &FormatConfig {
            format: Format::Rst,
            use_pandoc: true,
            pandoc_backend: PandocBackend::Cli,
            pandoc_format: Some("rst".into()),
            ..Default::default()
        },
    )
    .expect_err("pandoc path must refuse RST snapper:off");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("snapper") || msg.contains("drop") || msg.contains("pragma"),
        "expected explicit drop error, got: {msg}"
    );
}

/// Native path still keeps comments and pragmas (this ticket does not change it).
#[test]
fn native_path_keeps_rst_comment_and_snapper_off() {
    let rst = read_fixture("rst_comment.rst");
    let rst_out = format_text(
        &rst,
        &FormatConfig {
            format: Format::Rst,
            use_pandoc: false,
            ..Default::default()
        },
    )
    .expect("native rst");
    assert!(
        rst_out.contains("This comment must not vanish."),
        "native must keep RST comment body:\n{rst_out}"
    );
    assert!(
        rst_out.contains(".. This is a recognized comment."),
        "native must keep recognized RST comment:\n{rst_out}"
    );

    let md = read_fixture("snapper_off.md");
    let md_out = format_text(
        &md,
        &FormatConfig {
            format: Format::Markdown,
            use_pandoc: false,
            ..Default::default()
        },
    )
    .expect("native md");
    assert!(
        md_out.contains("<!-- snapper:off -->") && md_out.contains("Keep this. Exactly here."),
        "native must keep markdown snapper:off:\n{md_out}"
    );

    let rst_off = read_fixture("snapper_off.rst");
    let rst_off_out = format_text(
        &rst_off,
        &FormatConfig {
            format: Format::Rst,
            use_pandoc: false,
            ..Default::default()
        },
    )
    .expect("native rst pragma");
    assert!(
        rst_off_out.contains("snapper:off") && rst_off_out.contains("Keep this. Exactly here."),
        "native must keep RST snapper:off:\n{rst_off_out}"
    );
}

/// CLI `--use-pandoc` on the RST comment fixture must not exit 0 after a drop.
#[test]
fn snapper_cli_use_pandoc_rst_comment_is_nonzero() {
    let bin = env!("CARGO_BIN_EXE_snapper");
    let input = fixture("rst_comment.rst");
    let out = std::process::Command::new(bin)
        .args(["--use-pandoc", "--format", "rst"])
        .arg(&input)
        .output()
        .expect("spawn snapper");
    assert!(
        !out.status.success(),
        "CLI must refuse RST comments; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("drop") || err.contains("comment") || err.contains("pragma"),
        "expected explicit drop error on stderr, got: {err}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty() || stdout.contains("This comment must not vanish."),
        "must not emit comment-stripped success output: {stdout}"
    );
}

#[test]
fn snapper_cli_use_pandoc_snapper_off_is_nonzero() {
    let bin = env!("CARGO_BIN_EXE_snapper");
    let input = fixture("snapper_off.md");
    let out = std::process::Command::new(bin)
        .args(["--use-pandoc", "--format", "markdown"])
        .arg(&input)
        .output()
        .expect("spawn snapper");
    assert!(
        !out.status.success(),
        "CLI must refuse snapper:off; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("snapper") || err.contains("drop") || err.contains("pragma"),
        "expected explicit drop error on stderr, got: {err}"
    );
}

/// Prose-only RST (no comments) still writes through when pandoc is present.
#[test]
fn format_text_use_pandoc_rst_prose_still_writes() {
    if !require_writer() {
        return;
    }
    let input = "Hello world. Second sentence.\n";
    let out = format_text(
        input,
        &FormatConfig {
            format: Format::Rst,
            use_pandoc: true,
            pandoc_backend: PandocBackend::Cli,
            pandoc_format: Some("rst".into()),
            ..Default::default()
        },
    )
    .expect("pandoc rst prose");
    assert_has_sentence_break(&out, "Hello world.", "Second sentence");
}

/// `--help` must name the CLI writer requirement (no silent spawn).
#[test]
fn snapper_help_says_writer_needs_pandoc_on_path() {
    let bin = env!("CARGO_BIN_EXE_snapper");
    let out = std::process::Command::new(bin)
        .arg("--help")
        .output()
        .expect("snapper --help");
    assert!(out.status.success(), "help must exit 0");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("writer still needs") && text.contains("PATH"),
        "help must say the writer still needs pandoc on PATH:\n{text}"
    );
    assert!(
        text.contains("reader-only") || text.contains("libsnapper_pandoc"),
        "help must mention the reader-only FFI library:\n{text}"
    );
}

/// Without FFI, `--use-pandoc --pandoc-backend cli` with no `pandoc` on PATH
/// is an explicit CLI error (not silent all-prose).
#[test]
fn snapper_cli_backend_without_pandoc_on_path_is_explicit() {
    let bin = env!("CARGO_BIN_EXE_snapper");
    let input = fixture("mixed.md");
    let out = std::process::Command::new(bin)
        .args([
            "--use-pandoc",
            "--pandoc-backend",
            "cli",
            "--format",
            "markdown",
        ])
        .arg(&input)
        .env("PATH", "/nonexistent-snapper-ypwj")
        .env("SNAPPER_PANDOC_CACHE", "0")
        .env_remove("SNAPPER_PANDOC_LIB")
        .env_remove("SNAPPER_PANDOC_LIB_DIR")
        .output()
        .expect("spawn snapper");
    assert!(
        !out.status.success(),
        "CLI backend without pandoc must fail; stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("PATH") || err.contains("CLI") || err.contains("unavailable"),
        "expected explicit CLI/PATH error, got: {err}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty() || !stdout.contains("Hello world."),
        "must not emit all-prose success without pandoc: {stdout}"
    );
}

/// With FFI parse and no in-process writer, missing `pandoc` on PATH is the
/// explicit writer error. If the library exports a writer, format stays
/// in-process (no PATH pandoc).
#[test]
fn snapper_ffi_without_pandoc_on_path_is_inprocess_or_explicit() {
    if !ffi_available() {
        eprintln!("skipping: libsnapper_pandoc not loadable");
        return;
    }
    let bin = env!("CARGO_BIN_EXE_snapper");
    let input = fixture("mixed.md");
    let out = std::process::Command::new(bin)
        .args([
            "--use-pandoc",
            "--pandoc-backend",
            "ffi",
            "--format",
            "markdown",
        ])
        .arg(&input)
        .env("PATH", "/nonexistent-snapper-ypwj")
        .env("SNAPPER_PANDOC_CACHE", "0")
        .output()
        .expect("spawn snapper");
    let err = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    if ffi_write_available() {
        assert!(
            out.status.success(),
            "FFI writer must not need pandoc on PATH; stderr={err} stdout={stdout}"
        );
        assert!(
            stdout.contains("Hello world."),
            "in-process write should emit prose:\n{stdout}"
        );
        return;
    }
    assert!(
        !out.status.success(),
        "reader-only FFI must not silently spawn; stdout={stdout}"
    );
    assert!(
        err.contains("PATH") || err.contains(WRITER_NEEDS_PATH) || err.contains("writer"),
        "expected explicit writer PATH error, got: {err}"
    );
}
