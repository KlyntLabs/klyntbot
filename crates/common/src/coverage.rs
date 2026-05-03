//! Coverage delta types shared across crates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Coverage information from a test run.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CoverageDelta {
    /// Total lines that could be covered.
    pub lines_total: u32,
    /// Lines that were actually covered.
    pub lines_covered: u32,
    /// Per-file coverage breakdown.
    #[serde(default)]
    pub per_file: HashMap<String, FileCoverage>,
}

/// Coverage for a single file.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FileCoverage {
    pub total: u32,
    pub covered: u32,
}

impl CoverageDelta {
    /// Coverage percentage (0.0–100.0).
    pub fn percent(&self) -> f64 {
        if self.lines_total == 0 {
            0.0
        } else {
            100.0 * self.lines_covered as f64 / self.lines_total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_zero_total() {
        let d = CoverageDelta::default();
        assert_eq!(d.percent(), 0.0);
    }

    #[test]
    fn percent_half() {
        let d = CoverageDelta {
            lines_total: 100,
            lines_covered: 50,
            per_file: HashMap::new(),
        };
        assert!((d.percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn serde_roundtrip() {
        let d = CoverageDelta {
            lines_total: 200,
            lines_covered: 150,
            per_file: HashMap::from([(
                "src/lib.rs".to_string(),
                FileCoverage {
                    total: 100,
                    covered: 75,
                },
            )]),
        };
        let json = serde_json::to_string(&d).unwrap();
        let back: CoverageDelta = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
