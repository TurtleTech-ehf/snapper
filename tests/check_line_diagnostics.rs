//! CLI `--check` line-level fused / wrap / long diagnostics.

use std::fs;
use std::path::Path;
use std::process::Command;

fn snapper_binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_snapper"))
}

fn write_txt(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path.to_string_lossy().into_owned()
}

fn check_json(path: &str, extra: &[&str]) -> (bool, serde_json::Value, String) {
    let mut args = vec![
        "--check",
        "--output-format",
        "json",
        "--format",
        "plaintext",
        path,
    ];
    args.extend_from_slice(extra);
    let output = snapper_binary()
        .args(&args)
        .output()
        .expect("failed to run snapper");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("expected JSON on stdout, got {stdout:?} stderr={stderr:?}: {e}")
    });
    (output.status.success(), parsed, stderr)
}

fn diagnostics(parsed: &serde_json::Value) -> Vec<&serde_json::Value> {
    parsed
        .as_array()
        .expect("json check output is a file array")
        .iter()
        .flat_map(|file| {
            file.get("diagnostics")
                .and_then(|d| d.as_array())
                .into_iter()
                .flatten()
        })
        .collect()
}

#[test]
fn check_json_emits_fused_line_kind_excerpt() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(dir.path(), "fused.txt", "Hello world. This is a test.\n");
    let (ok, parsed, _) = check_json(&path, &[]);
    assert!(!ok, "fused prose should fail --check");
    let diags = diagnostics(&parsed);
    let fused = diags.iter().find(|d| d["kind"] == "fused");
    let fused = fused.unwrap_or_else(|| panic!("missing fused diagnostic in {parsed}"));
    assert_eq!(fused["line"], 1);
    let excerpt = fused["excerpt"].as_str().unwrap();
    assert!(excerpt.contains("Hello world"), "excerpt={excerpt:?}");
}

#[test]
fn check_json_fused_is_abbrev_aware() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(dir.path(), "fig.txt", "See Fig. 3 for details.\n");
    let (ok, parsed, _) = check_json(&path, &[]);
    assert!(ok, "a single abbreviated sentence should pass --check");
    let diags = diagnostics(&parsed);
    assert!(
        diags.iter().all(|d| d["kind"] != "fused"),
        "Fig. must not be fused: {parsed}"
    );
}

#[test]
fn check_json_emits_wrap_on_mid_clause() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(
        dir.path(),
        "wrap.txt",
        "The experiment ran for several\nweeks using the usual protocol.\n",
    );
    let (ok, parsed, _) = check_json(&path, &[]);
    assert!(!ok, "wrap should fail --check (identity changes on join)");
    let diags = diagnostics(&parsed);
    let wrap = diags.iter().find(|d| d["kind"] == "wrap");
    let wrap = wrap.unwrap_or_else(|| panic!("missing wrap diagnostic in {parsed}"));
    assert_eq!(wrap["line"], 2);
    assert!(
        wrap["excerpt"].as_str().unwrap().contains("weeks"),
        "excerpt={wrap}"
    );
}

#[test]
fn check_json_wrap_skips_connector_and() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(
        dir.path(),
        "and.txt",
        "The experiment ran for several weeks\nand used the usual protocol.\n",
    );
    let (_ok, parsed, _) = check_json(&path, &[]);
    let diags = diagnostics(&parsed);
    assert!(
        diags.iter().all(|d| d["kind"] != "wrap"),
        "connector-led and is not wrap: {parsed}"
    );
}

#[test]
fn check_json_long_is_advisory_without_strict() {
    let dir = tempfile::tempdir().unwrap();
    let body = "The quick brown fox jumps over the lazy dog, then continues running across a very long meadow without pausing for breath at all today.\n";
    assert!(body.trim_end().chars().count() > 120);
    let path = write_txt(dir.path(), "long.txt", body);
    let (ok, parsed, _) = check_json(&path, &[]);
    assert!(ok, "long alone must not fail --check unless --strict-long");
    let diags = diagnostics(&parsed);
    let long = diags.iter().find(|d| d["kind"] == "long");
    let long = long.unwrap_or_else(|| panic!("missing long diagnostic in {parsed}"));
    assert_eq!(long["line"], 1);
    assert!(
        long["excerpt"]
            .as_str()
            .unwrap()
            .contains("quick brown fox"),
        "excerpt={long}"
    );
}

