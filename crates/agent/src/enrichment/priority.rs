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
    fn test_urgent_keyword() {
        let mut task = Todo::default_instance();
        task.title = "Fix urgent bug in login".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 1);
        assert!(s.confidence >= 0.85);
    }

    #[test]
    fn test_bug_keyword() {
        let mut task = Todo::default_instance();
        task.title = "Fix broken auth flow".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 2);
    }

    #[test]
    fn test_feature_keyword() {
        let mut task = Todo::default_instance();
        task.title = "Add feature: dark mode".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 3);
        assert!(s.confidence >= 0.70);
    }

    #[test]
    fn test_low_priority_keyword() {
        let mut task = Todo::default_instance();
        task.title = "Correct typo in readme".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 4);
    }

    #[test]
    fn test_no_keywords_defaults_medium() {
        let mut task = Todo::default_instance();
        task.title = "Implement user dashboard".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 3);
        assert!(s.confidence <= 0.55);
    }

    #[test]
    fn test_description_keywords_counted() {
        let mut task = Todo::default_instance();
        task.title = "Handle edge case".to_string();
        task.description = Some("This is a critical security issue".to_string());

        let result = infer_priority(&task);
        assert!(result.is_some());
        // "critical" is high-priority, "security" is medium-high
        // "critical" should match first
        assert_eq!(result.unwrap().value, 1);
    }

    #[test]
    fn test_tag_keywords_counted() {
        let mut task = Todo::default_instance();
        task.title = "Some task".to_string();
        task.tags = vec!["bug".to_string()];

        let result = infer_priority(&task);
        assert!(result.is_some());
        assert_eq!(result.unwrap().value, 2);
    }

    // ========================================================================
    // Edge case tests (added by QA)
    // ========================================================================

    #[test]
    fn test_conflicting_keywords_picks_highest_confidence() {
        // "Fix typo" has both "fix" (MEDIUM_HIGH) and "typo" (LOW)
        // LOW_KEYWORDS have higher confidence (0.87) than MEDIUM_HIGH (0.82)
        // So should pick "typo" → priority 4
        let mut task = Todo::default_instance();
        task.title = "Fix typo in header".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(
            s.value, 4,
            "Should pick 'typo' (P4, conf 0.87) over 'fix' (P2, conf 0.82)"
        );
        assert!(s.confidence >= 0.85, "Should have high confidence");
    }

    #[test]
    fn test_multiword_keyword_nice_to_have() {
        let mut task = Todo::default_instance();
        task.title = "This would be nice to have eventually".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 4, "Should detect 'nice to have' as low priority");
    }

    #[test]
    fn test_multiword_keyword_low_priority() {
        let mut task = Todo::default_instance();
        task.title = "Low priority task for later".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 4, "Should detect 'low priority' as P4");
    }

    #[test]
    fn test_empty_title_defaults_medium() {
        let mut task = Todo::default_instance();
        task.title = "".to_string();
        task.description = None;

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 3, "Empty title should default to medium priority");
        assert!(
            s.confidence <= 0.55,
            "Empty title should have low confidence"
        );
    }

    #[test]
    fn test_case_insensitive_matching() {
        let mut task = Todo::default_instance();
        task.title = "URGENT FIX REQUIRED".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(
            s.value, 1,
            "Should match 'URGENT' case-insensitively to high priority"
        );
    }

    #[test]
    fn test_tags_only_matching_no_title_keywords() {
        let mut task = Todo::default_instance();
        task.title = "Do something".to_string(); // No keywords
        task.description = None;
        task.tags = vec!["urgent".to_string(), "production".to_string()];

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 1, "Should detect 'urgent' in tags");
        assert!(s.confidence >= 0.85);
    }

    #[test]
    fn test_multiple_conflicting_keywords_picks_best() {
        // Has "urgent" (P1, 0.90), "bug" (P2, 0.82), "minor" (P4, 0.87)
        // Should pick "urgent" (highest confidence)
        let mut task = Todo::default_instance();
        task.title = "URGENT minor bug fix".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(
            s.value, 1,
            "Should pick 'urgent' as it has highest confidence (0.90)"
        );
    }

    #[test]
    fn test_whitespace_only_title() {
        let mut task = Todo::default_instance();
        task.title = "   \t\n  ".to_string();

        let result = infer_priority(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        assert_eq!(s.value, 3, "Whitespace-only should default to medium");
    }
}
