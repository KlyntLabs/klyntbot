use crate::coding::review_handler::{ReviewIssue, ReviewResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewLlmOutput {
    pub summary: String,
    pub issues: Vec<ReviewLlmIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewLlmIssue {
    pub severity: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub description: String,
    pub suggestion: Option<String>,
}

impl From<ReviewLlmIssue> for ReviewIssue {
    fn from(v: ReviewLlmIssue) -> Self {
        ReviewIssue {
            severity: v.severity,
            file: v.file,
            line: v.line,
            description: v.description,
            suggestion: v.suggestion,
        }
    }
}

/// Parse LLM output, tolerating markdown fences and leading/trailing prose.
pub fn parse_review_output(raw: &str) -> common::Result<ReviewLlmOutput> {
    let trimmed = raw.trim();
    let stripped = strip_markdown_fence(trimmed);

    serde_json::from_str::<ReviewLlmOutput>(stripped).map_err(|e| {
        common::KlyntbotError::Storage(format!("review parse: {e}; raw: {trimmed:.200}"))
    })
}

fn strip_markdown_fence(s: &str) -> &str {
    let lines = s.lines().collect::<Vec<_>>();
    if lines.len() >= 2 && lines[0].starts_with("```") {
        let last = lines.len() - 1;
        if lines[last].starts_with("```") {
            let body_start = s.find('\n').unwrap_or(0) + 1;
            let body_end = s.rfind("\n```").unwrap_or(s.len());
            return &s[body_start..body_end];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_json() {
        let s = r#"{"summary":"ok","issues":[]}"#;
        let p = parse_review_output(s).unwrap();
        assert_eq!(p.summary, "ok");
        assert!(p.issues.is_empty());
    }

    #[test]
    fn parses_with_markdown_fence() {
        let s = "```json\n{\"summary\":\"ok\",\"issues\":[]}\n```";
        let p = parse_review_output(s).unwrap();
        assert_eq!(p.summary, "ok");
    }

    #[test]
    fn parses_full_issue() {
        let s = r#"{
            "summary":"found 1 bug",
            "issues":[{
              "severity":"error",
              "file":"src/lib.rs",
              "line":42,
              "description":"null deref",
              "suggestion":"add Option check"
            }]
        }"#;
        let p = parse_review_output(s).unwrap();
        assert_eq!(p.issues[0].severity, "error");
        assert_eq!(p.issues[0].line, Some(42));
    }

    #[test]
    fn rejects_invalid() {
        let s = "not json at all";
        assert!(parse_review_output(s).is_err());
    }
}