#[test]
fn check_strict_long_fails_on_advisory_long() {
    let dir = tempfile::tempdir().unwrap();
    let body = "The quick brown fox jumps over the lazy dog, then continues running across a very long meadow without pausing for breath at all today.\n";
    let path = write_txt(dir.path(), "long.txt", body);
    let (ok, parsed, _) = check_json(&path, &["--strict-long"]);
    assert!(!ok, "--strict-long must fail when a long diagnostic exists");
    assert!(
        diagnostics(&parsed).iter().any(|d| d["kind"] == "long"),
        "expected long diagnostic under --strict-long: {parsed}"
    );
}

#[test]
fn check_sarif_includes_start_line_and_kind() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(dir.path(), "fused.txt", "Hello world. This is a test.\n");
    let output = snapper_binary()
        .args([
            "--check",
            "--output-format",
            "sarif",
            "--format",
            "plaintext",
            &path,
        ])
        .output()
        .expect("failed to run snapper");
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("sarif json");
    let results = parsed["runs"][0]["results"]
        .as_array()
        .expect("sarif results");
    let fused = results
        .iter()
        .find(|r| r["ruleId"] == "snapper/fused")
        .unwrap_or_else(|| panic!("missing snapper/fused in {parsed}"));
    assert_eq!(
        fused["locations"][0]["physicalLocation"]["region"]["startLine"],
        1
    );
    let snippet = fused["locations"][0]["physicalLocation"]["region"]["snippet"]["text"]
        .as_str()
        .unwrap_or("");
    assert!(
        snippet.contains("Hello world"),
        "sarif snippet should carry the excerpt, got {snippet:?}"
    );
}

fn check_json_fmt(path: &str, format: &str) -> (bool, serde_json::Value) {
    let output = snapper_binary()
        .args([
            "--check",
            "--output-format",
            "json",
            "--format",
            format,
            path,
        ])
        .output()
        .expect("failed to run snapper");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("expected JSON on stdout, got {stdout:?} stderr={stderr:?}: {e}")
    });
    (output.status.success(), parsed)
}

#[test]
fn numbered_list_markdown_is_not_fused_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(dir.path(), "list.md", "1. Hello world.\n");
    let (ok, parsed) = check_json_fmt(&path, "markdown");
    assert!(ok, "1. Hello world. must pass --check: {parsed}");
    assert!(
        diagnostics(&parsed).iter().all(|d| d["kind"] != "fused"),
        "numbered list must not fused: {parsed}"
    );
    if let Some(arr) = parsed.as_array() {
        for file in arr {
            assert_ne!(file["would_reformat"], true, "{parsed}");
        }
    }
}

#[test]
fn numbered_list_org_is_not_fused_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(dir.path(), "list.org", "1. Hello world.\n");
    let (ok, parsed) = check_json_fmt(&path, "org");
    assert!(ok, "1. Hello world. must pass --check: {parsed}");
    assert!(
        diagnostics(&parsed).iter().all(|d| d["kind"] != "fused"),
        "org numbered list must not fused: {parsed}"
    );
}

#[test]
fn latex_mid_line_comment_is_not_fused_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(dir.path(), "cite.tex", "See Fig. 1. % TODO cite\n");
    let (ok, parsed) = check_json_fmt(&path, "latex");
    assert!(ok, "See Fig. 1. % TODO cite must pass --check: {parsed}");
    assert!(
        diagnostics(&parsed).iter().all(|d| d["kind"] != "fused"),
        "latex comment must not fused: {parsed}"
    );
}

#[test]
fn would_reformat_matches_cli_check_identity() {
    let dir = tempfile::tempdir().unwrap();
    let dirty = write_txt(dir.path(), "dirty.txt", "Hello world. This is a test.\n");
    let clean = write_txt(dir.path(), "clean.txt", "Hello world.\nThis is a test.\n");

    let dirty_out = snapper_binary()
        .args(["--check", "--format", "plaintext", &dirty])
        .output()
        .unwrap();
    let clean_out = snapper_binary()
        .args(["--check", "--format", "plaintext", &clean])
        .output()
        .unwrap();
    assert!(!dirty_out.status.success());
    assert!(clean_out.status.success());

    let (_ok, dirty_json, _) = check_json(&dirty, &[]);
    let file = &dirty_json.as_array().unwrap()[0];
    assert_eq!(file["would_reformat"], true);

    let clean_json_run = snapper_binary()
        .args([
            "--check",
            "--output-format",
            "json",
            "--format",
            "plaintext",
            &clean,
        ])
        .output()
        .unwrap();
    assert!(clean_json_run.status.success());
    let stdout = String::from_utf8(clean_json_run.stdout).unwrap();
    if !stdout.trim().is_empty() {
        let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        if let Some(arr) = parsed.as_array() {
            for file in arr {
                assert_ne!(file["would_reformat"], true);
            }
        }
    }
}

