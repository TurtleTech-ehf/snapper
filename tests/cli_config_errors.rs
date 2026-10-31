//! A config file that cannot be read or parsed is an error, never a silent
//! fallback to the built-in defaults.

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn snapper(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_snapper"))
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("failed to run snapper")
}

fn assert_config_error(out: &Output, path_hint: &str) {
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a bad config must fail the run; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        stderr.contains(path_hint),
        "error should name {path_hint}, got:\n{stderr}"
    );
    assert!(
        out.stdout.is_empty(),
        "nothing should be formatted with a bad config"
    );
}

#[test]
fn malformed_discovered_config_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".snapperrc.toml"), "max_width = [\n").unwrap();
    fs::write(dir.path().join("a.md"), "First one. Second one.\n").unwrap();
    let out = snapper(dir.path(), &["a.md"]);
    assert_config_error(&out, ".snapperrc.toml");
}

#[test]
fn wrongly_typed_config_value_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".snapperrc.toml"), "max_width = \"80\"\n").unwrap();
    fs::write(dir.path().join("a.md"), "First one. Second one.\n").unwrap();
    let out = snapper(dir.path(), &["--check", "a.md"]);
    assert_config_error(&out, ".snapperrc.toml");
}

#[test]
fn missing_explicit_config_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "First one. Second one.\n").unwrap();
    let out = snapper(dir.path(), &["--config", "nope.toml", "a.md"]);
    assert_config_error(&out, "nope.toml");
}

#[test]
fn malformed_explicit_config_is_an_error_on_stdin() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.toml"), "lang = [\n").unwrap();
    let out = snapper(
        dir.path(),
        &["--config", "bad.toml", "--format", "markdown"],
    );
    assert_config_error(&out, "bad.toml");
}

#[test]
fn watch_reports_a_malformed_config_and_exits() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.toml"), "max_width = [\n").unwrap();
    fs::write(dir.path().join("a.md"), "First one.\n").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_snapper"))
        .current_dir(dir.path())
        .args(["--config", "bad.toml", "watch", "a.md"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn snapper watch");
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("watch kept running with a malformed config");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!status.success());
    assert!(stderr.contains("bad.toml"), "got:\n{stderr}");
}

#[test]
fn valid_config_still_applies() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".snapperrc.toml"), "max_width = 12\n").unwrap();
    fs::write(dir.path().join("a.txt"), "Alpha beta gamma delta.\n").unwrap();
    let out = snapper(dir.path(), &["a.txt"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Alpha beta\ngamma delta.\n"
    );
}
