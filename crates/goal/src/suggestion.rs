//! Goal suggestion engine — detects patterns and proposes goals.

use uuid::Uuid;

/// Goal keywords with their confidence weights.
const GOAL_KEYWORDS: &[(&str, f64)] = &[
    ("launch", 0.8),
    ("build", 0.7),
    ("create", 0.6),
    ("improve", 0.7),
    ("learn", 0.8),
    ("master", 0.8),
    ("ship", 0.8),
    ("develop", 0.7),
    ("establish", 0.7),
    ("achieve", 0.9),
    ("complete", 0.6),
    ("migrate", 0.7),
    ("redesign", 0.7),
    ("scale", 0.7),
    ("automate", 0.7),
];

/// A proposed goal suggested by the engine.
#[derive(Debug, Clone)]
pub struct GoalSuggestion {
    pub proposed_title: String,
    pub rationale: String,
    pub linked_items: Vec<Uuid>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

/// Detects patterns in tasks and messages to suggest goals.
pub struct GoalSuggestionEngine {
    /// Suggest goal after N related tasks share a pattern (default: 5)
    pub pattern_threshold: usize,
}

impl Default for GoalSuggestionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GoalSuggestionEngine {
    pub fn new() -> Self {
        Self {
            pattern_threshold: 5,
        }
    }

    pub fn with_threshold(threshold: usize) -> Self {
        Self {
            pattern_threshold: threshold,
        }
    }

    /// Detect goal intent from a user message using keyword matching.
    pub fn detect_goal_intent(&self, message: &str) -> Option<GoalSuggestion> {
        let lower = message.to_lowercase();

        for &(keyword, confidence) in GOAL_KEYWORDS {
            if lower.contains(keyword) {
                // Extract context after the keyword for the title
                let title = self.extract_title(&lower, keyword, message);
                return Some(GoalSuggestion {
                    proposed_title: title,
                    rationale: format!("Detected goal-oriented keyword: \"{keyword}\""),
                    linked_items: vec![],
                    confidence,
                });
            }
        }

        None
    }

    /// Build a suggested title from the message context around the keyword.
    ///
    /// Uses char-boundary-safe indexing to avoid panics with multibyte characters.
    fn extract_title(&self, _lower: &str, keyword: &str, original: &str) -> String {
        // Search case-insensitively in the original string to avoid byte-offset
        // mismatches between `original` and its lowercased form.
        let original_lower = original.to_lowercase();
        if let Some(pos) = original_lower.find(keyword) {
            // Verify `pos` is a valid char boundary in `original` (it should be,
            // since both strings have the same char structure for ASCII keywords).
            if original.is_char_boundary(pos) {
                let from_keyword = &original[pos..];
                let mut chars = from_keyword.chars();
                match chars.next() {
                    Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                    None => original.to_string(),
                }
            } else {
                original.to_string()
            }
        } else {
            original.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggestion_engine_creation() {
        let engine = GoalSuggestionEngine::new();
        assert_eq!(engine.pattern_threshold, 5);

        let custom = GoalSuggestionEngine::with_threshold(3);
        assert_eq!(custom.pattern_threshold, 3);
    }

    #[test]
    fn test_detect_goal_intent_launch() {
        let engine = GoalSuggestionEngine::new();

        let result = engine.detect_goal_intent("I want to launch a new product");
        assert!(result.is_some());
        let suggestion = result.unwrap();
        assert!(!suggestion.proposed_title.is_empty());
        assert!(suggestion.confidence > 0.0);
        assert!(suggestion.confidence <= 1.0);
    }

    #[test]
    fn test_detect_goal_intent_learn() {
        let engine = GoalSuggestionEngine::new();

        let result = engine.detect_goal_intent("I need to learn Rust programming");
        assert!(result.is_some());
        let suggestion = result.unwrap();
        assert!(
            suggestion.proposed_title.contains("Learn")
                || suggestion.proposed_title.contains("learn")
        );
        assert!(suggestion.confidence > 0.0);
    }

    #[test]
    fn test_detect_goal_intent_build() {
        let engine = GoalSuggestionEngine::new();

        let result = engine.detect_goal_intent("Let's build an API gateway");
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_goal_intent_improve() {
        let engine = GoalSuggestionEngine::new();

        let result = engine.detect_goal_intent("We should improve our test coverage");
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_goal_intent_no_match() {
        let engine = GoalSuggestionEngine::new();

        let result = engine.detect_goal_intent("add a print statement here");
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_goal_intent_case_insensitive() {
        let engine = GoalSuggestionEngine::new();

        let result = engine.detect_goal_intent("LAUNCH the new feature");
        assert!(result.is_some());
    }

    #[test]
    fn test_suggestion_has_empty_linked_items() {
        let engine = GoalSuggestionEngine::new();

        let result = engine.detect_goal_intent("I want to build a CLI tool");
        assert!(result.is_some());
        assert!(result.unwrap().linked_items.is_empty());
    }
}
