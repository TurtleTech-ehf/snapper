//! `--changed` reports and reflows only paragraphs git marks as edited.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn snapper() -> Command {
    Command::new(env!("CARGO_BIN_EXE_snapper"))
}

fn git(dir: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.email=snapper-test@example.com",
            "-c",
            "user.name=Snapper Test",
        ])
        .args(args)
        .env("GIT_AUTHOR_NAME", "Snapper Test")
        .env("GIT_AUTHOR_EMAIL", "snapper-test@example.com")
        .env("GIT_COMMITTER_NAME", "Snapper Test")
        .env("GIT_COMMITTER_EMAIL", "snapper-test@example.com");
    cmd.output().expect("failed to run git")
}

fn init_repo(dir: &Path) {
    let init = git(dir, &["init", "-b", "main"]);
    assert!(
        init.status.success(),
        "git init: {}",
        String::from_utf8_lossy(&init.stderr)
    );
}

fn commit_all(dir: &Path, message: &str) {
    let add = git(dir, &["add", "-A"]);
    assert!(
        add.status.success(),
        "git add: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let commit = git(dir, &["commit", "-m", message]);
    assert!(
        commit.status.success(),
        "git commit: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
}

fn run(dir: &Path, args: &[&str]) -> Output {
    snapper()
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to run snapper")
}

#[test]
fn an_untouched_fused_paragraph_does_not_fail() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    let paper = root.join("paper.md");
    fs::write(
        &paper,
        "Old fused sentence. Still old.\n\nFresh sentence.\n",
    )
    .unwrap();
    commit_all(root, "base");
    fs::write(
        &paper,
        "Old fused sentence. Still old.\n\nFresh sentence now.\n",
    )
    .unwrap();

    let changed = run(root, &["--check", "--changed", "paper.md"]);
    assert!(
        changed.status.success(),
        "touched paragraph is already one sentence: {}",
        String::from_utf8_lossy(&changed.stderr)
    );
    let whole = run(root, &["--check", "paper.md"]);
    assert_eq!(
        whole.status.code(),
        Some(1),
        "the historical paragraph still fails a whole-file check"
    );
}

#[test]
fn an_edit_that_fuses_a_sentence_fails_and_in_place_fixes_only_that_paragraph() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    let paper = root.join("paper.md");
    fs::write(&paper, "Leave this fused. Please.\n\nAlready fine.\n").unwrap();
    commit_all(root, "base");
    fs::write(
        &paper,
        "Leave this fused. Please.\n\nAlready fine. And now fused.\n",
    )
    .unwrap();

    let changed = run(
        root,
        &["--check", "--changed", "--color", "never", "paper.md"],
    );
    assert_eq!(changed.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&changed.stderr);
    assert!(
        stderr.contains("would reformat:"),
        "expected a reformat notice, got {stderr}"
    );
    assert!(
        !stderr.contains("Leave this fused"),
        "the untouched paragraph must not be reported: {stderr}"
    );

    let write = run(root, &["--in-place", "--changed", "paper.md"]);
    assert!(
        write.status.success(),
        "{}",
        String::from_utf8_lossy(&write.stderr)
    );
    let text = fs::read_to_string(&paper).unwrap();
    assert_eq!(
        text,
        "Leave this fused. Please.\n\nAlready fine.\nAnd now fused.\n"
    );

    let again = run(root, &["--check", "--changed", "paper.md"]);
    assert!(
        again.status.success(),
        "the edited paragraph is formatted: {}",
        String::from_utf8_lossy(&again.stderr)
    );
}

#[test]
fn no_arguments_checks_untracked_prose_and_skips_a_clean_tree() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(root.join("kept.md"), "Old fused sentence. Still old.\n").unwrap();
    commit_all(root, "base");

    let clean = run(root, &["--check", "--changed"]);
    assert!(
        clean.status.success(),
        "a clean tree has nothing to check: {}",
        String::from_utf8_lossy(&clean.stderr)
    );
    let notice = String::from_utf8_lossy(&clean.stderr);
    assert!(
        notice.contains("No prose files changed"),
        "expected the empty notice, got {notice}"
    );

    fs::write(root.join("notes.md"), "Brand new. Two sentences.\n").unwrap();
    let dirty = run(root, &["--check", "--changed", "--output-format", "json"]);
    assert_eq!(dirty.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&dirty.stdout);
    assert!(
        stdout.contains("notes.md"),
        "the untracked file must be in the JSON report: {stdout}"
    );
    assert!(
        !stdout.contains("kept.md"),
        "the untouched file must stay out of the report: {stdout}"
    );
}

#[test]
fn a_ref_git_cannot_resolve_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(root.join("paper.md"), "One sentence.\n").unwrap();
    commit_all(root, "base");
    let output = run(
        root,
        &[
            "--check",
            "--changed",
            "--changed-from",
            "does-not-exist",
            "paper.md",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error:"),
        "expected a git error, got {stderr}"
    );
}
