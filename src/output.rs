use serde_json::{Value, json};

use crate::check::{DiagnosticKind, LineDiagnostic};

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
    let mut sarif_results: Vec<Value> = Vec::new();
    for r in results {
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
                            "uri": r.file
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
                            "uri": r.file
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
            "results": sarif_results
        }]
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&sarif).unwrap_or_default()
    );
}
