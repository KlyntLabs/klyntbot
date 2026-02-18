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

    // 1. Greeting pattern — fast path, no ambiguity
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

    // 2. Very short, non-keyword messages → DirectResponse
    if msg.len() < 20 && word_count <= 4 {
        // Check no other keyword categories fire
        if classify_by_keywords(&msg).is_none()
            || matches!(
                classify_by_keywords(&msg),
                Some(ExecutionStrategy::DirectResponse)
            )
        {
            return Some(ExecutionStrategy::DirectResponse);
        }
    }

    // 3. Plan keywords → AutonomousTask (checked before general keywords)
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

    // 4. Keyword-based classification with conflict detection
    classify_by_keywords(&msg)
}

/// Keywords that indicate autonomous multi-step execution.
const AUTONOMOUS_KEYWORDS: &[&str] = &[
    "write a script",
    "write me a script",
    "set up",
    "deploy",
    "implement a full",
    "build a full",
    "create a project",
    "write a program",
    "write a complete",
];

/// Keywords that indicate tool-assisted execution (search, lookup, etc.)
const TOOL_KEYWORDS: &[&str] = &[
    "search",
    "find",
    "list",
    "show",
    "fetch",
    "get",
    "read",
    "look up",
    "check",
    "show my tasks",
    "my todo",
    "list tasks",
];

/// Keywords that indicate code/task work (tool-assisted).
const CODE_KEYWORDS: &[&str] = &[
    "fix", "build", "implement", "create", "add", "update", "refactor", "debug", "git", ".rs",
    ".py", ".ts", "function", "class", "struct", "file",
];

/// Keywords that indicate a direct response (questions, explanations).
const DIRECT_KEYWORDS: &[&str] = &[
    "what is",
    "what are",
    "how does",
    "explain",
    "define",
    "tell me about",
    "who is",
    "why is",
    "when is",
    "describe",
];

fn classify_by_keywords(msg: &str) -> Option<ExecutionStrategy> {
    let has_autonomous = AUTONOMOUS_KEYWORDS.iter().any(|k| msg.contains(k));
    let has_tool = TOOL_KEYWORDS.iter().any(|k| msg.contains(k));
    let has_code = CODE_KEYWORDS.iter().any(|k| msg.contains(k));
    let has_direct = DIRECT_KEYWORDS.iter().any(|k| msg.contains(k));

    // Autonomous phrases are high-confidence multi-word patterns — they take priority
    // over single-word code/tool keywords that may co-occur incidentally.
    if has_autonomous {
        return Some(ExecutionStrategy::AutonomousTask {
            max_iterations: 15,
        });
    }

    // For the remaining categories, conflicting signals → defer to LLM
    let remaining_signals = has_tool as u8 + has_code as u8 + has_direct as u8;
    if remaining_signals > 1 {
        return None;
    }

    if has_tool {
        return Some(ExecutionStrategy::ToolAssisted { max_iterations: 5 });
    }

    if has_code {
        return Some(ExecutionStrategy::ToolAssisted {
            max_iterations: 10,
        });
    }

    if has_direct {
        return Some(ExecutionStrategy::DirectResponse);
    }

    None
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

    // --- Task 4.1 spec tests ---

    #[test]
    fn test_heuristic_question_direct_response() {
        // Simple questions → DirectResponse
        assert!(matches!(
            classify_heuristic("what is 2+2?"),
            Some(ExecutionStrategy::DirectResponse)
        ));
        assert!(matches!(
            classify_heuristic("what is Rust?"),
            Some(ExecutionStrategy::DirectResponse)
        ));
        assert!(matches!(
            classify_heuristic("how does async work?"),
            Some(ExecutionStrategy::DirectResponse)
        ));
        assert!(matches!(
            classify_heuristic("explain monads"),
            Some(ExecutionStrategy::DirectResponse)
        ));
    }

    #[test]
    fn test_heuristic_search_tool_assisted() {
        assert!(matches!(
            classify_heuristic("search for Rust tutorials online"),
            Some(ExecutionStrategy::ToolAssisted { .. })
        ));
    }

    #[test]
    fn test_heuristic_list_todos_tool_assisted() {
        assert!(matches!(
            classify_heuristic("list my todos"),
            Some(ExecutionStrategy::ToolAssisted { .. })
        ));
    }

    #[test]
    fn test_heuristic_autonomous_write_script() {
        assert!(matches!(
            classify_heuristic("write me a script that processes CSV files"),
            Some(ExecutionStrategy::AutonomousTask { .. })
        ));
    }

    #[test]
    fn test_heuristic_ambiguous_defers_to_llm() {
        // "I need help with..." is ambiguous — no clear signal category
        assert!(
            classify_heuristic("I need help with understanding the codebase").is_none(),
            "Ambiguous message should defer to LLM"
        );
    }

    #[test]
    fn test_heuristic_conflicting_signals_defers() {
        // Message with both direct ("what is") and code ("implement") signals → None
        assert!(
            classify_heuristic("what is the best way to implement error handling").is_none(),
            "Conflicting signals should defer to LLM"
        );
    }

    #[test]
    fn test_heuristic_tool_assisted_iterations_3_to_5() {
        // Tool keyword matches should have max_iterations 3-5
        if let Some(ExecutionStrategy::ToolAssisted { max_iterations }) =
            classify_heuristic("search for recent commits")
        {
            assert!(
                (3..=5).contains(&max_iterations),
                "Tool-assisted iterations should be 3-5, got {}",
                max_iterations
            );
        } else {
            panic!("Expected ToolAssisted");
        }
    }

    #[test]
    fn test_heuristic_autonomous_iterations_10_to_15() {
        if let Some(ExecutionStrategy::AutonomousTask { max_iterations }) =
            classify_heuristic("write me a script to automate deployments")
        {
            assert!(
                (10..=15).contains(&max_iterations),
                "Autonomous iterations should be 10-15, got {}",
                max_iterations
            );
        } else {
            panic!("Expected AutonomousTask");
        }
    }
}
