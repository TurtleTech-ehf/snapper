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
