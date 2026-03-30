use serde_json::{Value, json};

/// Summary of a file that needs reformatting, used by `--check` output modes.
pub struct CheckResult {
    pub file: String,
    pub original_lines: usize,
    pub formatted_lines: usize,
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
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
}

/// Output check results in SARIF v2.1.0 format for GitHub Code Scanning.
pub fn output_sarif(results: &[CheckResult]) {
    let sarif_results: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
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
            })
        })
        .collect();

    let sarif = json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "snapper",
                    "informationUri": "https://snapper.turtletech.us",
                    "rules": [{
                        "id": "snapper/needs-reformat",
                        "shortDescription": {
                            "text": "File needs semantic line break formatting"
                        }
                    }]
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
