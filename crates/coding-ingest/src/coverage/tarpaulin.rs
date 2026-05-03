//! cargo-tarpaulin JSON report parser.
//!
//! Parses `target/tarpaulin/tarpaulin-report.json`.
//! Schema: `{ "files": [{ "path": "...", "coverable": N, "covered": N }], "coverage": pct }`

use super::{CoverageDelta, FileCoverage};
use std::collections::HashMap;
use std::path::Path;

/// Parse a tarpaulin report file from disk.
pub fn parse_file(path: &Path) -> Option<CoverageDelta> {
    let content = std::fs::read_to_string(path).ok()?;
    parse(&content)
}

/// Parse tarpaulin JSON content from a string.
pub fn parse(content: &str) -> Option<CoverageDelta> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;

    let mut per_file = HashMap::new();
    let mut lines_total: u32 = 0;
    let mut lines_covered: u32 = 0;

    if let Some(files) = json["files"].as_array() {
        for file in files {
            let path = file["path"].as_str().unwrap_or("unknown").to_string();
            let total = file["coverable"].as_u64().unwrap_or(0) as u32;
            let covered = file["covered"].as_u64().unwrap_or(0) as u32;
            lines_total += total;
            lines_covered += covered;
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
    fn tarpaulin_parses_files() {
        let json = r#"{
            "files": [
                {"path": "src/lib.rs", "coverable": 100, "covered": 80},
                {"path": "src/main.rs", "coverable": 50, "covered": 25}
            ],
            "coverage": 0.7
        }"#;
        let d = parse(json).unwrap();
        assert_eq!(d.lines_total, 150);
        assert_eq!(d.lines_covered, 105);
        assert!((d.percent() - 70.0).abs() < 0.01);
        assert_eq!(d.per_file.len(), 2);
    }

    #[test]
    fn tarpaulin_empty_files_returns_none() {
        let json = r#"{"files": [], "coverage": 0}"#;
        assert!(parse(json).is_none());
    }

    #[test]
    fn tarpaulin_invalid_json_returns_none() {
        assert!(parse("not json").is_none());
    }
}
