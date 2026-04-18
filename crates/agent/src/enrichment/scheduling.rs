//! Due-date suggestion based on priority and task keywords.

use feature_tasks::{EnrichmentSuggestion, Task};

use super::priority::{build_searchable_text, HIGH_PRIORITY_KEYWORDS};

/// Suggest a due date based on task content and inferred urgency.
pub fn suggest_due_date(task: &Task) -> Option<EnrichmentSuggestion<jiff::Timestamp>> {
    let text = build_searchable_text(task).to_lowercase();

    let now = jiff::Timestamp::now();

    // Urgent tasks: due today
    if contains_urgency(&text) {
        return Some(EnrichmentSuggestion {
            value: now.checked_add(jiff::SignedDuration::from_secs(8 * 3600)).unwrap(),
            confidence: 0.75,
            reasoning: "Urgent keywords detected; suggesting end of day".to_string(),
        });
    }

    // Tasks with "today" or "tonight"
    if text.contains("today") || text.contains("tonight") {
        return Some(EnrichmentSuggestion {
            value: now.checked_add(jiff::SignedDuration::from_secs(8 * 3600)).unwrap(),
            confidence: 0.85,
            reasoning: "Task mentions 'today'/'tonight'".to_string(),
        });
    }

    // Tasks with "tomorrow"
    if text.contains("tomorrow") {
        return Some(EnrichmentSuggestion {
            value: now.checked_add(jiff::SignedDuration::from_secs(86400)).unwrap(),
            confidence: 0.85,
            reasoning: "Task mentions 'tomorrow'".to_string(),
        });
    }

    // Tasks with "this week" or "end of week"
    if text.contains("this week") || text.contains("end of week") || text.contains("eow") {
        return Some(EnrichmentSuggestion {
            value: now.checked_add(jiff::SignedDuration::from_secs(5 * 86400)).unwrap(),
            confidence: 0.75,
            reasoning: "Task mentions 'this week'".to_string(),
        });
    }

    // Tasks with "next week"
    if text.contains("next week") {
        return Some(EnrichmentSuggestion {
            value: now.checked_add(jiff::SignedDuration::from_secs(7 * 86400)).unwrap(),
            confidence: 0.75,
            reasoning: "Task mentions 'next week'".to_string(),
        });
    }

    // No time keywords — use priority as a heuristic
    if let Some(priority) = task.priority {
        let days: i64 = match priority {
            1 => 1,
            2 => 3,
            3 => 7,
            _ => 14,
        };
        return Some(EnrichmentSuggestion {
            value: now.checked_add(jiff::SignedDuration::from_secs(days * 86400)).unwrap(),
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
    HIGH_PRIORITY_KEYWORDS.iter().any(|kw| text.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use feature_tasks::Task;

    #[test]
    fn test_urgent_task_due_today() {
        let mut task = Task::default_instance();
        task.title = "Fix urgent production issue".to_string();

        let result = suggest_due_date(&task);
        assert!(result.is_some());
        let s = result.unwrap();
        // Should be within ~24 hours
        let diff_hours = (s.value.as_millisecond() - jiff::Timestamp::now().as_millisecond()) / 3_600_000;
        assert!(diff_hours <= 24);
    }

    #[test]
    fn test_tomorrow_keyword() {
        let mut task = Task::default_instance();
        task.title = "Submit report tomorrow".to_string();

        let result = suggest_due_date(&task).unwrap();
        let diff_hours = (result.value.as_millisecond() - jiff::Timestamp::now().as_millisecond()) / 3_600_000;
        assert!((20..=28).contains(&diff_hours));
        assert!(result.confidence >= 0.80);
    }

    #[test]
    fn test_this_week_keyword() {
        let mut task = Task::default_instance();
        task.title = "Finish this week".to_string();

        let result = suggest_due_date(&task).unwrap();
        let diff_days = (result.value.as_millisecond() - jiff::Timestamp::now().as_millisecond()) / 86_400_000;
        assert!((4..=6).contains(&diff_days));
    }

    #[test]
    fn test_priority_fallback() {
        let mut task = Task::default_instance();
        task.title = "Something generic".to_string();
        task.priority = Some(1);

        let result = suggest_due_date(&task).unwrap();
        let diff_days = (result.value.as_millisecond() - jiff::Timestamp::now().as_millisecond()) / 86_400_000;
        assert!(diff_days <= 2);
    }

    #[test]
    fn test_no_signal_returns_none() {
        let mut task = Task::default_instance();
        task.title = "Something generic".to_string();
        // No priority, no keywords

        let result = suggest_due_date(&task);
        assert!(result.is_none());
    }
}
