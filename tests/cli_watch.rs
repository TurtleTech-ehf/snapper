//! `snapper watch` rewrites a watched file the way the CLI formats it.

use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn watch_applies_the_project_code_settings() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".snapperrc.toml"),
        "[code.python]\nline_comment = \"#\"\n",
    )
    .unwrap();
    let text = "First one. Second one.\n\n```python\n# A one. B two.\nx = 1\n```\n";
    let file = dir.path().join("a.md");
    fs::write(&file, "Start.\n").unwrap();

    fs::write(dir.path().join("copy.md"), text).unwrap();
    let cli = Command::new(env!("CARGO_BIN_EXE_snapper"))
        .current_dir(dir.path())
        .arg("copy.md")
        .output()
        .unwrap();
    let expected = String::from_utf8(cli.stdout).unwrap();
    assert!(expected.contains("# A one.\n# B two.\n"), "{expected}");

    let mut child = Command::new(env!("CARGO_BIN_EXE_snapper"))
        .current_dir(dir.path())
        .args(["watch", "a.md"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn snapper watch");
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    stderr.read_line(&mut line).unwrap();
    assert!(line.starts_with("Watching"), "{line}");
    std::thread::spawn(move || std::io::copy(&mut stderr, &mut std::io::sink()));
    std::thread::sleep(Duration::from_millis(500));

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut last_write = Instant::now() - Duration::from_secs(10);
    let result = loop {
        let current = fs::read_to_string(&file).unwrap();
        if current != text && current != "Start.\n" {
            break current;
        }
        if Instant::now() > deadline {
            break current;
        }
        if current != text || last_write.elapsed() > Duration::from_secs(3) {
            fs::write(&file, text).unwrap();
            last_write = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    child.kill().ok();
    child.wait().ok();
    assert_eq!(result, expected);
}
