//! Due-date suggestion based on priority and task keywords.

use chrono::{Duration, Utc};
use tools::enrichment::EnrichmentSuggestion;
use tools::todo_types::Todo;

/// Suggest a due date based on task content and inferred urgency.
pub fn suggest_due_date(
    task: &Todo,
) -> Option<EnrichmentSuggestion<chrono::DateTime<chrono::Utc>>> {
    let text = format!(
        "{} {}",
        task.title,
        task.description.as_deref().unwrap_or_default()
    )
    .to_lowercase();

    let now = Utc::now();

    // Urgent tasks: due today
    if contains_urgency(&text) {
        return Some(EnrichmentSuggestion {
            value: now + Duration::hours(8),
            confidence: 0.75,
            reasoning: "Urgent keywords detected; suggesting end of day".to_string(),
        });
    }

    // Tasks with "today" or "tonight"
    if text.contains("today") || text.contains("tonight") {
        return Some(EnrichmentSuggestion {
            value: now + Duration::hours(8),
            confidence: 0.85,
            reasoning: "Task mentions 'today'/'tonight'".to_string(),
        });
    }

    // Tasks with "tomorrow"
    if text.contains("tomorrow") {
        return Some(EnrichmentSuggestion {
            value: now + Duration::days(1),
            confidence: 0.85,
            reasoning: "Task mentions 'tomorrow'".to_string(),
        });
    }

    // Tasks with "this week" or "end of week"
    if text.contains("this week") || text.contains("end of week") || text.contains("eow") {
        return Some(EnrichmentSuggestion {
            value: now + Duration::days(5),
            confidence: 0.75,
            reasoning: "Task mentions 'this week'".to_string(),
        });
    }

    // Tasks with "next week"
    if text.contains("next week") {
        return Some(EnrichmentSuggestion {
            value: now + Duration::days(7),
            confidence: 0.75,
            reasoning: "Task mentions 'next week'".to_string(),
        });
    }

    // No time keywords — use priority as a heuristic
    if let Some(priority) = task.priority {
        let days = match priority {
            1 => 1,
            2 => 3,
            3 => 7,
            _ => 14,
        };
        return Some(EnrichmentSuggestion {
            value: now + Duration::days(days),
            confidence: 0.50,
            reasoning: format!(
                "Based on priority {} — suggesting {} day(s)",
                priority, days
            ),
        });
    }

    // No signal at all — don't suggest
    None
}

fn contains_urgency(text: &str) -> bool {
    const URGENCY_KEYWORDS: &[&str] = &[
        "urgent",
        "asap",
        "critical",
        "blocker",
        "emergency",
        "hotfix",
        "p0",
    ];
    URGENCY_KEYWORDS.iter().any(|kw| text.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tools::todo_types::Todo;

    #[test]
    fn test_urgent_task_due_today() {
        let mut task = Todo::default_instance();
        task.title = "Fix urgent production issue".to_string();

        let result = suggest_due_date(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        // Should be within ~24 hours
        let diff = s.value - Utc::now();
        assert!(diff.num_hours() <= 24);
    }

    #[test]
    fn test_tomorrow_keyword() {
        let mut task = Todo::default_instance();
        task.title = "Submit report tomorrow".to_string();

        let result = suggest_due_date(&task).unwrap();
        let diff = result.value - Utc::now();
        assert!(diff.num_hours() >= 20 && diff.num_hours() <= 28);
        assert!(result.confidence >= 0.80);
    }

    #[test]
    fn test_this_week_keyword() {
        let mut task = Todo::default_instance();
        task.title = "Finish this week".to_string();

        let result = suggest_due_date(&task).unwrap();
        let diff = result.value - Utc::now();
        assert!(diff.num_days() >= 4 && diff.num_days() <= 6);
    }

    #[test]
    fn test_priority_fallback() {
        let mut task = Todo::default_instance();
        task.title = "Something generic".to_string();
        task.priority = Some(1);

        let result = suggest_due_date(&task).unwrap();
        let diff = result.value - Utc::now();
        assert!(diff.num_days() <= 2);
    }

    #[test]
    fn test_no_signal_returns_none() {
        let mut task = Todo::default_instance();
        task.title = "Something generic".to_string();
        // No priority, no keywords

        let result = suggest_due_date(&task);
        assert!(result.is_none());
    }
}
