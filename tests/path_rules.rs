//! `[[path]]` rules in `.snapperrc.toml` override width, format, and ignore.

use std::fs;
use std::path::Path;
use std::process::Command;

fn snapper() -> Command {
    Command::new(env!("CARGO_BIN_EXE_snapper"))
}

fn run(dir: &Path, args: &[&str]) -> std::process::Output {
    snapper()
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to run snapper")
}

#[test]
fn a_path_rule_wraps_one_directory_and_leaves_another() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join(".snapperrc.toml"),
        r#"
[[path]]
glob = "narrow/**/*.md"
max_width = 20
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("narrow")).unwrap();
    fs::create_dir_all(root.join("wide")).unwrap();
    let sentence = "This sentence is definitely wider than twenty characters.\n";
    fs::write(root.join("narrow/note.md"), sentence).unwrap();
    fs::write(root.join("wide/note.md"), sentence).unwrap();

    let narrow = run(root, &["--native", "narrow/note.md"]);
    assert!(
        narrow.status.success(),
        "{}",
        String::from_utf8_lossy(&narrow.stderr)
    );
    let narrow_out = String::from_utf8(narrow.stdout).unwrap();
    assert!(
        narrow_out.contains('\n') && narrow_out.lines().count() > 1,
        "narrow/note.md must wrap, got {narrow_out:?}"
    );

    let wide = run(root, &["--native", "wide/note.md"]);
    assert!(wide.status.success());
    assert_eq!(String::from_utf8(wide.stdout).unwrap(), sentence);
}

#[test]
fn a_path_rule_can_select_the_format_and_skip_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join(".snapperrc.toml"),
        r#"
[[path]]
glob = "notes/*.txt"
format = "markdown"

[[path]]
glob = "skip/*.md"
ignore = true
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::create_dir_all(root.join("skip")).unwrap();
    fs::write(root.join("notes/a.txt"), "- One. Two.\n").unwrap();
    fs::write(root.join("skip/bad.md"), "Two sentences. Here.\n").unwrap();

    let notes = run(root, &["--native", "notes/a.txt"]);
    assert!(
        notes.status.success(),
        "{}",
        String::from_utf8_lossy(&notes.stderr)
    );
    assert_eq!(String::from_utf8(notes.stdout).unwrap(), "- One.\n  Two.\n");

    let skipped = run(root, &["--check", "skip/bad.md"]);
    assert!(
        skipped.status.success(),
        "ignore must skip the file: {}",
        String::from_utf8_lossy(&skipped.stderr)
    );

    fs::write(
        root.join(".snapperrc.toml"),
        "[[path]]\nglob = \"bad.md\"\nformat = \"not-a-format\"\n",
    )
    .unwrap();
    fs::write(root.join("bad.md"), "One sentence.\n").unwrap();
    let bad = run(root, &["bad.md"]);
    assert_eq!(bad.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        stderr.contains("unknown format"),
        "expected a format error, got {stderr}"
    );
}

#[test]
fn a_relative_path_matches_from_the_config_root() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join(".snapperrc.toml"),
        "[[path]]\nglob = \"docs/**/*.md\"\nmax_width = 20\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();
    let sentence = "This sentence is definitely wider than twenty characters.\n";
    fs::write(root.join("docs/note.md"), sentence).unwrap();

    let output = snapper()
        .current_dir(root.join("docs"))
        .args(["--native", "note.md"])
        .output()
        .expect("failed to run snapper");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.lines().count() > 1,
        "docs/note.md must wrap when named from that directory, got {text:?}"
    );
}

#[test]
fn a_broken_config_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join(".snapperrc.toml"), "[[path]]\nglob = [\n").unwrap();
    fs::write(root.join("note.md"), "One sentence.\n").unwrap();
    let output = run(root, &["note.md"]);
    assert!(
        !output.status.success(),
        "invalid TOML must fail the command"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("toml") || stderr.contains("snapperrc"),
        "expected a config error, got {stderr}"
    );
}

#[test]
fn an_unsupported_glob_warns_without_failing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join(".snapperrc.toml"),
        "[[path]]\nglob = \"docs/{a,b}/*.md\"\nmax_width = 40\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("docs/a")).unwrap();
    fs::write(root.join("docs/a/note.md"), "One sentence.\n").unwrap();
    let output = run(root, &["--native", "docs/a/note.md"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not fully supported"),
        "expected a glob warning, got {stderr}"
    );
}

#[test]
fn stdin_filepath_honors_ignore() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join(".snapperrc.toml"),
        "[[path]]\nglob = \"skip/*.md\"\nignore = true\n",
    )
    .unwrap();
    let input = "Two sentences. Here.\n";
    let mut child = snapper()
        .current_dir(root)
        .args(["--check", "--stdin-filepath", "skip/a.md"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn snapper");
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let check = child.wait_with_output().unwrap();
    assert!(
        check.status.success(),
        "an ignored stdin path must pass --check: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        !stderr.contains("would reformat"),
        "ignore must not report the file: {stderr}"
    );

    let mut child = snapper()
        .current_dir(root)
        .args(["--native", "--stdin-filepath", "skip/a.md"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn snapper");
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let passed = child.wait_with_output().unwrap();
    assert!(
        passed.status.success(),
        "{}",
        String::from_utf8_lossy(&passed.stderr)
    );
    assert_eq!(String::from_utf8(passed.stdout).unwrap(), input);
}
