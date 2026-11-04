//! Issue 538: a list item or blockquote that ends the file with no final
//! newline must split, and stderr must not report a render check failure.

use std::io::Write;
use std::process::{Command, Stdio};

use snapper_fmt::format::Format;
use snapper_fmt::{FormatConfig, format_text};

fn snapper(input: &str, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_snapper"));
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("failed to spawn snapper");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("failed to wait")
}

fn final_newlines(s: &str) -> usize {
    s.bytes().rev().take_while(|&b| b == b'\n').count()
}

fn markdown_config() -> FormatConfig {
    FormatConfig {
        format: Format::Markdown,
        ..Default::default()
    }
}

fn assert_splits(input: &str, expected: &str) {
    let cfg = markdown_config();
    let library = format_text(input, &cfg).expect("format_text");
    assert_eq!(library, expected, "format_text {input:?}");
    assert_eq!(
        format_text(&library, &cfg).expect("second format_text"),
        library,
        "format_text twice {input:?}"
    );
    assert_eq!(
        final_newlines(&library),
        final_newlines(input),
        "final newlines {input:?}"
    );

    let out = snapper(input, &["--stdin-filepath", "x.md"]);
    let stdout = String::from_utf8(out.stdout).expect("stdout utf-8");
    let stderr = String::from_utf8(out.stderr).expect("stderr utf-8");
    assert!(
        out.status.success(),
        "snapper failed on {input:?}: {stderr}"
    );
    assert_eq!(stdout, expected, "cli {input:?}");
    assert!(
        !stderr.contains("render check failed"),
        "stderr must not report a render check failure for {input:?}: {stderr}"
    );

    let again = snapper(&stdout, &["--stdin-filepath", "x.md"]);
    let twice = String::from_utf8(again.stdout).expect("stdout utf-8");
    assert_eq!(twice, stdout, "cli twice {input:?}");

    let check = snapper(input, &["--check", "--stdin-filepath", "x.md"]);
    let check_err = String::from_utf8(check.stderr).expect("stderr utf-8");
    assert!(
        !check_err.contains("render check failed"),
        "check stderr must not report a render check failure for {input:?}: {check_err}"
    );
    let code = check.status.code().unwrap_or(127);
    if stdout == input {
        assert_eq!(code, 0, "unchanged input exits 0, stderr: {check_err}");
    } else {
        assert_eq!(
            code, 1,
            "a line that would split exits 1, stderr: {check_err}"
        );
        assert!(
            check_err.contains("would reformat"),
            "check names the reformat for {input:?}: {check_err}"
        );
    }
}

#[test]
fn eof_list_and_quote_without_final_newline_splits() {
    let cases = [
        ("- One. Two.", "- One.\n  Two."),
        ("1. One. Two.", "1. One.\n   Two."),
        ("> One. Two.", "> One.\n> Two."),
        ("x\n\n- One. Two.", "x\n\n- One.\n  Two."),
        ("One. Two.", "One.\nTwo."),
        ("- One. Two.\n", "- One.\n  Two.\n"),
        ("1. One. Two.\n", "1. One.\n   Two.\n"),
        ("> One. Two.\n", "> One.\n> Two.\n"),
        ("x\n\n- One. Two.\n", "x\n\n- One.\n  Two.\n"),
        ("- a\n  - One. Two.", "- a\n  - One.\n    Two."),
        ("- a\n  - One. Two.\n", "- a\n  - One.\n    Two.\n"),
        (
            "> One. Two.\nstill quoted.",
            "> One.\n> Two.\n> still quoted.",
        ),
        (
            "> One. Two.\nstill quoted.\n",
            "> One.\n> Two.\n> still quoted.\n",
        ),
        ("intro\n\n> One. Two.", "intro\n\n> One.\n> Two."),
        ("intro\n\n> One. Two.\n", "intro\n\n> One.\n> Two.\n"),
        ("- One.\n\n  Two. Three.", "- One.\n\n  Two.\n  Three."),
        ("- One.\n\n  Two. Three.\n", "- One.\n\n  Two.\n  Three.\n"),
    ];
    for (input, expected) in cases {
        assert_splits(input, expected);
    }
}
