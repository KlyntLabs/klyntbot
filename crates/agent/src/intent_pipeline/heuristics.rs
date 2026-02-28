//! Enhanced heuristic classifier for the intent pipeline.
//!
//! Returns `Option<IntentAnalysis>` with full `ComplexitySignals` instead of
//! the old `ExecutionStrategy`. Defers to LLM classifier when ambiguous.

use super::types::{
    AnalysisSource, ComplexitySignals, ExecutionMode, FailureRisk, IntentAnalysis, ToolGroup,
};

/// Attempt to classify a message using keyword/pattern heuristics.
///
/// Returns `Some(IntentAnalysis)` for clear-cut intents, `None` when ambiguous
/// (should fall through to LLM classifier).
pub fn analyze_heuristic(message: &str) -> Option<IntentAnalysis> {
    let msg = message.trim().to_lowercase();
    let word_count = msg.split_whitespace().count();

    // 1. Greeting pattern — fast path, zero complexity
    if is_greeting(&msg) {
        return Some(direct_analysis(
            "Greeting detected",
            0.95,
            vec![ToolGroup::None],
        ));
    }

    // 2. Very short non-keyword messages → Direct
    if msg.len() < 20 && word_count <= 4 && !has_any_action_keyword(&msg) {
        return Some(direct_analysis(
            "Short message, no action keywords",
            0.85,
            vec![ToolGroup::None],
        ));
    }

    // 3. Task management patterns → Reactive (CRUD, low iteration budget)
    //    Must be checked BEFORE plan/code keywords because task descriptions
    //    often contain code-like words that are just the task *description*.
    if is_task_management(&msg) {
        return Some(reactive_analysis(
            5,
            "Task management CRUD operation",
            0.90,
            ComplexitySignals {
                estimated_tool_calls: 1,
                has_sequential_deps: false,
                failure_risk: FailureRisk::Low,
                requires_state_tracking: false,
                requires_retries: false,
            },
            vec![ToolGroup::TaskManagement, ToolGroup::Search],
        ));
    }

    // 4. Direct response patterns (questions, explanations)
    if is_direct_question(&msg) {
        // Check for conflicting action signals
        if has_action_keyword(&msg) {
            return None; // "what is the best way to implement X" → ambiguous
        }
        return Some(direct_analysis(
            "Question/explanation pattern",
            0.90,
            vec![ToolGroup::None],
        ));
    }

    // 5. Explicit plan keywords → Planned (before action keywords, since
    //    "create a plan" contains "create" which is also an action keyword)
    if has_plan_keyword(&msg) {
        let signals = analyze_complexity(&msg);
        return Some(IntentAnalysis {
            mode: ExecutionMode::Planned {
                visibility: plan::PlanVisibility::default(),
                max_steps: 15,
            },
            signals,
            confidence: 0.85,
            source: AnalysisSource::Heuristic,
            reasoning: "Explicit planning language detected".to_string(),
            tool_groups: vec![ToolGroup::Full],
        });
    }

    // 6. Structural complexity analysis for remaining messages
    let signals = analyze_complexity(&msg);
    let score = signals.complexity_score();

    // Sequential language detected but couldn't fully classify → defer to LLM
    if signals.has_sequential_deps {
        return None;
    }

    // Simple tool-assisted patterns (search, list, show)
    if has_tool_keyword(&msg) && score <= 1 {
        let groups = infer_tool_groups(&msg);
        return Some(reactive_analysis(
            5,
            "Simple tool-assisted operation",
            0.85,
            signals,
            groups,
        ));
    }

    // Code/action keywords without complexity → Reactive
    if has_action_keyword(&msg) && score <= 2 {
        return Some(reactive_analysis(
            10,
            "Code/action keyword detected",
            0.80,
            signals,
            vec![ToolGroup::Full],
        ));
    }

    // High complexity with clear autonomous signals → Planned
    if score >= 4 {
        return Some(IntentAnalysis {
            mode: ExecutionMode::Planned {
                visibility: plan::PlanVisibility::default(),
                max_steps: 10,
            },
            signals,
            confidence: 0.75,
            source: AnalysisSource::Heuristic,
            reasoning: "High complexity score from heuristic analysis".to_string(),
            tool_groups: vec![ToolGroup::Full],
        });
    }

    // Not enough signal — defer to LLM
    None
}

