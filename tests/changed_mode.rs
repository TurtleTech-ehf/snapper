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
    let crlf = git(dir, &["config", "core.autocrlf", "false"]);
    assert!(
        crlf.status.success(),
        "git config: {}",
        String::from_utf8_lossy(&crlf.stderr)
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

#[test]
fn a_rename_does_not_reflow_the_untouched_paragraph() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(
        root.join("old.md"),
        "Old fused sentence. Still old.\n\nFresh sentence.\n",
    )
    .unwrap();
    commit_all(root, "base");
    let moved = git(root, &["mv", "old.md", "new.md"]);
    assert!(
        moved.status.success(),
        "git mv: {}",
        String::from_utf8_lossy(&moved.stderr)
    );
    let pure = run(root, &["--check", "--changed", "new.md"]);
    assert!(
        pure.status.success(),
        "a pure rename is not a new file: {}",
        String::from_utf8_lossy(&pure.stderr)
    );

    fs::write(
        root.join("new.md"),
        "Old fused sentence. Still old.\n\nFresh sentence. Now fused.\n",
    )
    .unwrap();
    let write = run(root, &["--in-place", "--changed", "new.md"]);
    assert!(
        write.status.success(),
        "{}",
        String::from_utf8_lossy(&write.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("new.md")).unwrap(),
        "Old fused sentence. Still old.\n\nFresh sentence.\nNow fused.\n"
    );
}

#[test]
fn changed_from_uses_the_merge_base() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(root.join("notes.md"), "Fused sentence. Still fused.\n").unwrap();
    commit_all(root, "base");
    let branch = git(root, &["checkout", "-b", "feature"]);
    assert!(
        branch.status.success(),
        "{}",
        String::from_utf8_lossy(&branch.stderr)
    );
    let back = git(root, &["checkout", "main"]);
    assert!(
        back.status.success(),
        "{}",
        String::from_utf8_lossy(&back.stderr)
    );
    fs::write(root.join("notes.md"), "Split sentence.\nStill split.\n").unwrap();
    commit_all(root, "split");
    let feature = git(root, &["checkout", "feature"]);
    assert!(
        feature.status.success(),
        "{}",
        String::from_utf8_lossy(&feature.stderr)
    );

    let check = run(
        root,
        &["--check", "--changed", "--changed-from", "main", "notes.md"],
    );
    assert!(
        check.status.success(),
        "the branch did not edit notes.md: {}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn in_place_changed_keeps_crlf() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    let paper = root.join("paper.md");
    fs::write(
        &paper,
        "Old fused sentence. Still old.\r\n\r\nFresh sentence.\r\n",
    )
    .unwrap();
    commit_all(root, "base");
    fs::write(
        &paper,
        "Old fused sentence. Still old.\r\n\r\nFresh sentence. Now fused.\r\n",
    )
    .unwrap();
    let write = run(root, &["--in-place", "--changed", "paper.md"]);
    assert!(
        write.status.success(),
        "{}",
        String::from_utf8_lossy(&write.stderr)
    );
    assert_eq!(
        fs::read_to_string(&paper).unwrap(),
        "Old fused sentence. Still old.\r\n\r\nFresh sentence.\r\nNow fused.\r\n"
    );
}

