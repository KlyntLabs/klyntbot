use crate::coding::review_handler::ReviewIssue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewLlmOutput {
    pub summary: String,
    pub issues: Vec<ReviewIssue>,
}

/// Parse LLM output, tolerating markdown fences.
pub fn parse_review_output(raw: &str) -> common::Result<ReviewLlmOutput> {
    let stripped = common::helpers::strip_llm_fences(raw);

    serde_json::from_str::<ReviewLlmOutput>(stripped)
        .map_err(|e| common::KlyntbotError::Storage(format!("review parse: {e}; raw: {raw:.200}")))
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
