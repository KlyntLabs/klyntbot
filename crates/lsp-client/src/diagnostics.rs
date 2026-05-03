//! LSP diagnostic types and delta diffing.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A diagnostic reported by an LSP server.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LspDiagnostic {
    /// Diagnostic code (e.g. "E0001", "unused import").
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// 1-indexed line number.
    pub line: u32,
    /// Severity level.
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl LspDiagnostic {
    pub fn error(code: impl Into<String>, msg: impl Into<String>, line: u32) -> Self {
        Self {
            code: code.into(),
            message: msg.into(),
            line,
            severity: Severity::Error,
        }
    }

    pub fn warning(code: impl Into<String>, msg: impl Into<String>, line: u32) -> Self {
        Self {
            code: code.into(),
            message: msg.into(),
            line,
            severity: Severity::Warning,
        }
    }
}

/// Diff between two sets of diagnostics.
#[derive(Debug, Default)]
pub struct DiagnosticDiff {
    /// Diagnostics present in `after` but not `before`.
    pub introduced: Vec<LspDiagnostic>,
    /// Diagnostics present in `before` but not `after`.
    pub resolved: Vec<LspDiagnostic>,
}

/// Compute the diff between two diagnostic snapshots.
pub fn diff(before: &[LspDiagnostic], after: &[LspDiagnostic]) -> DiagnosticDiff {
    let bset: HashSet<&LspDiagnostic> = before.iter().collect();
    let aset: HashSet<&LspDiagnostic> = after.iter().collect();
    DiagnosticDiff {
        introduced: aset.difference(&bset).map(|d| (*d).clone()).collect(),
        resolved: bset.difference(&aset).map(|d| (*d).clone()).collect(),
    }
}

/// Convert an `lsp_types::Diagnostic` to our `LspDiagnostic`.
impl From<&lsp_types::Diagnostic> for LspDiagnostic {
    fn from(d: &lsp_types::Diagnostic) -> Self {
        let code = match &d.code {
            Some(lsp_types::NumberOrString::Number(n)) => n.to_string(),
            Some(lsp_types::NumberOrString::String(s)) => s.clone(),
            None => String::new(),
        };
        let severity = match d.severity {
            Some(lsp_types::DiagnosticSeverity::ERROR) => Severity::Error,
            Some(lsp_types::DiagnosticSeverity::WARNING) => Severity::Warning,
            Some(lsp_types::DiagnosticSeverity::INFORMATION) => Severity::Info,
            Some(lsp_types::DiagnosticSeverity::HINT) => Severity::Hint,
            _ => Severity::Error,
        };
        Self {
            code,
            message: d.message.clone(),
            line: d.range.start.line + 1, // LSP is 0-indexed, we're 1-indexed
            severity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_introduced_and_resolved() {
        let before = vec![LspDiagnostic::error("E0001", "unused import", 3)];
        let after = vec![LspDiagnostic::error("E0277", "trait bound", 7)];
        let d = diff(&before, &after);
        assert_eq!(d.introduced.len(), 1);
        assert_eq!(d.resolved.len(), 1);
        assert_eq!(d.introduced[0].code, "E0277");
        assert_eq!(d.resolved[0].code, "E0001");
    }

    #[test]
    fn diff_empty_before() {
        let after = vec![
            LspDiagnostic::error("E0001", "err", 1),
            LspDiagnostic::warning("W0001", "warn", 2),
        ];
        let d = diff(&[], &after);
        assert_eq!(d.introduced.len(), 2);
        assert!(d.resolved.is_empty());
    }

    #[test]
    fn diff_empty_after_resolves_all() {
        let before = vec![LspDiagnostic::error("E0001", "err", 1)];
        let d = diff(&before, &[]);
        assert!(d.introduced.is_empty());
        assert_eq!(d.resolved.len(), 1);
    }

    #[test]
    fn diff_no_change() {
        let diags = vec![LspDiagnostic::error("E0001", "err", 1)];
        let d = diff(&diags, &diags);
        assert!(d.introduced.is_empty());
        assert!(d.resolved.is_empty());
    }

    #[test]
    fn lsp_diagnostic_serde_roundtrip() {
        let diag = LspDiagnostic::error("E0277", "trait bound not satisfied", 42);
        let json = serde_json::to_string(&diag).unwrap();
        let back: LspDiagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(diag, back);
    }
}
