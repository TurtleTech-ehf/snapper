use std::fs;
use std::path::Path;
use std::process::Command;

fn snapper_binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_snapper"))
}

fn run_format(format: &str, input_path: &str) -> String {
    let output = snapper_binary()
        .args(["--format", format, input_path])
        .output()
        .expect("failed to run snapper");
    assert!(
        output.status.success(),
        "snapper failed: {:?}",
        output.stderr
    );
    String::from_utf8(output.stdout).expect("invalid utf8")
}

fn pipe_stdin(input: &str, args: &[&str]) -> std::process::Output {
    let mut cmd = snapper_binary();
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("failed to spawn snapper");
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().expect("failed to wait")
}

fn fixture_path(name: &str) -> String {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    base.join(name).to_string_lossy().to_string()
}

// ---- EXISTING TESTS ----

#[test]
fn org_format() {
    let actual = run_format("org", &fixture_path("sample.org"));
    let expected = fs::read_to_string(fixture_path("expected.org")).unwrap();
    pretty_assertions::assert_eq!(actual, expected);
}

#[test]
fn plaintext_format() {
    let actual = run_format("plaintext", &fixture_path("sample.txt"));
    let expected = fs::read_to_string(fixture_path("expected.txt")).unwrap();
    pretty_assertions::assert_eq!(actual, expected);
}

#[test]
fn latex_format() {
    let actual = run_format("latex", &fixture_path("sample.tex"));
    let expected = fs::read_to_string(fixture_path("expected.tex")).unwrap();
    pretty_assertions::assert_eq!(actual, expected);
}

#[test]
fn markdown_format() {
    let actual = run_format("markdown", &fixture_path("sample.md"));
    let expected = fs::read_to_string(fixture_path("expected.md")).unwrap();
    pretty_assertions::assert_eq!(actual, expected);
}

#[test]
fn idempotent_org() {
    let first = run_format("org", &fixture_path("sample.org"));
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), &first).unwrap();
    let second = run_format("org", &tmp.path().to_string_lossy());
    pretty_assertions::assert_eq!(first, second, "org reflow must be idempotent");
}

#[test]
fn idempotent_plaintext() {
    let first = run_format("plaintext", &fixture_path("sample.txt"));
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), &first).unwrap();
    let second = run_format("plaintext", &tmp.path().to_string_lossy());
    pretty_assertions::assert_eq!(first, second, "plaintext reflow must be idempotent");
}

#[test]
fn idempotent_latex() {
    let first = run_format("latex", &fixture_path("sample.tex"));
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), &first).unwrap();
    let second = run_format("latex", &tmp.path().to_string_lossy());
    pretty_assertions::assert_eq!(first, second, "latex reflow must be idempotent");
}

#[test]
fn idempotent_markdown() {
    let first = run_format("markdown", &fixture_path("sample.md"));
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), &first).unwrap();
    let second = run_format("markdown", &tmp.path().to_string_lossy());
    pretty_assertions::assert_eq!(first, second, "markdown reflow must be idempotent");
}

#[test]
fn check_mode_passes_on_formatted() {
    let output = snapper_binary()
        .args(["--check", "--format", "org", &fixture_path("expected.org")])
        .output()
        .expect("failed to run snapper");
    assert!(
        output.status.success(),
        "--check should pass on already-formatted file"
    );
}

#[test]
fn check_mode_fails_on_unformatted() {
    let output = snapper_binary()
        .args(["--check", "--format", "org", &fixture_path("sample.org")])
        .output()
        .expect("failed to run snapper");
    assert!(
        !output.status.success(),
        "--check should fail on unformatted file"
    );
}

#[test]
fn stdin_stdout() {
    let input = "Hello world. This is a test. Another sentence.";
    let out = pipe_stdin(input, &[]);
    let result = String::from_utf8(out.stdout).unwrap();
    assert_eq!(result, "Hello world.\nThis is a test.\nAnother sentence.");
}

