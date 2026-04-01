use rand::Rng;

// ── Template arrays ──────────────────────────────────────────────────

pub const TASK_TEMPLATES: &[&str] = &[
    "Create a task: {action} for {project}, due {due_date}",
    "I need to {action} by {due_date}",
    "Add to my tasks: {action}",
    "Mark {task} as done",
    "What's left on {project}?",
    "Show me my tasks for today",
    "Prioritize {action} — it's urgent",
];

pub const NOTE_TEMPLATES: &[&str] = &[
    "Create a note about {topic}: {content}",
    "Update my {topic} note with: {content}",
    "What do my notes say about {topic}?",
    "Summarize my notes on {topic}",
    "Add to my {topic} note: {content}",
];

pub const FINANCE_TEMPLATES: &[&str] = &[
    "Record expense: {amount} for {category} — {description}",
    "How much did I spend on {category} this month?",
    "Show my budget status",
    "Add income: {amount} from {description}",
    "What's my spending trend for {category}?",
];

pub const CHAT_TEMPLATES: &[&str] = &[
    "Good morning",
    "What should I focus on today?",
    "How's my week looking?",
    "Give me a quick summary",
    "Thanks for the help",
];

pub const PRODUCTIVITY_TEMPLATES: &[&str] = &[
    "Start a focus session for {task}",
    "How productive was I this week?",
    "I'm feeling {energy} today",
    "Track my time on {task}",
];

pub const AUTOMATION_TEMPLATES: &[&str] = &[
    "Set up a daily reminder for {action} at {time}",
    "Create a recurring task: {action} every {frequency}",
    "Automate my {action} workflow",
];

pub const INSIGHTS_TEMPLATES: &[&str] = &[
    "Show me patterns across my notes and tasks",
    "What connections do you see in my work?",
    "Any insights from my recent activity?",
];

pub const LEARNING_TEMPLATES: &[&str] = &[
    "I'm learning {topic} — create some flashcards",
    "Quiz me on {topic}",
    "What should I review today?",
    "I just learned that {content}",
];

pub const CORRECTION_TEMPLATES: &[&str] = &[
    "No, I meant {correct_value}, not {wrong_value}",
    "That's wrong — it should be {correct_value}",
    "Actually, I prefer {correct_value}",
    "Not quite — I said {correct_value}",
];

pub const FACT_INTRODUCTION_TEMPLATES: &[&str] = &[
    "By the way, I {predicate} {object}",
    "Just so you know, I'm a {object}",
    "I've been {predicate} {object} lately",
    "Did I mention I {predicate} {object}?",
];

// ── Functions ─────────────────────────────────────────────────────────

/// Pick a random template from the slice.
pub fn pick_template<'a>(templates: &'a [&str], rng: &mut impl Rng) -> &'a str {
    let idx = rng.random_range(0..templates.len());
    templates[idx]
}

/// Return the template array matching the given topic name. Falls back to
/// `CHAT_TEMPLATES` for unknown topics.
pub fn templates_for_topic(topic: &str) -> &'static [&'static str] {
    match topic {
        "tasks" => TASK_TEMPLATES,
        "notes" => NOTE_TEMPLATES,
        "finance" => FINANCE_TEMPLATES,
        "chat" => CHAT_TEMPLATES,
        "productivity" => PRODUCTIVITY_TEMPLATES,
        "automation" => AUTOMATION_TEMPLATES,
        "insights" => INSIGHTS_TEMPLATES,
        "learning" => LEARNING_TEMPLATES,
        "correction" => CORRECTION_TEMPLATES,
        "fact_introduction" => FACT_INTRODUCTION_TEMPLATES,
        _ => CHAT_TEMPLATES,
    }
}

/// Replace `{key}` placeholders in `template` using the provided `vars`.
/// Any remaining `{word}` placeholders not covered by `vars` are replaced
/// with `"something"`.
pub fn fill_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut result = template.to_string();

    // First pass: replace known variables.
    for (key, value) in vars {
        let placeholder = format!("{{{key}}}");
        result = result.replace(&placeholder, value);
    }

    // Second pass: replace any remaining {word} placeholders with "something".
    // We scan for `{` and `}` pairs containing only alphanumeric + underscore chars.
    let mut output = String::with_capacity(result.len());
    let mut chars = result.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Try to read a placeholder name.
            let mut name = String::new();
            let mut found_close = false;

            for inner in chars.by_ref() {
                if inner == '}' {
                    found_close = true;
                    break;
                }
                name.push(inner);
            }

            if found_close
                && !name.is_empty()
                && name.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                // Valid placeholder — replace with fallback.
                output.push_str("something");
            } else {
                // Not a valid placeholder — emit as-is.
                output.push('{');
                output.push_str(&name);
                if found_close {
                    output.push('}');
                }
            }
        } else {
            output.push(ch);
        }
    }

    output
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn fill_template_replaces_vars() {
        let result = fill_template(
            "Create a task: {action} for {project}, due {due_date}",
            &[
                ("action", "write report"),
                ("project", "Q4 review"),
                ("due_date", "Friday"),
            ],
        );
        assert_eq!(
            result,
            "Create a task: write report for Q4 review, due Friday"
        );
    }

    #[test]
    fn fill_template_handles_missing_vars() {
        let result = fill_template(
            "I need to {action} by {due_date}",
            &[("action", "finish docs")],
        );
        assert_eq!(result, "I need to finish docs by something");
    }

    #[test]
    fn templates_for_known_topic() {
        assert_eq!(templates_for_topic("tasks"), TASK_TEMPLATES);
        assert_eq!(templates_for_topic("finance"), FINANCE_TEMPLATES);
    }

    #[test]
    fn templates_for_unknown_topic_defaults_to_chat() {
        assert_eq!(templates_for_topic("unknown_topic"), CHAT_TEMPLATES);
    }

    #[test]
    fn pick_template_returns_valid_entry() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let t = pick_template(TASK_TEMPLATES, &mut rng);
        assert!(TASK_TEMPLATES.contains(&t));
    }
}
