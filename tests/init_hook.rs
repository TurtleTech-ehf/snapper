//! The git pre-commit hook that `snapper init` writes.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const STAGED: &str = "# Notes\n\nFirst line. Staged change.\n\nSecond part.\n";
const WORKTREE: &str = "# Notes\n\nFirst line. Staged change.\n\nSecond part. Unstaged work.\n";

fn path_with_snapper() -> OsString {
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_snapper"))
        .parent()
        .expect("snapper bin dir")
        .to_path_buf();
    let mut paths = vec![bin_dir];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(paths).expect("PATH join")
}

fn spawn(program: &Path, args: &[&str], dir: &Path) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("PATH", path_with_snapper())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("spawn")
}

fn run(program: &Path, args: &[&str], dir: &Path) -> Output {
    let out = spawn(program, args, dir);
    assert!(
        out.status.success(),
        "{} {args:?} failed: {}",
        program.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn git(dir: &Path, args: &[&str]) -> String {
    String::from_utf8(run(Path::new("git"), args, dir).stdout).unwrap()
}

/// `git init` plus `snapper init`. Returns the hook path.
fn init_repo(root: &Path) -> PathBuf {
    git(root, &["-c", "init.templateDir=", "init", "-q"]);
    run(Path::new(env!("CARGO_BIN_EXE_snapper")), &["init"], root);
    root.join(git(root, &["rev-parse", "--git-path", "hooks/pre-commit"]).trim())
}

/// A repo with STAGED in the index and WORKTREE on disk.
fn partly_staged_repo(root: &Path) -> PathBuf {
    let hook = init_repo(root);
    fs::write(root.join("notes.md"), STAGED).unwrap();
    git(root, &["-c", "filter.snapper.clean=cat", "add", "notes.md"]);
    fs::write(root.join("notes.md"), WORKTREE).unwrap();
    hook
}

#[test]
fn hook_formats_the_staged_part_and_keeps_unstaged_work_out() {
    let dir = tempfile::tempdir().unwrap();
    let hook = partly_staged_repo(dir.path());
    run(&hook, &[], dir.path());

    assert_eq!(
        git(dir.path(), &["show", ":notes.md"]),
        "# Notes\n\nFirst line.\nStaged change.\n\nSecond part.\n",
        "the index must hold the formatted staged text and nothing unstaged"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("notes.md")).unwrap(),
        WORKTREE,
        "the unstaged edit must stay in the working tree"
    );
}

#[test]
fn hook_failure_on_a_partly_staged_file_leaves_the_index_alone() {
    let dir = tempfile::tempdir().unwrap();
    let hook = partly_staged_repo(dir.path());
    fs::write(dir.path().join(".snapperrc.toml"), "max_width = [\n").unwrap();

    let out = spawn(&hook, &[], dir.path());
    assert!(
        !out.status.success(),
        "a failed format must stop the commit, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(git(dir.path(), &["show", ":notes.md"]), STAGED);
}

#[test]
fn hook_formats_the_working_copy_of_a_file_the_filter_staged() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("notes.md"), "One. Two.\n").unwrap();
    let hook = init_repo(dir.path());
    git(dir.path(), &["add", "notes.md"]);
    assert_eq!(
        git(dir.path(), &["show", ":notes.md"]),
        "One.\nTwo.\n",
        "the clean filter from snapper init must format on add"
    );

    run(&hook, &[], dir.path());
    assert_eq!(
        fs::read_to_string(dir.path().join("notes.md")).unwrap(),
        "One.\nTwo.\n"
    );
    assert_eq!(git(dir.path(), &["show", ":notes.md"]), "One.\nTwo.\n");
}

// Windows file names cannot hold a colon.
#[cfg(unix)]
#[test]
fn hook_stages_a_file_whose_name_starts_like_an_index_stage() {
    let dir = tempfile::tempdir().unwrap();
    let hook = init_repo(dir.path());
    fs::write(dir.path().join("x.md"), "X one. X two.\n").unwrap();
    fs::write(dir.path().join("0:x.md"), "Zero one. Zero two.\n").unwrap();
    git(
        dir.path(),
        &["-c", "filter.snapper.clean=cat", "add", "x.md", "0:x.md"],
    );
    run(&hook, &[], dir.path());

    assert_eq!(
        git(dir.path(), &["show", ":0:0:x.md"]),
        "Zero one.\nZero two.\n",
        "git reads :0:x.md as stage 0 of x.md"
    );
    assert_eq!(git(dir.path(), &["show", ":x.md"]), "X one.\nX two.\n");
}
