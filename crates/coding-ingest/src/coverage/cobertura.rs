//! Cobertura XML format parser.
//!
//! Parses `coverage.xml` files. Uses the `line-rate` attribute on the root
//! `<coverage>` element.

use super::CoverageDelta;
use std::path::Path;

/// Parse a cobertura XML file from disk.
pub fn parse_file(path: &Path) -> Option<CoverageDelta> {
    let content = std::fs::read_to_string(path).ok()?;
    parse(&content)
}

/// Parse cobertura XML content from a string.
pub fn parse(content: &str) -> Option<CoverageDelta> {
    // Use simple string parsing to avoid adding quick-xml as a dependency.
    // Look for line-rate="..." on the <coverage> element.
    let line_rate = extract_line_rate(content)?;

    // Synthesize total/covered from the rate (use 10000 as base for precision).
    let total = 10000u32;
    let covered = (line_rate * 10000.0).round() as u32;

    Some(CoverageDelta {
        lines_total: total,
        lines_covered: covered,
        per_file: std::collections::HashMap::new(),
    })
}

fn extract_line_rate(content: &str) -> Option<f64> {
    // Find line-rate="..." in the <coverage> tag
    let marker = "line-rate=\"";
    let start = content.find(marker)? + marker.len();
    let end = content[start..].find('"')? + start;
    content[start..end].parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cobertura_parses_line_rate() {
        let xml = r#"<coverage line-rate="0.75"><packages><package><classes><class filename="src/a.rs" line-rate="0.75"/></classes></package></packages></coverage>"#;
        let d = parse(xml).unwrap();
        assert!((d.percent() - 75.0).abs() < 0.01);
    }

    #[test]
    fn cobertura_full_coverage() {
        let xml = r#"<coverage line-rate="1.0"></coverage>"#;
        let d = parse(xml).unwrap();
        assert!((d.percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn cobertura_no_coverage() {
        let xml = r#"<coverage line-rate="0.0"></coverage>"#;
        let d = parse(xml).unwrap();
        assert!((d.percent()).abs() < 0.01);
    }

    #[test]
    fn cobertura_invalid_xml_returns_none() {
        assert!(parse("not xml").is_none());
    }
}
