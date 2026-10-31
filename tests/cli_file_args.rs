//! How file arguments map to outputs and to the config's ignore list.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn snapper(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_snapper"))
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("failed to run snapper")
}

#[test]
fn output_file_gets_every_input_like_stdout() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "A one. A two.\n").unwrap();
    fs::write(dir.path().join("b.md"), "B one. B two.\n").unwrap();

    let stdout = snapper(dir.path(), &["a.md", "b.md"]);
    assert!(stdout.status.success());
    let to_file = snapper(dir.path(), &["a.md", "b.md", "-o", "out.md"]);
    assert!(
        to_file.status.success(),
        "{}",
        String::from_utf8_lossy(&to_file.stderr)
    );

    let written = fs::read_to_string(dir.path().join("out.md")).unwrap();
    assert_eq!(written, String::from_utf8(stdout.stdout).unwrap());
    assert_eq!(written, "A one.\nA two.\nB one.\nB two.\n");
}

#[test]
fn ignore_pattern_matches_a_dot_slash_path() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    fs::write(
        dir.path().join(".snapperrc.toml"),
        "ignore = [\"docs/*.md\"]\n",
    )
    .unwrap();
    fs::write(dir.path().join("docs/a.md"), "A one. A two.\n").unwrap();

    for path in ["docs/a.md", "./docs/a.md", "docs/./a.md"] {
        let out = snapper(dir.path(), &["--check", path]);
        assert!(
            out.status.success(),
            "{path} is ignored, yet --check reported:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn ignore_pattern_written_with_a_dot_slash_prefix_still_matches() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    fs::write(
        dir.path().join(".snapperrc.toml"),
        "ignore = [\"./docs/*.md\"]\n",
    )
    .unwrap();
    fs::write(dir.path().join("docs/a.md"), "A one. A two.\n").unwrap();

    for path in ["docs/a.md", "./docs/a.md", "docs/./a.md"] {
        let out = snapper(dir.path(), &["--check", path]);
        assert!(
            out.status.success(),
            "{path} is ignored by \"./docs/*.md\", yet --check reported:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn check_on_stdin_names_the_stdin_filepath() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_snapper"))
        .args(["--check", "--stdin-filepath", "docs/a.md"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run snapper");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"A one. B two.\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(stderr.contains("would reformat: docs/a.md"), "{stderr}");
    assert!(stderr.contains("docs/a.md:1:fused:"), "{stderr}");
}

#[test]
fn diff_on_stdin_names_the_stdin_filepath() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_snapper"))
        .args([
            "--diff",
            "--color",
            "never",
            "--stdin-filepath",
            "docs/a.md",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run snapper");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"A one. B two.\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success());
    assert!(
        stdout.starts_with("--- a/docs/a.md\n+++ b/docs/a.md\n"),
        "{stdout}"
    );
}

#[test]
fn sarif_uris_are_percent_encoded() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("my notes #1.md"), "A one. B two.\n").unwrap();
    fs::write(dir.path().join("caf\u{e9}.md"), "A one. B two.\n").unwrap();
    let out = snapper(
        dir.path(),
        &[
            "--check",
            "--output-format",
            "sarif",
            "my notes #1.md",
            "caf\u{e9}.md",
        ],
    );
    let sarif: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut uris: Vec<&str> = sarif["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
        })
        .collect();
    uris.dedup();
    assert_eq!(uris, ["my%20notes%20%231.md", "caf%C3%A9.md"]);
}
