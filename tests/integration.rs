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

fn fixture_path(name: &str) -> String {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    base.join(name).to_string_lossy().to_string()
}

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
    // Write first pass to temp file, run again
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
    let output = snapper_binary()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .expect("failed to run snapper");
    let result = String::from_utf8(output.stdout).unwrap();
    // Input has no trailing newline, so output preserves that.
    assert_eq!(result, "Hello world.\nThis is a test.\nAnother sentence.");
}
