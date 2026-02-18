//! Heuristic pre-filter for zero-cost intent classification.
//!
//! Classifies user messages into execution strategies using pattern matching,
//! avoiding LLM calls for obvious cases (greetings, simple questions, code tasks).
//! Returns `None` when the message is ambiguous and needs LLM classification.

use context_engine::ExecutionStrategy;

/// Attempt to classify a message using keyword/pattern heuristics.
///
/// Returns `Some(strategy)` for clear-cut intents, `None` when ambiguous
/// (should fall through to LLM classifier).
pub fn classify_heuristic(message: &str) -> Option<ExecutionStrategy> {
    let msg = message.trim().to_lowercase();
    let word_count = msg.split_whitespace().count();

    // 1. Greeting pattern
    const GREETINGS: &[&str] = &[
        "hi",
        "hello",
        "hey",
        "howdy",
        "greetings",
        "good morning",
        "good afternoon",
        "good evening",
    ];
    if msg.len() < 20
        && GREETINGS.iter().any(|g| {
            msg == *g
                || msg.starts_with(&format!("{} ", g))
                || msg.starts_with(&format!("{}!", g))
        })
    {
        return Some(ExecutionStrategy::DirectResponse);
    }

    // 2. Very short, non-code messages -> DirectResponse
    if msg.len() < 20 && word_count <= 4 && !contains_code_keywords(&msg) {
        return Some(ExecutionStrategy::DirectResponse);
    }

    // 3. Plan keywords -> AutonomousTask
    const PLAN_KEYWORDS: &[&str] = &[
        "create a plan",
        "plan and implement",
        "plan to",
        "make a plan",
        "design and build",
        "architect and",
    ];
    if PLAN_KEYWORDS.iter().any(|k| msg.contains(k)) {
        return Some(ExecutionStrategy::AutonomousTask {
            max_iterations: 50,
        });
    }

    // 4. Code/task keywords -> ToolAssisted
    if contains_code_keywords(&msg) {
        return Some(ExecutionStrategy::ToolAssisted {
            max_iterations: 10,
        });
    }

    // 5. Fall through to LLM classifier
    None
}

fn contains_code_keywords(msg: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "fix",
        "build",
        "implement",
        "create",
        "add",
        "update",
        "refactor",
        "debug",
        "show my tasks",
        "my todo",
        "list tasks",
        "git",
        ".rs",
        ".py",
        ".ts",
        "function",
        "class",
        "struct",
        "file",
    ];
    KEYWORDS.iter().any(|k| msg.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greeting_detected_as_direct() {
        assert!(matches!(
            classify_heuristic("hi"),
            Some(ExecutionStrategy::DirectResponse)
        ));
        assert!(matches!(
            classify_heuristic("hello!"),
            Some(ExecutionStrategy::DirectResponse)
        ));
        assert!(matches!(
            classify_heuristic("hey"),
            Some(ExecutionStrategy::DirectResponse)
        ));
    }

    #[test]
    fn test_greeting_with_whitespace() {
        assert!(matches!(
            classify_heuristic("  hi  "),
            Some(ExecutionStrategy::DirectResponse)
        ));
        assert!(matches!(
            classify_heuristic("hello! "),
            Some(ExecutionStrategy::DirectResponse)
        ));
    }

    #[test]
    fn test_very_short_message_direct() {
        assert!(matches!(
            classify_heuristic("what time is it?"),
            Some(ExecutionStrategy::DirectResponse)
        ));
    }

    #[test]
    fn test_code_keywords_tool_assisted() {
        assert!(matches!(
            classify_heuristic("fix the auth bug in login.rs"),
            Some(ExecutionStrategy::ToolAssisted { .. })
        ));
        assert!(matches!(
            classify_heuristic("build the new feature"),
            Some(ExecutionStrategy::ToolAssisted { .. })
        ));
        assert!(matches!(
            classify_heuristic("show my tasks"),
            Some(ExecutionStrategy::ToolAssisted { .. })
        ));
    }

    #[test]
    fn test_plan_keywords_autonomous() {
        assert!(matches!(
            classify_heuristic("create a plan for the database refactor"),
            Some(ExecutionStrategy::AutonomousTask { .. })
        ));
        assert!(matches!(
            classify_heuristic("plan and implement the new auth system"),
            Some(ExecutionStrategy::AutonomousTask { .. })
        ));
    }

    #[test]
    fn test_ambiguous_falls_through() {
        assert!(
            classify_heuristic("what do you think about the architecture decision?").is_none()
        );
        assert!(
            classify_heuristic("can you help me understand this concept better?").is_none()
        );
    }

    #[test]
    fn test_autonomous_max_iterations() {
        if let Some(ExecutionStrategy::AutonomousTask { max_iterations }) =
            classify_heuristic("create a plan for migration")
        {
            assert_eq!(max_iterations, 50);
        } else {
            panic!("Expected AutonomousTask");
        }
    }

    #[test]
    fn test_tool_assisted_max_iterations() {
        if let Some(ExecutionStrategy::ToolAssisted { max_iterations }) =
            classify_heuristic("fix the login bug")
        {
            assert_eq!(max_iterations, 10);
        } else {
            panic!("Expected ToolAssisted");
        }
    }
}
