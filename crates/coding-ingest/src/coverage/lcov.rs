//! lcov format parser.
//!
//! Parses `lcov.info` / `coverage.lcov` files. The format uses `DA:line,hit_count`
//! records and `LF:`/`LH:` summary lines.

use super::{CoverageDelta, FileCoverage};
use std::collections::HashMap;
use std::path::Path;

/// Parse an lcov file from disk.
pub fn parse_file(path: &Path) -> Option<CoverageDelta> {
    let content = std::fs::read_to_string(path).ok()?;
    parse(&content)
}

/// Parse lcov content from a string.
pub fn parse(content: &str) -> Option<CoverageDelta> {
    let mut lines_total: u32 = 0;
    let mut lines_covered: u32 = 0;
    let mut per_file: HashMap<String, FileCoverage> = HashMap::new();

    let mut current_file: Option<String> = None;
    let mut file_total: u32 = 0;
    let mut file_covered: u32 = 0;

    for line in content.lines() {
        let line = line.trim();
        if let Some(path) = line.strip_prefix("SF:") {
            current_file = Some(path.to_string());
            file_total = 0;
            file_covered = 0;
        } else if line.starts_with("DA:") {
            // DA:line,hit_count
            if let Some(data) = line.strip_prefix("DA:") {
                let parts: Vec<&str> = data.split(',').collect();
                if parts.len() >= 2 {
                    file_total += 1;
                    let hits: u32 = parts[1].parse().unwrap_or(0);
                    if hits > 0 {
                        file_covered += 1;
                    }
                }
            }
        } else if let Some(val) = line.strip_prefix("LF:") {
            // LF: is the total lines summary — use it if we don't have DA records
            if file_total == 0 {
                file_total = val.parse().unwrap_or(0);
            }
        } else if let Some(val) = line.strip_prefix("LH:") {
            // LH: is the hit lines summary
            if file_covered == 0 {
                file_covered = val.parse().unwrap_or(0);
            }
        } else if line == "end_of_record" {
            if let Some(path) = current_file.take() {
                lines_total += file_total;
                lines_covered += file_covered;
                per_file.insert(
                    path,
                    FileCoverage {
                        total: file_total,
                        covered: file_covered,
                    },
                );
            }
        }
    }

    // Handle case where there's no end_of_record (single-file lcov)
    if let Some(path) = current_file {
        lines_total += file_total;
        lines_covered += file_covered;
        per_file.insert(
            path,
            FileCoverage {
                total: file_total,
                covered: file_covered,
            },
        );
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
    fn lcov_parses_total_percentage() {
        let sample = "TN:\nSF:src/lib.rs\nDA:1,1\nDA:2,0\nLF:2\nLH:1\nend_of_record\n";
        let delta = parse(sample).unwrap();
        assert_eq!(delta.lines_total, 2);
        assert_eq!(delta.lines_covered, 1);
        assert!((delta.percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn lcov_parses_multiple_files() {
        let sample = "\
TN:
SF:src/lib.rs
DA:1,1
DA:2,1
end_of_record
SF:src/main.rs
DA:1,0
DA:2,1
DA:3,0
end_of_record
";
        let delta = parse(sample).unwrap();
        assert_eq!(delta.lines_total, 5);
        assert_eq!(delta.lines_covered, 3);
        assert_eq!(delta.per_file.len(), 2);
    }

    #[test]
    fn lcov_empty_returns_none() {
        assert!(parse("").is_none());
    }

    #[test]
    fn lcov_uses_lf_lh_when_no_da_records() {
        let sample = "TN:\nSF:src/lib.rs\nLF:100\nLH:75\nend_of_record\n";
        let delta = parse(sample).unwrap();
        assert_eq!(delta.lines_total, 100);
        assert_eq!(delta.lines_covered, 75);
    }
}
