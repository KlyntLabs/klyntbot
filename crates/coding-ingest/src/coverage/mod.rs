//! Per-framework coverage parsers.
//!
//! Detects and parses coverage output from lcov, cobertura, tarpaulin,
//! and cargo-llvm-cov.

pub mod cobertura;
pub mod lcov;
pub mod llvm_cov;
pub mod tarpaulin;

pub use common::coverage::{CoverageDelta, FileCoverage};

use std::path::Path;

/// Detect and parse coverage output from the working directory.
///
/// Probes for known coverage file formats in priority order:
/// 1. `lcov.info` / `coverage.lcov` → lcov
/// 2. `coverage.xml` → cobertura
/// 3. `target/tarpaulin/tarpaulin-report.json` → tarpaulin
/// 4. `target/llvm-cov/json/coverage.json` → cargo-llvm-cov
pub fn detect_and_parse(working_dir: &Path) -> Option<CoverageDelta> {
    let candidates: &[(&str, fn(&Path) -> Option<CoverageDelta>)] = &[
        ("lcov.info", lcov::parse_file),
        ("coverage.lcov", lcov::parse_file),
        ("coverage.xml", cobertura::parse_file),
        (
            "target/tarpaulin/tarpaulin-report.json",
            tarpaulin::parse_file,
        ),
        ("target/llvm-cov/json/coverage.json", llvm_cov::parse_file),
    ];

    for (rel, parser) in candidates {
        if let Some(delta) = parser(&working_dir.join(rel)) {
            return Some(delta);
        }
    }
    None
}
