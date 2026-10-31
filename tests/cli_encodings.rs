//! Encoding and line-ending details survive the real binary unchanged.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(input: &[u8], args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_snapper"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn snapper");
    child.stdin.take().unwrap().write_all(input).unwrap();
    child
        .wait_with_output()
        .expect("failed to wait for snapper")
}

fn format(input: &str, args: &[&str]) -> String {
    let out = run(input.as_bytes(), args);
    assert!(
        out.status.success(),
        "snapper failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("output is UTF-8")
}

#[test]
fn bom_does_not_hide_a_markdown_heading() {
    let input = "\u{feff}# Title\nFirst one. Second one.\n";
    assert_eq!(
        format(input, &["--format", "markdown"]),
        "\u{feff}# Title\nFirst one.\nSecond one.\n"
    );
}

#[test]
fn bom_does_not_hide_an_org_keyword() {
    let input = "\u{feff}#+TITLE: Foo. Bar.\n* Head\nFirst one. Second one.\n";
    assert_eq!(
        format(input, &["--format", "org"]),
        "\u{feff}#+TITLE: Foo. Bar.\n* Head\nFirst one.\nSecond one.\n"
    );
}

#[test]
fn bom_alone_and_bom_with_crlf_round_trip() {
    assert_eq!(format("\u{feff}", &["--format", "markdown"]), "\u{feff}");
    assert_eq!(
        format(
            "\u{feff}# T\r\n\r\nA one. B two.\r\n",
            &["--format", "markdown"]
        ),
        "\u{feff}# T\r\n\r\nA one.\r\nB two.\r\n"
    );
}

#[test]
fn check_passes_on_a_formatted_file_with_a_bom() {
    let input = "\u{feff}#+TITLE: Foo. Bar.\n* Head\nFirst one.\nSecond one.\n";
    let out = run(input.as_bytes(), &["--check", "--format", "org"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "no diagnostics expected, got:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn range_keeps_crlf_outside_and_inside_the_range() {
    let input = "# T\r\n\r\nFirst one. Second one.\r\nKeep. This.\r\n";
    assert_eq!(
        format(input, &["--format", "markdown", "--range", "3:3"]),
        "# T\r\n\r\nFirst one.\r\nSecond one.\r\nKeep. This.\r\n"
    );
}

#[test]
fn range_on_the_last_crlf_line_without_a_final_newline() {
    let input = "Keep. This.\r\nA one. B two.";
    assert_eq!(
        format(input, &["--format", "plaintext", "--range", "2:2"]),
        "Keep. This.\r\nA one.\r\nB two."
    );
}

#[test]
fn text_ending_in_a_multibyte_character_without_a_final_newline() {
    let input = "中文。日本語。";
    for format_arg in ["markdown", "org", "plaintext"] {
        assert_eq!(
            format(input, &["--format", format_arg]),
            input,
            "--format {format_arg}"
        );
    }
}