// ---------------------------------------------------------------------------
// Structural analysis helpers
// ---------------------------------------------------------------------------

fn is_greeting(msg: &str) -> bool {
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
    msg.len() < 20
        && GREETINGS.iter().any(|g| {
            msg == *g || msg.starts_with(&format!("{} ", g)) || msg.starts_with(&format!("{}!", g))
        })
}

fn is_task_management(msg: &str) -> bool {
    const TASK_MGMT: &[&str] = &[
        "create a task",
        "add a task",
        "add a todo",
        "create a todo",
        "new task",
        "new todo",
        "add task",
        "create todo",
    ];
    TASK_MGMT.iter().any(|k| msg.contains(k))
}

fn is_direct_question(msg: &str) -> bool {
    const DIRECT: &[&str] = &[
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
    DIRECT.iter().any(|k| msg.contains(k))
}

fn has_tool_keyword(msg: &str) -> bool {
    const TOOL: &[&str] = &[
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
    TOOL.iter().any(|k| msg.contains(k))
}

fn has_action_keyword(msg: &str) -> bool {
    const CODE: &[&str] = &[
        "fix",
        "build",
        "implement",
        "create",
        "add",
        "update",
        "refactor",
        "debug",
        "git",
        ".rs",
        ".py",
        ".ts",
        "function",
        "class",
        "struct",
        "file",
    ];
    CODE.iter().any(|k| msg.contains(k))
}

fn has_any_action_keyword(msg: &str) -> bool {
    has_tool_keyword(msg) || has_action_keyword(msg) || has_plan_keyword(msg)
}

fn has_plan_keyword(msg: &str) -> bool {
    const PLAN: &[&str] = &[
        "create a plan",
        "plan and implement",
        "plan to",
        "make a plan",
        "design and build",
        "architect and",
    ];
    PLAN.iter().any(|k| msg.contains(k))
}

/// Count indicators of tool usage in the message.
fn count_tool_indicators(msg: &str) -> u8 {
    let mut count: u8 = 0;
    let indicators = [
        "search", "find", "list", "show", "create", "update", "delete", "add", "remove", "fetch",
        "get", "read", "write", "send", "check", "look up", "set",
    ];
    for ind in &indicators {
        if msg.contains(ind) {
            count = count.saturating_add(1);
        }
    }
    count
}

/// Detect sequential/ordering language ("first...then", "after that", "step 1").
fn detect_sequential_language(msg: &str) -> bool {
    let patterns = [
        "first ",
        "then ",
        "after that",
        "next ",
        "finally ",
        "step 1",
        "step 2",
        "and then",
        "followed by",
        "before ",
        "once you",
        "once that",
    ];
    let matches = patterns.iter().filter(|p| msg.contains(**p)).count();
    matches >= 2 // Need at least 2 sequential indicators
}

/// Detect keywords suggesting operations that could fail.
fn assess_failure_risk(msg: &str) -> FailureRisk {
    let high_risk = [
        "deploy",
        "production",
        "database",
        "migration",
        "payment",
        "delete all",
    ];
    let medium_risk = [
        "api", "external", "network", "download", "upload", "install", "booking", "book ",
    ];

    if high_risk.iter().any(|k| msg.contains(k)) {
        return FailureRisk::High;
    }
    if medium_risk.iter().any(|k| msg.contains(k)) {
        return FailureRisk::Medium;
    }
    FailureRisk::Low
}

/// Detect if the task requires tracking state across steps.
fn detect_state_requirements(msg: &str) -> bool {
    let state_indicators = [
        "compare",
        "cheapest",
        "best",
        "most",
        "least",
        "between",
        "choose",
        "pick",
        "select from",
        "rank",
        "sort by",
    ];
    state_indicators.iter().any(|k| msg.contains(k))
}

/// Detect if the task might need retries (flaky operations).
fn detect_retry_indicators(msg: &str) -> bool {
    let retry_indicators = [
        "retry",
        "try again",
        "if it fails",
        "fallback",
        "keep trying",
        "until it works",
    ];
    retry_indicators.iter().any(|k| msg.contains(k))
}

/// Build full complexity signals from message analysis.
fn analyze_complexity(msg: &str) -> ComplexitySignals {
    ComplexitySignals {
        estimated_tool_calls: count_tool_indicators(msg),
        has_sequential_deps: detect_sequential_language(msg),
        failure_risk: assess_failure_risk(msg),
        requires_state_tracking: detect_state_requirements(msg),
        requires_retries: detect_retry_indicators(msg),
    }
}

// ---------------------------------------------------------------------------
// Analysis constructors
// ---------------------------------------------------------------------------

fn direct_analysis(
    reasoning: &str,
    confidence: f32,
    tool_groups: Vec<ToolGroup>,
) -> IntentAnalysis {
    IntentAnalysis {
        mode: ExecutionMode::Direct,
        signals: ComplexitySignals {
            estimated_tool_calls: 0,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        },
        confidence,
        source: AnalysisSource::Heuristic,
        reasoning: reasoning.to_string(),
        tool_groups,
    }
}

fn reactive_analysis(
    max_iterations: u32,
    reasoning: &str,
    confidence: f32,
    signals: ComplexitySignals,
    tool_groups: Vec<ToolGroup>,
) -> IntentAnalysis {
    IntentAnalysis {
        mode: ExecutionMode::Reactive { max_iterations },
        signals,
        confidence,
        source: AnalysisSource::Heuristic,
        reasoning: reasoning.to_string(),
        tool_groups,
    }
}

/// Infer relevant tool groups from message keywords.
fn infer_tool_groups(msg: &str) -> Vec<ToolGroup> {
    let mut groups = Vec::new();

    // Calendar keywords
    if msg.contains("calendar")
        || msg.contains("schedule")
        || msg.contains("event")
        || msg.contains("appointment")
        || msg.contains("meeting")
    {
        groups.push(ToolGroup::Calendar);
    }

    // Finance keywords
    if msg.contains("finance")
        || msg.contains("budget")
        || msg.contains("expense")
        || msg.contains("investment")
        || msg.contains("money")
        || msg.contains("spending")
    {
        groups.push(ToolGroup::Finance);
    }

    // Task/todo keywords
    if msg.contains("task") || msg.contains("todo") || msg.contains("goal") || msg.contains("plan")
    {
        groups.push(ToolGroup::TaskManagement);
    }

    // Search/file keywords
    if msg.contains("search")
        || msg.contains("find")
        || msg.contains("look up")
        || msg.contains("fetch")
        || msg.contains("read")
    {
        groups.push(ToolGroup::Search);
    }

    // Default to Search + Communication if nothing specific detected
    if groups.is_empty() {
        groups.push(ToolGroup::Search);
        groups.push(ToolGroup::Communication);
    }

    groups
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Plan-specified tests ---

    #[test]
    fn greeting_is_direct() {
        let result = analyze_heuristic("hello");
        assert!(result.is_some());
        assert!(matches!(result.unwrap().mode, ExecutionMode::Direct));
    }

    #[test]
    fn task_crud_is_reactive() {
        let result = analyze_heuristic("create a task to buy groceries");
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap().mode,
            ExecutionMode::Reactive { .. }
        ));
    }

    #[test]
    fn sequential_multi_step_is_none() {
        let result = analyze_heuristic(
            "first search for flights to Tokyo, then compare prices, and book the cheapest one",
        );
        // Ambiguous — should defer to LLM
        assert!(result.is_none());
    }

    #[test]
    fn simple_search_is_reactive() {
        let result = analyze_heuristic("search for tasks about database migration");
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap().mode,
            ExecutionMode::Reactive { .. }
        ));
    }

    #[test]
    fn what_is_question_is_direct() {
        let result = analyze_heuristic("what is the capital of France?");
        assert!(result.is_some());
        assert!(matches!(result.unwrap().mode, ExecutionMode::Direct));
    }

    // --- Additional coverage ---

    #[test]
    fn greeting_variants() {
        for greeting in &["hi", "hey", "hello!", "good morning", "  hi  "] {
            let result = analyze_heuristic(greeting);
            assert!(
                result.is_some() && matches!(result.as_ref().unwrap().mode, ExecutionMode::Direct),
                "Expected Direct for '{}', got {:?}",
                greeting,
                result
            );
        }
    }

    #[test]
    fn task_mgmt_overrides_code_keywords() {
        // Even though "implement" is an action keyword, task management takes priority
        let result = analyze_heuristic("create a task: implement the new auth system");
        assert!(result.is_some());
        let analysis = result.unwrap();
        assert!(matches!(
            analysis.mode,
            ExecutionMode::Reactive { max_iterations: 5 }
        ));
    }

    #[test]
    fn ambiguous_defers_to_llm() {
        // "what is the best way to implement" has both direct + action signals
        let result = analyze_heuristic("what is the best way to implement error handling in Rust?");
        assert!(result.is_none(), "Conflicting signals should defer to LLM");
    }

    #[test]
    fn short_message_no_keywords_is_direct() {
        let result = analyze_heuristic("ok cool");
        assert!(result.is_some());
        assert!(matches!(result.unwrap().mode, ExecutionMode::Direct));
    }

    #[test]
    fn explicit_plan_keyword_is_planned() {
        let result = analyze_heuristic("create a plan for the database refactor");
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap().mode,
            ExecutionMode::Planned { .. }
        ));
    }

    #[test]
    fn heuristic_source_is_always_heuristic() {
        let result = analyze_heuristic("hello").unwrap();
        assert_eq!(result.source, AnalysisSource::Heuristic);
    }

    // --- Tool group tests ---

    #[test]
    fn greeting_gets_no_tools() {
        let result = analyze_heuristic("hello").unwrap();
        assert!(result.tool_groups.contains(&ToolGroup::None));
    }

    #[test]
    fn task_crud_gets_task_management_and_search() {
        let result = analyze_heuristic("create a task to buy groceries").unwrap();
        assert!(result.tool_groups.contains(&ToolGroup::TaskManagement));
        assert!(result.tool_groups.contains(&ToolGroup::Search));
    }

    #[test]
    fn complex_planned_gets_full_tools() {
        let result = analyze_heuristic("create a plan for the database refactor").unwrap();
        assert!(result.tool_groups.contains(&ToolGroup::Full));
    }

    #[test]
    fn signals_populated_for_reactive() {
        let result = analyze_heuristic("search for tasks about database migration").unwrap();
        // Should have some tool call estimate
        assert!(result.signals.estimated_tool_calls >= 1);
    }

    #[test]
    fn sequential_language_detection() {
        assert!(detect_sequential_language(
            "first do X, then do Y, finally do Z"
        ));
        assert!(!detect_sequential_language("just do this one thing"));
    }

    #[test]
    fn failure_risk_assessment() {
        assert_eq!(
            assess_failure_risk("deploy to production"),
            FailureRisk::High
        );
        assert_eq!(
            assess_failure_risk("call the external api"),
            FailureRisk::Medium
        );
        assert_eq!(assess_failure_risk("list my tasks"), FailureRisk::Low);
    }
}
