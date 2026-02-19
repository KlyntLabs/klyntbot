//! Duration prediction from task title, tags, and complexity heuristics.

use tools::enrichment::EnrichmentSuggestion;
use tools::todo_types::Todo;

/// Keyword-to-duration mappings (minutes).
const QUICK_KEYWORDS: &[&str] = &["typo", "rename", "tweak", "bump", "toggle", "minor"];
const SMALL_KEYWORDS: &[&str] = &["fix", "patch", "update", "adjust", "lint", "format"];
const MEDIUM_KEYWORDS: &[&str] = &[
    "feature",
    "implement",
    "add",
    "create",
    "build",
    "design",
    "migrate",
];
const LARGE_KEYWORDS: &[&str] = &[
    "refactor",
    "overhaul",
    "rewrite",
    "redesign",
    "architecture",
    "system",
    "integration",
];

/// Predict duration from task content.
pub fn predict_duration(task: &Todo) -> Option<EnrichmentSuggestion<u32>> {
    let text = format!(
        "{} {}",
        task.title,
        task.description.as_deref().unwrap_or_default()
    )
    .to_lowercase();

    if contains_any(&text, QUICK_KEYWORDS) {
        return Some(EnrichmentSuggestion {
            value: 15,
            confidence: 0.80,
            reasoning: "Quick task based on keywords (typo, rename, etc.)".to_string(),
        });
    }

    if contains_any(&text, SMALL_KEYWORDS) {
        return Some(EnrichmentSuggestion {
            value: 30,
            confidence: 0.75,
            reasoning: "Small task based on keywords (fix, patch, etc.)".to_string(),
        });
    }

    if contains_any(&text, LARGE_KEYWORDS) {
        return Some(EnrichmentSuggestion {
            value: 120,
            confidence: 0.65,
            reasoning: "Large task based on keywords (refactor, rewrite, etc.)".to_string(),
        });
    }

    if contains_any(&text, MEDIUM_KEYWORDS) {
        return Some(EnrichmentSuggestion {
            value: 60,
            confidence: 0.70,
            reasoning: "Medium task based on keywords (feature, implement, etc.)".to_string(),
        });
    }

    // Default estimate
    Some(EnrichmentSuggestion {
        value: 45,
        confidence: 0.45,
        reasoning: "No duration keywords found; defaulting to 45 minutes".to_string(),
    })
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools::todo_types::Todo;

    #[test]
    fn test_duration_prediction_from_keywords() {
        let cases = [
            ("Fix typo in header", 15),             // quick (typo)
            ("Fix login validation", 30),            // small (fix, but no quick keyword)
            ("Implement user notifications", 60),    // medium (implement)
            ("Refactor authentication system", 120), // large (refactor)
            ("Review PR", 45),                       // default (no keywords)
        ];
        for (title, expected_minutes) in cases {
            let mut task = Todo::default_instance();
            task.title = title.to_string();
            let result = predict_duration(&task).expect("should always return Some");
            assert_eq!(result.value, expected_minutes, "failed for: {title}");
        }
    }
}
