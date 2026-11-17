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
