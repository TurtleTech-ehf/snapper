//! CLI coverage for native AsciiDoc: extension detection and `--check`.

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn snapper() -> Command {
    Command::new(env!("CARGO_BIN_EXE_snapper"))
}

fn pipe(input: &str, args: &[&str]) -> std::process::Output {
    let mut cmd = snapper();
    cmd.arg("--native");
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("failed to spawn snapper");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        stdin.write_all(input.as_bytes()).expect("write stdin");
    }
    child.wait_with_output().expect("wait")
}

#[test]
fn adoc_extension_splits_without_a_format_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.adoc");
    fs::write(&path, "Hello world. This is a test.\n").unwrap();
    let output = snapper()
        .arg(&path)
        .output()
        .expect("failed to run snapper");
    assert!(
        output.status.success(),
        "snapper failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Hello world.\nThis is a test.\n"
    );
}

#[test]
fn adoc_check_fails_on_a_fused_paragraph_and_passes_after() {
    let bad = pipe(
        "The method is stable. It converges.\n",
        &["--check", "--format", "asciidoc"],
    );
    assert_eq!(
        bad.status.code(),
        Some(1),
        "fused AsciiDoc must fail --check"
    );
    let good = pipe(
        "The method is stable.\nIt converges.\n",
        &["--check", "--format", "asciidoc"],
    );
    assert!(
        good.status.success(),
        "formatted AsciiDoc must pass --check: {}",
        String::from_utf8_lossy(&good.stderr)
    );
}

#[test]
fn adoc_listing_is_left_alone() {
    let input = "----\nKeep this. Together.\n----\n";
    let output = pipe(input, &["--format", "asciidoc"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), input);
}