// ---- FEATURE 1: --stdin-filepath ----

#[test]
fn stdin_filepath_auto_detects_org() {
    let input = "#+TITLE: Test\n\nFirst sentence. Second sentence.\n";
    let out = pipe_stdin(input, &["--stdin-filepath", "paper.org"]);
    assert!(out.status.success());
    let result = String::from_utf8(out.stdout).unwrap();
    // Should detect org format and preserve #+TITLE as structure
    assert!(result.starts_with("#+TITLE: Test\n"));
    assert!(result.contains("First sentence.\n"));
    assert!(result.contains("Second sentence.\n"));
}

#[test]
fn stdin_filepath_detects_latex() {
    // .tex extension should trigger LaTeX parser (preamble preserved)
    let input =
        "\\documentclass{article}\n\\begin{document}\nHello world. Second.\n\\end{document}\n";
    let out = pipe_stdin(input, &["--stdin-filepath", "paper.tex"]);
    assert!(out.status.success());
    let result = String::from_utf8(out.stdout).unwrap();
    assert!(result.contains("\\documentclass{article}"));
    assert!(result.contains("Hello world.\n"));
}

#[test]
fn format_flag_overrides_stdin_filepath() {
    // --format should take priority over --stdin-filepath
    let input = "Hello world. Second sentence.\n";
    let out = pipe_stdin(
        input,
        &["--format", "plaintext", "--stdin-filepath", "paper.org"],
    );
    assert!(out.status.success());
    let result = String::from_utf8(out.stdout).unwrap();
    assert_eq!(result, "Hello world.\nSecond sentence.\n");
}

// ---- FEATURE 2: snapper:off / snapper:on pragmas ----

#[test]
fn pragma_off_on_org() {
    let input =
        "First. Second.\n# snapper:off\nDo not. Touch this.\n# snapper:on\nThird. Fourth.\n";
    let out = pipe_stdin(input, &["--format", "org"]);
    let result = String::from_utf8(out.stdout).unwrap();
    assert!(result.contains("First.\nSecond.\n"));
    assert!(result.contains("Do not. Touch this.\n")); // untouched
    assert!(result.contains("Third.\nFourth.\n"));
}

#[test]
fn pragma_off_on_latex() {
    let input = "\\begin{document}\nHello world. Goodbye world.\n% snapper:off\nKeep this. Exactly here.\n% snapper:on\nFinal thing. Last sentence.\n\\end{document}\n";
    let out = pipe_stdin(input, &["--format", "latex"]);
    let result = String::from_utf8(out.stdout).unwrap();
    assert!(result.contains("Hello world.\nGoodbye world.\n"));
    assert!(result.contains("Keep this. Exactly here.\n")); // untouched
    assert!(result.contains("Final thing.\nLast sentence.\n"));
}

#[test]
fn pragma_off_on_markdown() {
    let input = "Hello world. Goodbye world.\n<!-- snapper:off -->\nKeep this. Exactly here.\n<!-- snapper:on -->\nFinal thing. Last sentence.\n";
    let out = pipe_stdin(input, &["--format", "markdown"]);
    let result = String::from_utf8(out.stdout).unwrap();
    assert!(result.contains("Hello world.\nGoodbye world.\n"));
    assert!(result.contains("Keep this. Exactly here.\n"));
    assert!(result.contains("Final thing.\nLast sentence."));
}

#[test]
fn pragma_off_on_plaintext() {
    let input = "Hello world. Goodbye world.\nsnapper:off\nKeep this. Exactly here.\nsnapper:on\nFinal thing. Last sentence.\n";
    let out = pipe_stdin(input, &["--format", "plaintext"]);
    let result = String::from_utf8(out.stdout).unwrap();
    assert!(result.contains("Hello world.\nGoodbye world.\n"));
    assert!(result.contains("Keep this. Exactly here.\n"));
    assert!(result.contains("Final thing.\nLast sentence.\n"));
}

