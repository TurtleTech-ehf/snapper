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

#[cfg(unix)]
fn symlink_to(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(unix)]
#[test]
fn changed_check_rejects_a_symlink_to_a_file_outside_the_repo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let outside = dir.path().join("outside.md");
    fs::create_dir_all(&root).unwrap();
    init_repo(&root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(&root, "base");
    let original = "Two sentences. Here.\n";
    fs::write(&outside, original).unwrap();
    symlink_to(&outside, &root.join("notes.md"));
    let check = run(
        &root,
        &["--check", "--changed", "--color", "never", "notes.md"],
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        !check.status.success(),
        "a symlink to a file outside the repository must be rejected: {stderr}"
    );
    assert!(
        stderr.contains("outside the git repository"),
        "the error must say the symlink target is outside the repository, got {stderr}"
    );
    assert!(
        !stderr.contains("would reformat"),
        "the external file must not be checked, got {stderr}"
    );
    assert_eq!(
        fs::read_to_string(&outside).unwrap(),
        original,
        "check must not rewrite the external file"
    );
}

#[cfg(unix)]
#[test]
fn changed_in_place_does_not_overwrite_a_symlink_target_outside_the_repo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let outside = dir.path().join("outside.md");
    fs::create_dir_all(&root).unwrap();
    init_repo(&root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(&root, "base");
    let original = "Two sentences. Here.\n";
    fs::write(&outside, original).unwrap();
    symlink_to(&outside, &root.join("notes.md"));
    let edit = run(
        &root,
        &["--in-place", "--changed", "--color", "never", "notes.md"],
    );
    let stderr = String::from_utf8_lossy(&edit.stderr);
    let after = fs::read_to_string(&outside).unwrap();
    assert_eq!(
        after,
        original,
        "the external file must be left untouched, status {:?}, stderr: {stderr}",
        edit.status.code()
    );
    assert!(
        !edit.status.success(),
        "in-place --changed must refuse a symlink that leaves the repository: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn changed_discovery_skips_one_outside_symlink_and_checks_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let outside = dir.path().join("outside.md");
    fs::create_dir_all(&root).unwrap();
    init_repo(&root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(&root, "base");
    let original = "Two sentences. Here.\n";
    fs::write(&outside, original).unwrap();
    symlink_to(&outside, &root.join("aaa.md"));
    fs::write(root.join("zzz.md"), original).unwrap();
    let check = run(&root, &["--check", "--changed", "--color", "never"]);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("zzz.md"),
        "a sibling prose file must still be checked when one symlink points outside, got {stderr}"
    );
    assert!(
        stderr.contains("would reformat"),
        "the in-repo file must be reported, got {stderr}"
    );
    assert!(
        stderr.contains("outside the git repository"),
        "the outside symlink must be warned, got {stderr}"
    );
    assert!(
        !check.status.success(),
        "skipping an outside symlink still fails the run: {stderr}"
    );
    assert_eq!(fs::read_to_string(&outside).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn changed_discovery_skips_a_relative_symlink_and_checks_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let outside = dir.path().join("outside.md");
    fs::create_dir_all(&root).unwrap();
    init_repo(&root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(&root, "base");
    let original = "Two sentences. Here.\n";
    fs::write(&outside, original).unwrap();
    symlink_to(Path::new("../outside.md"), &root.join("aaa.md"));
    fs::write(root.join("zzz.md"), original).unwrap();
    let check = run(&root, &["--check", "--changed", "--color", "never"]);
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("would reformat") && stderr.contains("zzz.md"),
        "a relative link must not abort discovery of the other file, got {stderr}"
    );
    assert!(
        stderr.contains("outside the git repository"),
        "the relative link must be warned, got {stderr}"
    );
    assert!(!check.status.success(), "{stderr}");
    assert_eq!(fs::read_to_string(&outside).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn changed_skips_a_symlinked_directory_and_checks_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let outside_dir = dir.path().join("out");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside_dir).unwrap();
    init_repo(&root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(&root, "base");
    let original = "Two sentences. Here.\n";
    let page = outside_dir.join("page.md");
    fs::write(&page, original).unwrap();
    symlink_to(&outside_dir, &root.join("sub"));
    fs::write(root.join("zzz.md"), original).unwrap();
    let page_arg = root.join("sub").join("page.md");
    let check = run(
        &root,
        &[
            "--check",
            "--changed",
            "--color",
            "never",
            page_arg.to_str().unwrap(),
            "zzz.md",
        ],
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.contains("zzz.md") && stderr.contains("would reformat"),
        "a symlinked directory must not abort the rest of the run, got {stderr}"
    );
    assert!(
        stderr.contains("outside the git repository"),
        "the file reached through the directory symlink must be warned, got {stderr}"
    );
    assert!(!check.status.success(), "{stderr}");
    assert_eq!(fs::read_to_string(&page).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn changed_in_place_updates_an_in_repo_link_target() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    init_repo(&root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(&root, "base");
    let real = root.join("real.md");
    fs::write(&real, "Two sentences. Here.\n").unwrap();
    symlink_to(Path::new("real.md"), &root.join("link.md"));
    let edit = run(
        &root,
        &["--in-place", "--changed", "--color", "never", "link.md"],
    );
    let stderr = String::from_utf8_lossy(&edit.stderr);
    assert!(
        edit.status.success(),
        "an in-repo link target must be formatted: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(&real).unwrap(),
        "Two sentences.\nHere.\n"
    );
    let meta = fs::symlink_metadata(root.join("link.md")).unwrap();
    assert!(
        meta.file_type().is_symlink(),
        "the link itself must stay a symlink"
    );
}

#[cfg(unix)]
#[test]
fn changed_in_place_keeps_the_file_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    init_repo(&root);
    fs::write(root.join("README.md"), "Fine.\n").unwrap();
    commit_all(&root, "base");
    let original = "Two sentences. Here.\n";
    let expected = "Two sentences.\nHere.\n";
    for (name, mode) in [("secret.md", 0o600), ("script.md", 0o755)] {
        let path = root.join(name);
        fs::write(&path, original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    }
    let edit = run(&root, &["--in-place", "--changed", "--color", "never"]);
    let stderr = String::from_utf8_lossy(&edit.stderr);
    assert!(
        edit.status.success(),
        "in-place --changed must format the prose files: {stderr}"
    );
    for (name, mode) in [("secret.md", 0o600), ("script.md", 0o755)] {
        let path = root.join(name);
        let got = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(got, mode, "{name} must keep mode {mode:04o}, got {got:04o}");
        assert_eq!(fs::read_to_string(&path).unwrap(), expected);
    }
}

#[cfg(unix)]
#[test]
fn in_place_without_changed_does_not_follow_a_symlink() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let outside = root.join("outside.md");
    let original = "Two sentences. Here.\n";
    fs::write(&outside, original).unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o640)).unwrap();
    symlink_to(&outside, &root.join("notes.md"));
    let edit = run(root, &["--in-place", "--color", "never", "notes.md"]);
    let stderr = String::from_utf8_lossy(&edit.stderr);
    assert_eq!(
        fs::read_to_string(&outside).unwrap(),
        original,
        "--in-place must not follow a symlink, status {:?}, stderr: {stderr}",
        edit.status.code()
    );
    let mode = fs::metadata(root.join("notes.md"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, 0o640,
        "plain --in-place must keep the target mode, got {mode:04o}"
    );
}

#[cfg(unix)]
#[test]
fn in_place_refuses_a_hard_link_unless_break_hardlinks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let original = "Two sentences. Here.\n";
    let expected = "Two sentences.\nHere.\n";
    let first = root.join("a.md");
    let second = root.join("b.md");
    fs::write(&first, original).unwrap();
    fs::hard_link(&first, &second).unwrap();
    let refused = run(root, &["--in-place", "--color", "never", "a.md"]);
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        !refused.status.success(),
        "a hard link must exit non-zero, stderr: {stderr}"
    );
    assert!(
        stderr.contains("hard-linked") && stderr.contains("a.md"),
        "the error must name the hard-linked file, got {stderr}"
    );
    assert_eq!(fs::read_to_string(&first).unwrap(), original);
    assert_eq!(fs::read_to_string(&second).unwrap(), original);
    let broken = run(
        root,
        &[
            "--in-place",
            "--break-hardlinks",
            "--color",
            "never",
            "a.md",
        ],
    );
    let stderr = String::from_utf8_lossy(&broken.stderr);
    assert!(
        broken.status.success(),
        "--break-hardlinks must replace this path only: {stderr}"
    );
    assert_eq!(fs::read_to_string(&first).unwrap(), expected);
    assert_eq!(
        fs::read_to_string(&second).unwrap(),
        original,
        "the other name must keep the old text"
    );
    assert_eq!(nlink(&first), 1);
    assert_eq!(nlink(&second), 1);
}

#[cfg(unix)]
fn nlink(path: &Path) -> u64 {
    let meta = fs::metadata(path).unwrap();
    #[cfg(target_os = "linux")]
    {
        std::os::linux::fs::MetadataExt::st_nlink(&meta)
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::os::unix::fs::MetadataExt::nlink(&meta)
    }
}
