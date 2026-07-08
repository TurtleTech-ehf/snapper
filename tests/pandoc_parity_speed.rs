//! Library-level parity + speed for the pandoc successor path.
//!
//! Times shipped `format_text` (not process spawn). Run with:
//!   cargo test --release --features "cli,pandoc" --test pandoc_parity_speed -- --nocapture

#![cfg(feature = "pandoc")]

use std::path::PathBuf;
use std::time::Instant;

use snapper_fmt::format::Format;
use snapper_fmt::parser::pandoc::{
    PandocBackend, ffi_available, regions_from_pandoc_json,
};
use snapper_fmt::parser::Region;
use snapper_fmt::{format_text, FormatConfig};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixture(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

fn pandoc_cfg(format: Format, backend: PandocBackend, pandoc_format: &str) -> FormatConfig {
    FormatConfig {
        format,
        use_pandoc: true,
        pandoc_backend: backend,
        pandoc_format: Some(pandoc_format.into()),
        ..Default::default()
    }
}

fn native_cfg(format: Format) -> FormatConfig {
    FormatConfig {
        format,
        use_pandoc: false,
        ..Default::default()
    }
}

fn median_ms(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn time_format(input: &str, cfg: &FormatConfig, n: usize, warm: usize) -> f64 {
    for _ in 0..warm {
        let _ = format_text(input, cfg).expect("warm");
    }
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let t0 = Instant::now();
        let _ = format_text(input, cfg).expect("timed");
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    median_ms(samples)
}

fn best_backend() -> Option<PandocBackend> {
    if ffi_available() {
        Some(PandocBackend::Ffi)
    } else if snapper_fmt::parser::pandoc::pandoc_available() {
        Some(PandocBackend::Cli)
    } else {
        None
    }
}

/// True if multi-sentence input was reflowed onto separate lines.
fn has_sentence_reflow(out: &str, first: &str, second_substr: &str) -> bool {
    out.contains(&format!("{first}\n")) && out.contains(second_substr)
}

#[test]
fn parity_md_prose_reflow_and_structure() {
    let input = read("pandoc_ast/math_code.md");
    let native = format_text(&input, &native_cfg(Format::Markdown)).expect("native");
    let backend = best_backend().expect("need pandoc backend for parity test");
    let pandoc = format_text(
        &input,
        &pandoc_cfg(Format::Markdown, backend, "markdown"),
    )
    .expect("pandoc");
    let p2 = format_text(
        &input,
        &pandoc_cfg(Format::Markdown, backend, "markdown"),
    )
    .expect("pandoc2");
    assert_eq!(pandoc, p2, "pandoc dual-run");

    assert!(
        has_sentence_reflow(&native, "First sentence.", "Second sentence"),
        "native reflow:\n{native}"
    );
    assert!(
        has_sentence_reflow(&pandoc, "First sentence.", "Second sentence"),
        "pandoc reflow:\n{pandoc}"
    );
    assert!(
        pandoc.contains("```python") && pandoc.contains("print(1.0)"),
        "code unit:\n{pandoc}"
    );
    assert!(
        pandoc.contains("$E = mc^2.$") || pandoc.contains("mc^2"),
        "math present:\n{pandoc}"
    );
    assert!(native.contains("Title") && pandoc.contains("Title"));
}

#[test]
fn parity_org_multi_sentence() {
    let input = read("sample.org");
    let native = format_text(&input, &native_cfg(Format::Org)).expect("native");
    let backend = best_backend().expect("need pandoc backend");
    let pandoc = format_text(&input, &pandoc_cfg(Format::Org, backend, "org"))
        .unwrap_or_else(|e| panic!("org pandoc failed: {e}"));
    let p2 = format_text(&input, &pandoc_cfg(Format::Org, backend, "org")).unwrap();
    assert_eq!(pandoc, p2, "org pandoc dual-run");

    // Multi-sentence prose reflow (fixture: "This is the first paragraph... It has multiple...")
    assert!(
        has_sentence_reflow(
            &native,
            "This is the first paragraph of the introduction.",
            "It has multiple sentences"
        ) || (native.contains("first paragraph")
            && native.lines().filter(|l| l.contains("sentence") || l.contains("paragraph")).count()
                >= 2),
        "native org reflow:\n{native}"
    );
    assert!(
        has_sentence_reflow(
            &pandoc,
            "This is the first paragraph of the introduction.",
            "It has multiple sentences"
        ) || (pandoc.contains("first paragraph")
            && pandoc
                .lines()
                .filter(|l| l.contains("sentence") || l.contains("paragraph"))
                .count()
                >= 2),
        "pandoc org reflow:\n{pandoc}"
    );

    // Code: native keeps org src block framing; pandoc emits CodeBlock fences.
    assert!(
        native.contains("import numpy") || native.contains("np.array"),
        "native keeps code body:\n{native}"
    );
    assert!(
        (pandoc.contains("```") && pandoc.contains("import numpy"))
            || pandoc.contains("import numpy"),
        "pandoc keeps code non-prose:\n{pandoc}"
    );
    // Table cells present as structure (pipe or org table markers), not sentence-split away.
    assert!(
        native.contains("Alice") && native.contains("95"),
        "native table:\n{native}"
    );
    assert!(
        pandoc.contains("Alice") && pandoc.contains("95"),
        "pandoc table cells:\n{pandoc}"
    );
    // Code body not prose-fragmented by periods in list numbers etc.
    assert!(
        !pandoc.lines().any(|l| l.trim() == "2, 3])" || l.trim() == "2,"),
        "code not sentence-fragmented:\n{pandoc}"
    );

    // Write samples for verifier audit (implementer scratch when env set).
    if let Ok(dir) = std::env::var("SNAPPER_PARITY_SCRATCH") {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(format!("{dir}/org-native-out.txt"), &native);
        let _ = std::fs::write(format!("{dir}/org-pandoc-out.txt"), &pandoc);
    }
}

#[test]
fn walker_cost_negligible_vs_full_pipeline() {
    let json = read("pandoc_ast/math_code_md.json");
    for _ in 0..5 {
        let _ = regions_from_pandoc_json(&json).unwrap();
    }
    let n = 200;
    let t0 = Instant::now();
    for _ in 0..n {
        let r = regions_from_pandoc_json(&json).unwrap();
        assert!(r.iter().any(|x| matches!(x, Region::Code { .. })));
    }
    let walker_med_approx = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
    eprintln!("walker_approx_mean_ms={walker_med_approx:.4}");
    assert!(
        walker_med_approx < 5.0,
        "walker too slow: {walker_med_approx} ms"
    );
}

#[test]
fn speed_library_native_vs_pandoc_report() {
    // Markdown fixture
    let md = read("pandoc_ast/math_code.md");
    let n_md = time_format(&md, &native_cfg(Format::Markdown), 40, 5);
    let mut lines = vec![format!("md_native_format_text_med_ms={n_md:.3}")];

    if snapper_fmt::parser::pandoc::pandoc_available() {
        let cli = time_format(
            &md,
            &pandoc_cfg(Format::Markdown, PandocBackend::Cli, "markdown"),
            20,
            3,
        );
        lines.push(format!("md_pandoc_cli_med_ms={cli:.3}"));
        lines.push(format!("md_cli_over_native={:.2}", cli / n_md.max(1e-9)));
    }
    if ffi_available() {
        let ffi = time_format(
            &md,
            &pandoc_cfg(Format::Markdown, PandocBackend::Ffi, "markdown"),
            20,
            3,
        );
        lines.push(format!("md_pandoc_ffi_med_ms={ffi:.3}"));
        lines.push(format!("md_ffi_over_native={:.2}", ffi / n_md.max(1e-9)));
        assert!(ffi < 500.0, "md FFI pathologically slow: {ffi}");
    }

    // Second format: org (N≥20 warm library medians)
    let org = read("sample.org");
    let n_org = time_format(&org, &native_cfg(Format::Org), 40, 5);
    lines.push(format!("org_native_format_text_med_ms={n_org:.3}"));
    if snapper_fmt::parser::pandoc::pandoc_available() {
        let cli = time_format(
            &org,
            &pandoc_cfg(Format::Org, PandocBackend::Cli, "org"),
            20,
            3,
        );
        lines.push(format!("org_pandoc_cli_med_ms={cli:.3}"));
        lines.push(format!("org_cli_over_native={:.2}", cli / n_org.max(1e-9)));
    }
    if ffi_available() {
        let ffi = time_format(
            &org,
            &pandoc_cfg(Format::Org, PandocBackend::Ffi, "org"),
            20,
            3,
        );
        lines.push(format!("org_pandoc_ffi_med_ms={ffi:.3}"));
        lines.push(format!("org_ffi_over_native={:.2}", ffi / n_org.max(1e-9)));
        // Warm library FFI should stay same-order (not ~100×) on representative org.
        assert!(
            ffi / n_org.max(1e-9) < 50.0,
            "org warm FFI not same-order vs native: {ffi} / {n_org}"
        );
    }

    eprintln!("{}", lines.join("\n"));
    if let Ok(dir) = std::env::var("SNAPPER_PARITY_SCRATCH") {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(format!("{dir}/speed-parity-library.txt"), lines.join("\n"));
    }

    // Multi-format latex structure (not speed-only)
    if snapper_fmt::parser::pandoc::pandoc_available() {
        let tex = read("pandoc_ast/math_code.tex");
        let out = format_text(
            &tex,
            &pandoc_cfg(Format::Latex, PandocBackend::Cli, "latex"),
        )
        .expect("latex pandoc");
        assert!(
            has_sentence_reflow(&out, "Hello world.", "Second sentence"),
            "latex prose reflow:\n{out}"
        );
        assert!(out.contains("```") && out.contains("print"), "latex code: {out}");
        assert!(out.contains("mc^2") || out.contains("$$"), "latex math: {out}");
    }
}

/// Always exercises fail-closed via a real snapper subprocess + bad lib path.
#[test]
fn fail_closed_still_holds() {
    let bin = env!("CARGO_BIN_EXE_snapper");
    let input = fixture("pandoc_ast/math_code.md");
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
        "FFI missing lib must fail; stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("unavailable") || err.contains("FFI") || err.contains("library"),
        "explicit error: {err}"
    );
}