// ---- FEATURE 5: --range ----

#[test]
fn range_formats_only_specified_lines() {
    let input = "Line one. Stay same.\nLine two. Should split. Into two.\nLine three. Stay same.\n";
    let out = pipe_stdin(input, &["--range", "2:2"]);
    let result = String::from_utf8(out.stdout).unwrap();
    // Line 1 unchanged
    assert!(result.starts_with("Line one. Stay same.\n"));
    // Line 2 split
    assert!(result.contains("Line two.\nShould split.\nInto two.\n"));
    // Line 3 unchanged
    assert!(result.ends_with("Line three. Stay same.\n"));
}

#[test]
fn range_preserves_outside_lines_exactly() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        tmp.path(),
        "Keep. This line.\nReformat. This line. Please.\nKeep. This too.\n",
    )
    .unwrap();
    let output = snapper_binary()
        .args([
            "--format",
            "plaintext",
            "--range",
            "2:2",
            &tmp.path().to_string_lossy(),
        ])
        .output()
        .expect("failed to run");
    let result = String::from_utf8(output.stdout).unwrap();
    assert!(result.starts_with("Keep. This line.\n"));
    assert!(result.contains("Reformat.\nThis line.\nPlease.\n"));
    assert!(result.ends_with("Keep. This too.\n"));
}

// ---- FEATURE 6: --output-format json/sarif ----

#[test]
fn check_json_output() {
    let output = snapper_binary()
        .args([
            "--check",
            "--output-format",
            "json",
            "--format",
            "org",
            &fixture_path("sample.org"),
        ])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert!(parsed.is_array());
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert!(arr[0]["file"].as_str().unwrap().contains("sample.org"));
}

#[test]
fn check_sarif_output() {
    let output = snapper_binary()
        .args([
            "--check",
            "--output-format",
            "sarif",
            "--format",
            "org",
            &fixture_path("sample.org"),
        ])
        .output()
        .expect("failed to run");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("invalid SARIF");
    assert_eq!(parsed["version"], "2.1.0");
    let results = &parsed["runs"][0]["results"];
    assert!(results.is_array());
    assert!(!results.as_array().unwrap().is_empty());
}

// ---- Golden master: edge cases ----

#[test]
fn edge_cases_org() {
    let actual = run_format("org", &fixture_path("edge_cases.org"));
    let expected = fs::read_to_string(fixture_path("expected_edge_cases.org")).unwrap();
    pretty_assertions::assert_eq!(actual, expected);
}

#[test]
fn edge_cases_plaintext() {
    let actual = run_format("plaintext", &fixture_path("edge_cases.txt"));
    let expected = fs::read_to_string(fixture_path("expected_edge_cases.txt")).unwrap();
    pretty_assertions::assert_eq!(actual, expected);
}

#[test]
fn idempotent_edge_cases_org() {
    let first = run_format("org", &fixture_path("edge_cases.org"));
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), &first).unwrap();
    let second = run_format("org", &tmp.path().to_string_lossy());
    pretty_assertions::assert_eq!(first, second, "edge cases org reflow must be idempotent");
}

#[test]
fn idempotent_edge_cases_plaintext() {
    let first = run_format("plaintext", &fixture_path("edge_cases.txt"));
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), &first).unwrap();
    let second = run_format("plaintext", &tmp.path().to_string_lossy());
    pretty_assertions::assert_eq!(
        first, second,
        "edge cases plaintext reflow must be idempotent"
    );
}

// ---- FEATURE 7: snapper init ----

#[test]
fn init_dry_run_shows_config() {
    let output = snapper_binary()
        .args(["init", "--dry-run"])
        .output()
        .expect("failed to run");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(".snapperrc.toml"));
    assert!(stderr.contains("format ="));
    assert!(stderr.contains("pre-commit"));
    assert!(stderr.contains("apheleia"));
}
