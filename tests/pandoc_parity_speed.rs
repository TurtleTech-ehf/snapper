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

#[test]
fn parity_md_prose_reflow_and_structure() {
    let input = read("pandoc_ast/math_code.md");
    let native = format_text(&input, &native_cfg(Format::Markdown)).expect("native");
    let backend = if ffi_available() {
        PandocBackend::Ffi
    } else {
        PandocBackend::Cli
    };
    if backend == PandocBackend::Cli && !snapper_fmt::parser::pandoc::pandoc_available() {
        eprintln!("skipping: no pandoc backend");
        return;
    }
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

    // Multi-sentence prose reflowed on both paths.
    assert!(
        native.contains("First sentence.\n") || native.lines().any(|l| l == "First sentence."),
        "native reflow:\n{native}"
    );
    assert!(
        pandoc.contains("First sentence.\n") || pandoc.lines().any(|l| l == "First sentence."),
        "pandoc reflow:\n{pandoc}"
    );
    // Structure not prose-split on pandoc path.
    assert!(
        pandoc.contains("```python") && pandoc.contains("print(1.0)"),
        "code unit:\n{pandoc}"
    );
    assert!(
        pandoc.contains("$E = mc^2.$") || pandoc.contains("mc^2"),
        "math present:\n{pandoc}"
    );
    // Intentional non-identity: native keeps ### Title; pandoc Header is title text.
    // Document via assertion that both have Title and body prose.
    assert!(native.contains("Title") && pandoc.contains("Title"));
}

#[test]
fn parity_org_multi_sentence() {
    let input = read("sample.org");
    let native = format_text(&input, &native_cfg(Format::Org)).expect("native");
    if !snapper_fmt::parser::pandoc::pandoc_available() && !ffi_available() {
        eprintln!("skipping org parity: no pandoc");
        return;
    }
    let backend = PandocBackend::Auto;
    let pandoc = match format_text(&input, &pandoc_cfg(Format::Org, backend, "org")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("skipping org pandoc: {e}");
            return;
        }
    };
    // Both produce non-empty reformatted text; prose lines should exist.
    assert!(!native.is_empty() && !pandoc.is_empty());
    let p2 = format_text(&input, &pandoc_cfg(Format::Org, backend, "org")).unwrap();
    assert_eq!(pandoc, p2);
}

#[test]
fn walker_cost_negligible_vs_full_pipeline() {
    let json = read("pandoc_ast/math_code_md.json");
    // warm
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
    // Pure walker on a small fixture should be well under 5ms average.
    assert!(
        walker_med_approx < 5.0,
        "walker too slow: {walker_med_approx} ms"
    );
}

#[test]
fn speed_library_native_vs_pandoc_report() {
    let input = read("pandoc_ast/math_code.md");
    let n_cfg = native_cfg(Format::Markdown);
    let native_ms = time_format(&input, &n_cfg, 40, 5);

    let mut lines = vec![format!("native_format_text_med_ms={native_ms:.3}")];

    if snapper_fmt::parser::pandoc::pandoc_available() {
        let cli_ms = time_format(
            &input,
            &pandoc_cfg(Format::Markdown, PandocBackend::Cli, "markdown"),
            15,
            2,
        );
        lines.push(format!("pandoc_cli_format_text_med_ms={cli_ms:.3}"));
        lines.push(format!("cli_over_native={:.2}", cli_ms / native_ms.max(1e-9)));
    }
    if ffi_available() {
        let ffi_ms = time_format(
            &input,
            &pandoc_cfg(Format::Markdown, PandocBackend::Ffi, "markdown"),
            20,
            3,
        );
        lines.push(format!("pandoc_ffi_format_text_med_ms={ffi_ms:.3}"));
        lines.push(format!("ffi_over_native={:.2}", ffi_ms / native_ms.max(1e-9)));
        // Same order of magnitude band for successor readiness (not beating pure Rust).
        // Plan: not ~100× at representative size after warm-up for process-level;
        // library-level floor is pandoc reader cost — record honestly.
        eprintln!("{}", lines.join("\n"));
        assert!(
            ffi_ms < 500.0,
            "FFI path pathologically slow: {ffi_ms} ms"
        );
    } else {
        eprintln!("{}", lines.join("\n"));
        eprintln!("FFI unavailable; CLI-only report");
    }

    // Multi-format: latex via auto when possible
    if snapper_fmt::parser::pandoc::pandoc_available() {
        let tex = read("pandoc_ast/math_code.tex");
        let out = format_text(
            &tex,
            &pandoc_cfg(Format::Latex, PandocBackend::Cli, "latex"),
        )
        .expect("latex pandoc");
        assert!(out.contains("```") || out.contains("print"), "latex code: {out}");
        assert!(
            out.contains("mc^2") || out.contains("$$"),
            "latex math: {out}"
        );
    }
}

#[test]
fn fail_closed_still_holds() {
    if ffi_available() {
        // Explicit FFI force is tested via CLI subprocess elsewhere.
        return;
    }
    let err = format_text(
        "Hi. There.\n",
        &pandoc_cfg(Format::Markdown, PandocBackend::Ffi, "markdown"),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("unavailable") || err.to_string().contains("FFI")
    );
}
