//! Priority inference from task title, description, and tags.

use tools::enrichment::EnrichmentSuggestion;
use tools::todo_types::Todo;

/// Keyword groups mapped to priority levels.
/// Priority scale: 1 = highest, 4 = lowest.
const HIGH_PRIORITY_KEYWORDS: &[&str] = &[
    "urgent",
    "critical",
    "blocker",
    "hotfix",
    "emergency",
    "asap",
    "p0",
    "sev1",
    "production",
    "outage",
];

const MEDIUM_HIGH_KEYWORDS: &[&str] = &[
    "important",
    "bug",
    "fix",
    "broken",
    "regression",
    "p1",
    "sev2",
    "security",
];

const MEDIUM_KEYWORDS: &[&str] = &[
    "feature",
    "enhance",
    "improvement",
    "update",
    "p2",
    "refactor",
];

const LOW_KEYWORDS: &[&str] = &[
    "nice to have",
    "low priority",
    "cleanup",
    "chore",
    "documentation",
    "docs",
    "typo",
    "minor",
    "p3",
    "p4",
];

/// Infer priority from task content using keyword matching.
/// When multiple keywords match, use the most specific (highest confidence) one.
pub fn infer_priority(task: &Todo) -> Option<EnrichmentSuggestion<u8>> {
    let text = build_searchable_text(task);
    let lower = text.to_lowercase();

    // Check all keyword groups and collect matches with their confidence scores
    // Note: LOW_KEYWORDS get higher confidence (0.87) than MEDIUM_HIGH (0.82)
    // because they're more specific (e.g., "typo" is more specific than "fix")
    let mut matches = Vec::new();

    if let Some(kw) = find_keyword(&lower, HIGH_PRIORITY_KEYWORDS) {
        matches.push((1, 0.90, format!("Contains high-priority keyword: '{}'", kw)));
    }

    if let Some(kw) = find_keyword(&lower, MEDIUM_HIGH_KEYWORDS) {
        matches.push((
            2,
            0.82,
            format!("Contains medium-high priority keyword: '{}'", kw),
        ));
    }

    if let Some(kw) = find_keyword(&lower, MEDIUM_KEYWORDS) {
        matches.push((
            3,
            0.75,
            format!("Contains medium priority keyword: '{}'", kw),
        ));
    }

    if let Some(kw) = find_keyword(&lower, LOW_KEYWORDS) {
        matches.push((4, 0.87, format!("Contains low-priority keyword: '{}'", kw)));
    }

    // Return the match with highest confidence score
    if let Some((value, confidence, reasoning)) = matches
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    {
        return Some(EnrichmentSuggestion {
            value,
            confidence,
            reasoning,
        });
    }

    // Default: medium priority with low confidence
    Some(EnrichmentSuggestion {
        value: 3,
        confidence: 0.50,
        reasoning: "No priority keywords found; defaulting to medium".to_string(),
    })
}

fn build_searchable_text(task: &Todo) -> String {
    let mut text = task.title.clone();
    if let Some(ref desc) = task.description {
        text.push(' ');
        text.push_str(desc);
    }
    for tag in &task.tags {
        text.push(' ');
        text.push_str(tag);
    }
    text
}

fn find_keyword<'a>(text: &str, keywords: &[&'a str]) -> Option<&'a str> {
    keywords.iter().find(|kw| text.contains(**kw)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools::todo_types::Todo;

    #[test]
    fn test_priority_inference_from_keywords() {
        let cases = [
            ("URGENT: fix production", 1), // high-priority keyword (case-insensitive)
            ("Fix broken auth flow", 2),   // medium-high keyword ("broken")
            ("Add feature: dark mode", 3), // medium keyword ("feature")
            ("Correct typo in readme", 4), // low keyword ("typo")
            ("Implement user dashboard", 3), // no keywords → default medium
            ("nice to have: polish UI", 4), // multi-word low keyword
            ("low priority: cleanup", 4),  // multi-word low keyword
            ("", 3),                       // empty title → default medium
            ("   \t\n  ", 3),              // whitespace-only → default medium
        ];
        for (title, expected_priority) in cases {
            let mut task = Todo::default_instance();
            task.title = title.to_string();
            let result = infer_priority(&task).expect("should always return Some");
            assert_eq!(result.value, expected_priority, "failed for: {title:?}");
        }
    }

    #[test]
    fn test_priority_inference_from_multiple_sources() {
        // Description keywords: "critical" in description → P1
        let mut task = Todo::default_instance();
        task.title = "Handle edge case".to_string();
        task.description = Some("This is a critical security issue".to_string());
        assert_eq!(
            infer_priority(&task).unwrap().value,
            1,
            "failed for: description keyword 'critical'"
        );

        // Tag keywords: "bug" in tags → P2
        let mut task = Todo::default_instance();
        task.title = "Some task".to_string();
        task.tags = vec!["bug".to_string()];
        assert_eq!(
            infer_priority(&task).unwrap().value,
            2,
            "failed for: tag keyword 'bug'"
        );

        // Tags only (no title/description keywords): "urgent" in tags → P1
        let mut task = Todo::default_instance();
        task.title = "Do something".to_string();
        task.tags = vec!["urgent".to_string(), "production".to_string()];
        let result = infer_priority(&task).unwrap();
        assert_eq!(result.value, 1, "failed for: tags-only keyword 'urgent'");
        assert!(
            result.confidence >= 0.85,
            "tags-only urgent should have high confidence"
        );
    }

    #[test]
    fn test_priority_conflict_resolution() {
        // "Fix typo" has "fix" (P2, conf 0.82) and "typo" (P4, conf 0.87)
        // Highest confidence wins → "typo" at P4
        let mut task = Todo::default_instance();
        task.title = "Fix typo in header".to_string();
        let result = infer_priority(&task).unwrap();
        assert_eq!(
            result.value, 4,
            "failed for: 'Fix typo' should pick typo (conf 0.87) over fix (conf 0.82)"
        );
        assert!(result.confidence >= 0.85);

        // "URGENT minor bug fix" has "urgent" (P1, 0.90), "minor" (P4, 0.87), "bug" (P2, 0.82)
        // Highest confidence wins → "urgent" at P1
        let mut task = Todo::default_instance();
        task.title = "URGENT minor bug fix".to_string();
        let result = infer_priority(&task).unwrap();
        assert_eq!(
            result.value, 1,
            "failed for: 'URGENT minor bug fix' should pick urgent (conf 0.90)"
        );
    }
}