#[test]
fn a_bracket_name_does_not_take_another_files_hunk() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(
        root.join("note[1].md"),
        "Line one is fine.\n\nTwo sentences. On one line.\n",
    )
    .unwrap();
    fs::write(root.join("note1.md"), "Hello.\n\nWorld.\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("note1.md"), "Hello.\n\nWorld changed.\n").unwrap();

    let check = run(root, &["--check", "--changed", "note[1].md"]);
    assert!(
        check.status.success(),
        "note[1].md was not edited: {}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn an_external_diff_driver_does_not_hide_the_edit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(root.join(".gitattributes"), "*.md diff=nodiff\n").unwrap();
    let cfg = git(root, &["config", "diff.nodiff.command", "true"]);
    assert!(
        cfg.status.success(),
        "git config: {}",
        String::from_utf8_lossy(&cfg.stderr)
    );
    fs::write(root.join("paper.md"), "Already fine.\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("paper.md"), "Already fine. Now fused.\n").unwrap();

    let check = run(root, &["--check", "--changed", "paper.md"]);
    assert_eq!(
        check.status.code(),
        Some(1),
        "the fused edit must still fail: {}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn deleting_a_line_still_checks_the_paragraph() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(
        root.join("paper.md"),
        "Old fused sentence. Still old.\n\nFused sentence. Still fused.\nTrailing line.\n",
    )
    .unwrap();
    commit_all(root, "base");
    fs::write(
        root.join("paper.md"),
        "Old fused sentence. Still old.\n\nFused sentence. Still fused.\n",
    )
    .unwrap();
    let check = run(root, &["--check", "--changed", "--color", "never"]);
    assert_eq!(
        check.status.code(),
        Some(1),
        "a deletion-only hunk must still check the paragraph: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("Fused sentence"),
        "the paragraph that lost a line must be reported: {stderr}"
    );
    assert!(
        !stderr.contains("Old fused"),
        "the untouched paragraph must stay quiet: {stderr}"
    );
}

#[test]
fn an_unborn_head_still_checks_untracked_prose() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(root.join("notes.md"), "Two sentences. Here.\n").unwrap();
    let check = run(root, &["--check", "--changed", "--color", "never"]);
    assert_eq!(
        check.status.code(),
        Some(1),
        "untracked prose in a repo with no commits must fail --check: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("would reformat"),
        "the file must be checked, not rejected by merge-base: {stderr}"
    );
    assert!(
        !stderr.contains("merge-base") && !stderr.contains("Not a valid object name"),
        "{stderr}"
    );
}

#[test]
fn changed_paths_are_printed_relative_to_the_repo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(root, "base");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/notes.md"), "Two sentences. Here.\n").unwrap();
    let head = git(root, &["rev-parse", "--verify", "HEAD"]);
    assert!(
        head.status.success(),
        "this test must have a commit, so a broken unborn-HEAD path cannot hide the path assertion: {}",
        String::from_utf8_lossy(&head.stderr)
    );
    let check = run(root, &["--check", "--changed", "--color", "never"]);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("docs/notes.md"),
        "the path must be repo-relative, got {stderr}"
    );
    assert!(
        !stderr.contains(root.to_str().unwrap()),
        "the path must not be absolute, got {stderr}"
    );
}

#[test]
fn changed_rejects_a_file_from_another_repository() {
    let dir = tempfile::tempdir().unwrap();
    let here = dir.path().join("here");
    let there = dir.path().join("there");
    fs::create_dir_all(&here).unwrap();
    fs::create_dir_all(&there).unwrap();
    init_repo(&here);
    init_repo(&there);
    fs::write(here.join("README.md"), "Fine.\n").unwrap();
    commit_all(&here, "base");
    fs::write(there.join("note.md"), "Fine.\n").unwrap();
    commit_all(&there, "base");
    fs::write(there.join("note.md"), "Two sentences. Here.\n").unwrap();
    let foreign = there.join("note.md");
    let check = run(
        &here,
        &[
            "--check",
            "--changed",
            "--color",
            "never",
            foreign.to_str().unwrap(),
        ],
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        !check.status.success(),
        "a file from another repository must be rejected: {stderr}"
    );
    assert!(
        stderr.contains("different git repository"),
        "the error must say the file is in a different repository, got {stderr}"
    );
    assert!(
        !stderr.contains("would reformat"),
        "the other repository's prose must not be checked, got {stderr}"
    );
}

#[test]
fn changed_checks_a_path_with_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(root, "base");
    fs::write(root.join("my notes.md"), "Two sentences. Here.\n").unwrap();
    let check = run(
        root,
        &["--check", "--changed", "--color", "never", "my notes.md"],
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert_eq!(
        check.status.code(),
        Some(1),
        "a changed file whose name has a space must be checked: {stderr}"
    );
    assert!(
        stderr.contains("my notes.md"),
        "the spaced path must be printed whole, got {stderr}"
    );
    assert!(
        stderr.contains("would reformat"),
        "the spaced path must reach the check, got {stderr}"
    );
}

#[test]
fn changed_rejects_a_path_outside_the_repository() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let outside = dir.path().join("loose notes.md");
    fs::create_dir_all(&root).unwrap();
    init_repo(&root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(&root, "base");
    fs::write(&outside, "Two sentences. Here.\n").unwrap();
    let check = run(
        &root,
        &[
            "--check",
            "--changed",
            "--color",
            "never",
            outside.to_str().unwrap(),
        ],
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        !check.status.success(),
        "a path outside the repository must be rejected: {stderr}"
    );
    assert!(
        stderr.contains("outside the git repository"),
        "the error must say the path is outside the repository, got {stderr}"
    );
    assert!(
        stderr.contains("loose notes.md"),
        "the error must include the path, spaces and all, got {stderr}"
    );
    assert!(
        !stderr.contains("would reformat"),
        "prose outside the repository must not be checked, got {stderr}"
    );
}
