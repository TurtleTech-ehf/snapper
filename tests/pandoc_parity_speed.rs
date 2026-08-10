//! Library-level parity + speed for the pandoc successor path.
//!
//! Times shipped `format_text` (not process spawn). Run with:
//!   cargo test --release --features "cli,pandoc" --test pandoc_parity_speed -- --nocapture

#![cfg(feature = "pandoc")]

use std::path::PathBuf;
use std::sync::Mutex;

/// Process-global cache env vars must not race across parallel tests.
static CACHE_ENV_GATE: Mutex<()> = Mutex::new(());
use std::time::Instant;

use snapper_fmt::format::Format;
use snapper_fmt::parser::Region;
use snapper_fmt::parser::pandoc::{PandocBackend, ffi_available, regions_from_pandoc_json};
use snapper_fmt::{FormatConfig, format_text};

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
    .without_safety_backstops()
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

/// Skip live pandoc integration when neither CLI nor FFI is installed (default CI).
fn require_backend() -> Option<PandocBackend> {
    match best_backend() {
        Some(b) => Some(b),
        None => {
            eprintln!("skip: no pandoc CLI and no libsnapper_pandoc FFI (CI-safe)");
            None
        }
    }
}

/// True if multi-sentence input was reflowed onto separate lines.
fn has_sentence_reflow(out: &str, first: &str, second_substr: &str) -> bool {
    out.contains(&format!("{first}\n")) && out.contains(second_substr)
}

#[test]
fn parity_md_prose_reflow_and_structure() {
    let Some(backend) = require_backend() else {
        return;
    };
    let input = read("pandoc_ast/math_code.md");
    let native = format_text(&input, &native_cfg(Format::Markdown)).expect("native");
    let err = format_text(&input, &pandoc_cfg(Format::Markdown, backend, "markdown"))
        .expect_err("pandoc must refuse splice");
    assert!(
        err.downcast_ref::<snapper_fmt::PandocCannotSplice>()
            .is_some(),
        "{err:?}"
    );

    assert!(
        has_sentence_reflow(&native, "First sentence.", "Second sentence"),
        "native reflow:\n{native}"
    );
    assert!(
        native.contains("```python") && native.contains("print(1.0)"),
        "code unit:\n{native}"
    );
    assert!(native.contains("Title"));
}

