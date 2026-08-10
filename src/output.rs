use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::check::{DiagnosticKind, LineDiagnostic};

/// SARIF artifact URI: repo-relative when `path` is under `cwd`, else `file:`.
pub fn sarif_artifact_uri(path: &str, cwd: Option<&Path>) -> String {
    if path == "<stdin>" || path == "stdin" {
        return "stdin".to_string();
    }
    let given = Path::new(path);
    if let Some(cwd) = cwd {
        if let Ok(rel) = given.strip_prefix(cwd) {
            return rel.to_string_lossy().replace('\\', "/");
        }
        if given.is_relative() {
            return given.to_string_lossy().replace('\\', "/");
        }
        if let (Ok(abs), Ok(cwd_abs)) = (given.canonicalize(), cwd.canonicalize()) {
            if let Ok(rel) = abs.strip_prefix(cwd_abs) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }
    }
    if given.is_relative() {
        return given.to_string_lossy().replace('\\', "/");
    }
    let abs = given.canonicalize().unwrap_or_else(|_| PathBuf::from(path));
    format!("file://{}", abs.display())
}

fn file_uri(path: &Path) -> String {
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    format!("file://{}", abs.display())
}

/// Summary of a file inspected by `--check`, including line diagnostics.
pub struct CheckResult {
    pub file: String,
    pub original_lines: usize,
    pub formatted_lines: usize,
    pub would_reformat: bool,
    pub diagnostics: Vec<LineDiagnostic>,
}

/// Output check results in JSON format.
pub fn output_json(results: &[CheckResult]) {
    let arr: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "file": r.file,
                "original_lines": r.original_lines,
                "formatted_lines": r.formatted_lines,
                "would_reformat": r.would_reformat,
                "diagnostics": r.diagnostics,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
}

/// Output check results in SARIF v2.1.0 format for GitHub Code Scanning.
pub fn output_sarif(results: &[CheckResult]) {
    let cwd = std::env::current_dir().ok();
    let cwd_ref = cwd.as_deref();
    let mut sarif_results: Vec<Value> = Vec::new();
    for r in results {
        let uri = sarif_artifact_uri(&r.file, cwd_ref);
        if r.would_reformat {
            sarif_results.push(json!({
                "ruleId": "snapper/needs-reformat",
                "level": "warning",
                "message": {
                    "text": format!(
                        "File needs semantic line break formatting ({} -> {} lines)",
                        r.original_lines, r.formatted_lines
                    )
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": uri.clone()
                        }
                    }
                }]
            }));
        }
        for d in &r.diagnostics {
            let (rule_id, level) = match d.kind {
                DiagnosticKind::Fused => ("snapper/fused", "warning"),
                DiagnosticKind::Wrap => ("snapper/wrap", "warning"),
                DiagnosticKind::Long => ("snapper/long", "note"),
            };
            sarif_results.push(json!({
                "ruleId": rule_id,
                "level": level,
                "message": {
                    "text": format!("{}: {}", d.kind.as_str(), d.excerpt)
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": uri.clone()
                        },
                        "region": {
                            "startLine": d.line,
                            "snippet": {
                                "text": d.excerpt
                            }
                        }
                    }
                }]
            }));
        }
    }

    let working_directory = cwd.as_ref().map(|p| json!({ "uri": file_uri(p) }));

    let sarif = json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "snapper",
                    "informationUri": "https://snapper.turtletech.us",
                    "rules": [
                        {
                            "id": "snapper/needs-reformat",
                            "shortDescription": {
                                "text": "File needs semantic line break formatting"
                            }
                        },
                        {
                            "id": "snapper/fused",
                            "shortDescription": {
                                "text": "Prose line contains more than one sentence"
                            }
                        },
                        {
                            "id": "snapper/wrap",
                            "shortDescription": {
                                "text": "Prose line continues a clause from the previous line"
                            }
                        },
                        {
                            "id": "snapper/long",
                            "shortDescription": {
                                "text": "Prose line exceeds the width threshold at a clause boundary"
                            }
                        }
                    ]
                }
            },
            "invocations": [{
                "executionSuccessful": true,
                "workingDirectory": working_directory
            }],
            "results": sarif_results
        }]
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&sarif).unwrap_or_default()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn relative_path_stays_relative() {
        assert_eq!(
            sarif_artifact_uri("fused.txt", Some(Path::new("/tmp/work"))),
            "fused.txt"
        );
    }

    #[test]
    fn stdin_uri_is_literal() {
        assert_eq!(sarif_artifact_uri("<stdin>", None), "stdin");
    }
}