fn pipe_check(input: &str, extra: &[&str]) -> (bool, String, String) {
    let mut cmd = snapper_binary();
    let mut args = vec!["--check", "--format", "plaintext"];
    args.extend_from_slice(extra);
    cmd.args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn snapper");
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let out = child.wait_with_output().expect("wait snapper");
    (
        out.status.success(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn stdin_check_json_emits_diagnostics_and_exit_1() {
    let (ok, stdout, stderr) = pipe_check(
        "Hello world. This is a test.\n",
        &["--output-format", "json"],
    );
    assert!(!ok, "stdin fused check must exit 1, stderr={stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("stdin json: {stdout:?}: {e}"));
    let diags = diagnostics(&parsed);
    assert!(
        diags.iter().any(|d| d["kind"] == "fused" && d["line"] == 1),
        "stdin json must include fused, got {parsed}"
    );
}

#[test]
fn stdin_check_json_clean_exits_0() {
    let (ok, stdout, stderr) = pipe_check(
        "Hello world.\nThis is a test.\n",
        &["--output-format", "json"],
    );
    assert!(ok, "clean stdin check must exit 0, stderr={stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("stdin json: {stdout:?}: {e}"));
    assert!(
        parsed.as_array().is_some(),
        "clean stdin json must be an array, got {parsed}"
    );
}

#[test]
fn strict_long_does_not_fail_structure_equation() {
    let dir = tempfile::tempdir().unwrap();
    let body = concat!(
        "\\documentclass{article}\n",
        "\\begin{document}\n",
        "\\begin{equation}\n",
        "E = mc^2 + a very long expression, with commas, that exceeds one hundred twenty characters easily xxxxxxxxxxxxxxxxx\n",
        "\\end{equation}\n",
        "\\end{document}\n",
    );
    let path = dir.path().join("eq.tex");
    fs::write(&path, body).unwrap();
    let output = snapper_binary()
        .args([
            "--check",
            "--strict-long",
            "--output-format",
            "json",
            "--format",
            "latex",
            &path.to_string_lossy(),
        ])
        .output()
        .expect("run snapper");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "--strict-long must not fail a structure-only equation, stderr={stderr} stdout={stdout}"
    );
    if !stdout.trim().is_empty() {
        let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let diags = diagnostics(&parsed);
        assert!(
            diags.iter().all(|d| d["kind"] != "long"),
            "equation must not yield long: {parsed}"
        );
        for file in parsed.as_array().unwrap() {
            assert_ne!(file["would_reformat"], true, "{parsed}");
        }
    }
}

#[test]
fn sarif_uri_is_repo_relative_or_file_with_workdir() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_txt(dir.path(), "fused.txt", "Hello world. This is a test.\n");
    let output = snapper_binary()
        .current_dir(dir.path())
        .args([
            "--check",
            "--output-format",
            "sarif",
            "--format",
            "plaintext",
            "fused.txt",
        ])
        .output()
        .expect("run snapper");
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("sarif json");
    let uri = parsed["runs"][0]["results"][0]["locations"][0]["physicalLocation"]
        ["artifactLocation"]["uri"]
        .as_str()
        .unwrap_or("");
    let wd = parsed["runs"][0]["invocations"][0]["workingDirectory"]["uri"]
        .as_str()
        .unwrap_or("");
    let relative = uri == "fused.txt" || uri == "./fused.txt";
    let file_uri = uri.starts_with("file:");
    assert!(
        relative || (file_uri && wd.starts_with("file:")),
        "SARIF uri must be repo-relative or file: with workingDirectory, got uri={uri:?} wd={wd:?} path={path}"
    );
    if !relative {
        assert!(
            wd.starts_with("file:"),
            "file: uri requires invocations[0].workingDirectory.uri, wd={wd:?}"
        );
    }
}
