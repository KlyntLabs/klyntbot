//! cargo-llvm-cov JSON report parser.
//!
//! Parses `target/llvm-cov/json/coverage.json`.
//! Schema: `{ "data": [{ "totals": { "lines": { "percent": N, "covered": N, "count": N } },
//!   "files": [{ "filename": "...", "summary": { "lines": { "percent": N, "covered": N, "count": N } } }] }] }`

use super::{CoverageDelta, FileCoverage};
use std::collections::HashMap;
use std::path::Path;

/// Parse a cargo-llvm-cov JSON file from disk.
pub fn parse_file(path: &Path) -> Option<CoverageDelta> {
    let content = std::fs::read_to_string(path).ok()?;
    parse(&content)
}

/// Parse cargo-llvm-cov JSON content from a string.
pub fn parse(content: &str) -> Option<CoverageDelta> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let data = json["data"].as_array()?.first()?;

    let totals = &data["totals"]["lines"];
    let lines_total = totals["count"].as_u64()? as u32;
    let lines_covered = totals["covered"].as_u64()? as u32;

    let mut per_file = HashMap::new();
    if let Some(files) = data["files"].as_array() {
        for file in files {
            let path = file["filename"].as_str().unwrap_or("unknown").to_string();
            let summary = &file["summary"]["lines"];
            let total = summary["count"].as_u64().unwrap_or(0) as u32;
            let covered = summary["covered"].as_u64().unwrap_or(0) as u32;
            per_file.insert(path, FileCoverage { total, covered });
        }
    }

    if lines_total == 0 {
        return None;
    }

    Some(CoverageDelta {
        lines_total,
        lines_covered,
        per_file,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llvm_cov_parses_totals() {
        let json = r#"{
            "data": [{
                "totals": {
                    "lines": {"percent": 82.5, "covered": 825, "count": 1000}
                },
                "files": [
                    {
                        "filename": "src/lib.rs",
                        "summary": {
                            "lines": {"percent": 90.0, "covered": 90, "count": 100}
                        }
                    }
                ]
            }]
        }"#;
        let d = parse(json).unwrap();
        assert_eq!(d.lines_total, 1000);
        assert_eq!(d.lines_covered, 825);
        assert!((d.percent() - 82.5).abs() < 0.01);
        assert_eq!(d.per_file.len(), 1);
    }

    #[test]
    fn llvm_cov_empty_data_returns_none() {
        let json = r#"{"data": []}"#;
        assert!(parse(json).is_none());
    }

    #[test]
    fn llvm_cov_zero_lines_returns_none() {
        let json = r#"{"data": [{"totals": {"lines": {"percent": 0, "covered": 0, "count": 0}}, "files": []}]}"#;
        assert!(parse(json).is_none());
    }
}
