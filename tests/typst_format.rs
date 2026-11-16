//! CLI coverage for native Typst: extension detection, `--check`, and fixtures.

use std::fs;
use std::path::Path;
use std::process::Command;

fn snapper() -> Command {
    Command::new(env!("CARGO_BIN_EXE_snapper"))
}

fn fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .to_string()
}

#[test]
fn typst_fixture_matches_expected() {
    let output = snapper()
        .args(["--native", "--format", "typst", &fixture("sample.typ")])
        .output()
        .expect("failed to run snapper");
    assert!(
        output.status.success(),
        "snapper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual = String::from_utf8(output.stdout).expect("invalid utf8");
    let expected = fs::read_to_string(fixture("expected.typ")).unwrap();
    pretty_assertions::assert_eq!(actual, expected);
}

#[test]
fn typst_extension_is_detected() {
    let output = snapper()
        .args(["--native", &fixture("sample.typ")])
        .output()
        .expect("failed to run snapper");
    assert!(
        output.status.success(),
        "detection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let with_flag = snapper()
        .args(["--native", "--format", "typst", &fixture("sample.typ")])
        .output()
        .expect("failed to run snapper");
    assert_eq!(output.stdout, with_flag.stdout);
}

#[test]
fn typst_check_fails_then_passes() {
    let unformatted = fs::read_to_string(fixture("sample.typ")).unwrap();
    let formatted = fs::read_to_string(fixture("expected.typ")).unwrap();
    let bad = pipe(&unformatted, &["--check", "--format", "typst"]);
    assert_eq!(
        bad.status.code(),
        Some(1),
        "unformatted Typst must fail --check"
    );
    let good = pipe(&formatted, &["--check", "--format", "typst"]);
    assert!(
        good.status.success(),
        "formatted Typst must pass --check: {}",
        String::from_utf8_lossy(&good.stderr)
    );
}

fn pipe(input: &str, args: &[&str]) -> std::process::Output {
    let mut cmd = snapper();
    cmd.arg("--native");
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