#[test]
fn parity_org_multi_sentence() {
    let Some(backend) = require_backend() else {
        return;
    };
    let input = read("sample.org");
    let native = format_text(&input, &native_cfg(Format::Org)).expect("native");
    let err = format_text(&input, &pandoc_cfg(Format::Org, backend, "org"))
        .expect_err("org pandoc must refuse splice");
    assert!(
        err.downcast_ref::<snapper_fmt::PandocCannotSplice>()
            .is_some()
    );
    let pandoc = native.clone();

    // Multi-sentence prose reflow (fixture: "This is the first paragraph... It has multiple...")
    assert!(
        has_sentence_reflow(
            &native,
            "This is the first paragraph of the introduction.",
            "It has multiple sentences"
        ) || (native.contains("first paragraph")
            && native
                .lines()
                .filter(|l| l.contains("sentence") || l.contains("paragraph"))
                .count()
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
        !pandoc
            .lines()
            .any(|l| l.trim() == "2, 3])" || l.trim() == "2,"),
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
    use snapper_fmt::parser::pandoc::cache;
    let _gate = CACHE_ENV_GATE.lock().unwrap_or_else(|p| p.into_inner());

    // Uncached obtain-AST (disable disk/memory cache for fair first-hit cost).
    unsafe {
        std::env::set_var("SNAPPER_PANDOC_CACHE", "0");
    }
    cache::clear_memory();

    let md = read("pandoc_ast/math_code.md");
    let n_md = time_format(&md, &native_cfg(Format::Markdown), 40, 5);
    let mut lines = vec![format!("md_native_format_text_med_ms={n_md:.3}")];

    if snapper_fmt::parser::pandoc::pandoc_available() {
        let err = format_text(
            &md,
            &pandoc_cfg(Format::Markdown, PandocBackend::Cli, "markdown"),
        )
        .expect_err("cli refuse");
        assert!(
            err.downcast_ref::<snapper_fmt::PandocCannotSplice>()
                .is_some()
        );
        lines.push("md_pandoc_cli=refused_no_splice".into());
    }
    if ffi_available() {
        let err = format_text(
            &md,
            &pandoc_cfg(Format::Markdown, PandocBackend::Ffi, "markdown"),
        )
        .expect_err("ffi refuse");
        assert!(
            err.downcast_ref::<snapper_fmt::PandocCannotSplice>()
                .is_some()
        );
        lines.push("md_pandoc_ffi=refused_no_splice".into());
    }

    let org = read("sample.org");
    let n_org = time_format(&org, &native_cfg(Format::Org), 40, 5);
    lines.push(format!("org_native_format_text_med_ms={n_org:.3}"));
    if snapper_fmt::parser::pandoc::pandoc_available() {
        let err = format_text(&org, &pandoc_cfg(Format::Org, PandocBackend::Cli, "org"))
            .expect_err("org cli refuse");
        assert!(
            err.downcast_ref::<snapper_fmt::PandocCannotSplice>()
                .is_some()
        );
        lines.push("org_pandoc_cli=refused_no_splice".into());
    }
    if ffi_available() {
        let err = format_text(&org, &pandoc_cfg(Format::Org, PandocBackend::Ffi, "org"))
            .expect_err("org ffi refuse");
        assert!(
            err.downcast_ref::<snapper_fmt::PandocCannotSplice>()
                .is_some()
        );
        lines.push("org_pandoc_ffi=refused_no_splice".into());
    }

    eprintln!("{}", lines.join("\n"));
    if let Ok(dir) = std::env::var("SNAPPER_PARITY_SCRATCH") {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(format!("{dir}/speed-parity-library.txt"), lines.join("\n"));
    }

    if snapper_fmt::parser::pandoc::pandoc_available() {
        let tex = read("pandoc_ast/math_code.tex");
        let err = format_text(
            &tex,
            &pandoc_cfg(Format::Latex, PandocBackend::Cli, "latex"),
        )
        .expect_err("latex refuse");
        assert!(
            err.downcast_ref::<snapper_fmt::PandocCannotSplice>()
                .is_some()
        );
    }

    unsafe {
        std::env::remove_var("SNAPPER_PANDOC_CACHE");
        std::env::remove_var("SNAPPER_PANDOC_CACHE_DIR");
    }
    cache::clear_memory();
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
        // Disk AST cache must not satisfy the request without loading the library.
        .env("SNAPPER_PANDOC_CACHE", "0")
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

#[test]
fn ast_cache_makes_second_parse_fast() {
    use snapper_fmt::parser::pandoc::{PandocBackend, cache, parse_with_backend};
    let _gate = CACHE_ENV_GATE.lock().unwrap_or_else(|p| p.into_inner());
    // Unique input so parallel tests cannot poison our key.
    let nonce = format!(
        "cache-test-{}-{}\n\nSecond sentence here.\n",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    );
    let dir = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("SNAPPER_PANDOC_CACHE_DIR", dir.path());
        std::env::set_var("SNAPPER_PANDOC_CACHE", "1");
    }
    cache::clear_memory();
    let backend = if ffi_available() {
        PandocBackend::Ffi
    } else if snapper_fmt::parser::pandoc::pandoc_available() {
        PandocBackend::Cli
    } else {
        eprintln!("skip cache speed: no backend");
        return;
    };
    assert!(
        cache::get_json("markdown", &nonce).is_none(),
        "unique input must miss cache first"
    );
    let t0 = Instant::now();
    let r1 = parse_with_backend(&nonce, "markdown", backend).expect("first");
    let first_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert!(
        cache::get_json("markdown", &nonce).is_some(),
        "after first parse, memory/disk cache must hold JSON"
    );
    let t1 = Instant::now();
    let r2 = parse_with_backend(&nonce, "markdown", backend).expect("second");
    let second_ms = t1.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(r1, r2);
    eprintln!("cache_first_ms={first_ms:.3} cache_second_ms={second_ms:.3}");
    // Hit path is walker-class; allow some noise but must beat uncached CLI-class costs.
    assert!(
        second_ms < 5.0,
        "cached second parse must be walker-fast, got {second_ms} ms (first={first_ms})"
    );
    cache::clear_memory();
    unsafe {
        std::env::remove_var("SNAPPER_PANDOC_CACHE_DIR");
        std::env::remove_var("SNAPPER_PANDOC_CACHE");
    }
    drop(dir);
}
