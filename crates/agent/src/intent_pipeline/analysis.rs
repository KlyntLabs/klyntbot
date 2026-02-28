//! Intent analysis — heuristic classification, LLM classifier, and two-stage analyzer.
//!
//! Merges the former `heuristics`, `classifier`, and `analyzer` modules into a single
//! cohesive module that owns the full classification pipeline:
//!
//! 1. `analyze_heuristic()` — zero-cost keyword/pattern classification
//! 2. `IntentClassifier` — lightweight LLM call for ambiguous messages
//! 3. `IntentAnalyzer` — two-stage orchestrator (heuristics → LLM)

use std::time::{Duration, Instant};

use common::Result;
use config::OrchestratorConfig;
use providers::{ChatParams, DynProvider, Message};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use super::types::{
    AnalysisSource, ComplexitySignals, ExecutionMode, FailureRisk, IntentAnalysis, ToolGroup,
};

// ===========================================================================
// Heuristic classification
// ===========================================================================

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
                visibility: domain::PlanVisibility::default(),
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
                visibility: domain::PlanVisibility::default(),
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
// Heuristic helpers
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

    // Task/todo keywords — reuse is_task_management + extra domain terms
    if is_task_management(msg)
        || msg.contains("task")
        || msg.contains("todo")
        || msg.contains("goal")
        || msg.contains("plan")
    {
        groups.push(ToolGroup::TaskManagement);
    }

    // Search/file keywords — reuse has_tool_keyword
    if has_tool_keyword(msg) {
        groups.push(ToolGroup::Search);
    }

    // Default to Search + Communication if nothing specific detected
    if groups.is_empty() {
        groups.push(ToolGroup::Search);
        groups.push(ToolGroup::Communication);
    }

    groups
}

// ===========================================================================
// LLM-based classifier
// ===========================================================================

const CLASSIFICATION_PROMPT: &str = r#"Classify this user message and assess its complexity.

Respond ONLY with valid JSON:
{
  "mode": "direct" | "reactive" | "planned",
  "estimated_tool_calls": <0-10>,
  "has_sequential_deps": <true|false>,
  "failure_risk": "low" | "medium" | "high",
  "requires_state_tracking": <true|false>,
  "requires_retries": <true|false>,
  "relevant_tools": ["tool1", "tool2"],
  "confidence": <0.0-1.0>,
  "reasoning": "<brief explanation>"
}

Mode guide:
- "direct": Greetings, factual Q&A, explanations — no tools needed
- "reactive": Single-shot tasks needing tools — search, CRUD, lookups
- "planned": Multi-step tasks with dependencies, state tracking, or high failure risk

For "relevant_tools": list ONLY the tools from the available set that are needed.
Use an empty array for "direct" mode (no tools needed).

User message: "{message}"
Available tools: {tools}"#;

/// Classifies user intent via a lightweight LLM call, returning structured
/// `IntentAnalysis` with `ComplexitySignals`.
pub struct IntentClassifier {
    provider: DynProvider,
    timeout: Duration,
}

impl IntentClassifier {
    pub fn new(provider: DynProvider, timeout: Duration) -> Self {
        Self { provider, timeout }
    }

    /// Classify a user message using an LLM call.
    ///
    /// `strategy_context` optionally provides historical strategy performance
    /// data to help the LLM make better classification decisions.
    pub async fn classify(
        &self,
        message: &str,
        tool_names: &[&str],
        params: &ChatParams,
        strategy_context: Option<&str>,
    ) -> Result<IntentAnalysis> {
        let mut prompt = CLASSIFICATION_PROMPT
            .replace("{message}", message)
            .replace("{tools}", &tool_names.join(", "));

        if let Some(ctx) = strategy_context {
            prompt.push_str("\n\n");
            prompt.push_str(ctx);
        }

        let messages = vec![Message::user(prompt)];

        let result =
            tokio::time::timeout(self.timeout, self.provider.chat(&messages, None, params)).await;

        let response = match result {
            Ok(Ok(r)) => r,
            _ => return Ok(IntentAnalysis::fallback()),
        };

        let content = response.content.as_deref().unwrap_or("");
        Ok(Self::parse_classification_json(content))
    }

    fn parse_classification_json(content: &str) -> IntentAnalysis {
        let json_str = match common::utils::extract_json_object(content) {
            Some(s) => s,
            None => return IntentAnalysis::fallback(),
        };

        let v: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return IntentAnalysis::fallback(),
        };

        let mode_str = v["mode"].as_str().unwrap_or("reactive");
        let confidence = v["confidence"].as_f64().unwrap_or(0.7) as f32;
        let reasoning = v["reasoning"].as_str().unwrap_or("").to_string();

        let signals = ComplexitySignals {
            estimated_tool_calls: v["estimated_tool_calls"].as_u64().unwrap_or(1) as u8,
            has_sequential_deps: v["has_sequential_deps"].as_bool().unwrap_or(false),
            failure_risk: match v["failure_risk"].as_str().unwrap_or("low") {
                "high" => FailureRisk::High,
                "medium" => FailureRisk::Medium,
                _ => FailureRisk::Low,
            },
            requires_state_tracking: v["requires_state_tracking"].as_bool().unwrap_or(false),
            requires_retries: v["requires_retries"].as_bool().unwrap_or(false),
        };

        let mode = match mode_str {
            "direct" => ExecutionMode::Direct,
            "planned" => ExecutionMode::Planned {
                visibility: domain::PlanVisibility::default(),
                max_steps: signals.estimated_tool_calls.max(5),
            },
            _ => ExecutionMode::Reactive {
                max_iterations: (signals.estimated_tool_calls as u32).max(5),
            },
        };

        // Map relevant_tools from LLM response to ToolGroups
        let tool_groups = match mode_str {
            "direct" => vec![ToolGroup::None],
            _ => {
                let relevant: Vec<String> = v["relevant_tools"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                if relevant.is_empty() {
                    vec![ToolGroup::Full]
                } else {
                    map_tool_names_to_groups(&relevant)
                }
            }
        };

        IntentAnalysis {
            mode,
            signals,
            confidence,
            source: AnalysisSource::LlmClassifier,
            reasoning,
            tool_groups,
        }
    }
}

/// Map a list of tool names to the ToolGroups they belong to.
fn map_tool_names_to_groups(tool_names: &[String]) -> Vec<ToolGroup> {
    let all_groups = [
        ToolGroup::TaskManagement,
        ToolGroup::Search,
        ToolGroup::Calendar,
        ToolGroup::Finance,
        ToolGroup::Communication,
        ToolGroup::Automation,
    ];

    let mut matched = Vec::new();
    for group in &all_groups {
        let group_tools = group.tool_names();
        if tool_names
            .iter()
            .any(|name| group_tools.contains(&name.as_str()))
        {
            matched.push(group.clone());
        }
    }

    if matched.is_empty() {
        vec![ToolGroup::Full]
    } else {
        matched
    }
}

// ===========================================================================
// Two-stage analyzer (orchestrator)
// ===========================================================================

/// TTL for cached strategy context (seconds).
const STRATEGY_CACHE_TTL_SECS: u64 = 60;

/// Two-stage intent analyzer: heuristics → LLM classifier.
pub struct IntentAnalyzer {
    classifier: IntentClassifier,
    classifier_params: ChatParams,
    strategy_repo: Option<storage::StrategyRepo>,
    config: OrchestratorConfig,
    /// Cached strategy context to avoid hitting DB on every ambiguous message.
    strategy_cache: Mutex<Option<(Instant, Option<String>)>>,
}

impl IntentAnalyzer {
    pub fn new(provider: DynProvider, model: &str, config: &OrchestratorConfig) -> Self {
        let timeout = Duration::from_millis(config.llm_classifier_timeout);
        Self {
            classifier: IntentClassifier::new(provider, timeout),
            classifier_params: ChatParams::new(model),
            strategy_repo: None,
            config: config.clone(),
            strategy_cache: Mutex::new(None),
        }
    }

    pub fn with_strategy_repo(mut self, repo: storage::StrategyRepo) -> Self {
        self.strategy_repo = Some(repo);
        self
    }

    /// Analyze a user message and return the recommended execution mode.
    pub async fn analyze(&self, message: &str, tool_names: &[&str]) -> IntentAnalysis {
        // Stage 1: Heuristics (0ms)
        if let Some(analysis) = analyze_heuristic(message) {
            if analysis.confidence >= self.config.heuristic_confidence_threshold {
                debug!(
                    mode = ?analysis.mode,
                    confidence = analysis.confidence,
                    "Heuristic classification accepted"
                );
                return analysis;
            }
            debug!(
                confidence = analysis.confidence,
                threshold = self.config.heuristic_confidence_threshold,
                "Heuristic confidence below threshold, falling through to LLM"
            );
        }

        // Stage 2: LLM classifier
        let strategy_context = self.build_strategy_context().await;
        match self
            .classifier
            .classify(
                message,
                tool_names,
                &self.classifier_params,
                strategy_context.as_deref(),
            )
            .await
        {
            Ok(result) => {
                if result.confidence < 0.5 {
                    debug!(
                        confidence = result.confidence,
                        "LLM classifier low confidence, defaulting to Reactive"
                    );
                    return IntentAnalysis {
                        mode: ExecutionMode::Reactive { max_iterations: 10 },
                        source: AnalysisSource::LlmClassifier,
                        ..result
                    };
                }
                result
            }
            Err(e) => {
                warn!("LLM classifier error: {}, using fallback", e);
                IntentAnalysis::fallback()
            }
        }
    }

    async fn build_strategy_context(&self) -> Option<String> {
        let repo = self.strategy_repo.as_ref()?;

        // Check TTL cache first
        {
            let cache = self.strategy_cache.lock().await;
            if let Some((cached_at, ref value)) = *cache {
                if cached_at.elapsed() < Duration::from_secs(STRATEGY_CACHE_TTL_SECS) {
                    return value.clone();
                }
            }
        }

        let since = chrono::Utc::now() - chrono::Duration::days(30);
        let result = match repo.get_strategy_summaries(since).await {
            Ok(summaries) if !summaries.is_empty() => {
                let ctx = format_strategy_context(&summaries);
                debug!("Strategy feedback context: {} strategies", summaries.len());
                Some(ctx)
            }
            Ok(_) => None,
            Err(e) => {
                warn!("Failed to load strategy summaries: {}", e);
                None
            }
        };

        // Cache the result
        *self.strategy_cache.lock().await = Some((Instant::now(), result.clone()));
        result
    }
}

/// Format strategy summaries into a human-readable context for the LLM classifier.
pub(crate) fn format_strategy_context(summaries: &[storage::StrategySummaryRow]) -> String {
    let mut ctx = String::from("Historical strategy performance (last 30 days):\n");
    for s in summaries {
        let accuracy = if s.sample_count > 0 {
            s.correct_count as f32 / s.sample_count as f32 * 100.0
        } else {
            0.0
        };
        use std::fmt::Write;
        let _ = writeln!(
            ctx,
            "- {}: {:.0}% accuracy ({} samples), avg {:.1} escalations",
            s.predicted_strategy, accuracy, s.sample_count, s.avg_escalations
        );
    }
    ctx.push_str("Prefer strategies with higher historical accuracy when confidence is similar.");
    ctx
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use common::Result;
    use providers::{LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Arc;

    // ── Mock providers ──────────────────────────────────────────

    /// Mock that returns a fixed text response (for classifier tests).
    struct MockClassifierProvider {
        response_text: String,
    }

    impl MockClassifierProvider {
        fn new(text: &str) -> Self {
            Self {
                response_text: text.to_string(),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockClassifierProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some(self.response_text.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }

        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    fn mock_provider(text: &str) -> DynProvider {
        Arc::new(MockClassifierProvider::new(text))
    }

    /// Mock that panics if called — verifies heuristic path short-circuits.
    struct PanickingProvider;

    #[async_trait]
    impl LlmProvider for PanickingProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            panic!("LLM should not have been called — heuristic should have handled this");
        }

        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    fn default_params() -> ChatParams {
        ChatParams::new("test-model")
    }

    // ── Heuristic tests ─────────────────────────────────────────

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

    // ── Classifier tests ────────────────────────────────────────

    #[tokio::test]
    async fn parses_structured_classification() {
        let response = r#"{"mode":"planned","estimated_tool_calls":5,"has_sequential_deps":true,"failure_risk":"high","requires_state_tracking":true,"requires_retries":false,"confidence":0.9,"reasoning":"Multi-step booking"}"#;
        let classifier = IntentClassifier::new(mock_provider(response), Duration::from_secs(2));
        let result = classifier
            .classify(
                "book a flight",
                &["web_search", "web_fetch"],
                &default_params(),
                None,
            )
            .await
            .unwrap();
        assert!(matches!(result.mode, ExecutionMode::Planned { .. }));
        assert_eq!(result.signals.estimated_tool_calls, 5);
        assert!(result.signals.has_sequential_deps);
        assert_eq!(result.signals.failure_risk, FailureRisk::High);
        assert!((result.confidence - 0.9).abs() < 1e-6);
        assert_eq!(result.source, AnalysisSource::LlmClassifier);
    }

    #[tokio::test]
    async fn parses_direct_mode() {
        let response = r#"{"mode":"direct","estimated_tool_calls":0,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.95,"reasoning":"Simple greeting"}"#;
        let classifier = IntentClassifier::new(mock_provider(response), Duration::from_secs(2));
        let result = classifier
            .classify("hello", &[], &default_params(), None)
            .await
            .unwrap();
        assert!(matches!(result.mode, ExecutionMode::Direct));
    }

    #[tokio::test]
    async fn parses_reactive_mode() {
        let response = r#"{"mode":"reactive","estimated_tool_calls":2,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.85,"reasoning":"Needs search"}"#;
        let classifier = IntentClassifier::new(mock_provider(response), Duration::from_secs(2));
        let result = classifier
            .classify("search for tasks", &["todo"], &default_params(), None)
            .await
            .unwrap();
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 5 }
        ));
    }

    #[tokio::test]
    async fn invalid_json_returns_fallback() {
        let classifier = IntentClassifier::new(
            mock_provider("I can't classify this"),
            Duration::from_secs(2),
        );
        let result = classifier
            .classify("hello", &[], &default_params(), None)
            .await
            .unwrap();
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 10 }
        ));
        assert_eq!(result.source, AnalysisSource::Heuristic); // fallback uses Heuristic source
    }

    #[tokio::test]
    async fn json_embedded_in_text() {
        let response = r#"Sure: {"mode":"direct","estimated_tool_calls":0,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.9,"reasoning":"Greeting"} done"#;
        let classifier = IntentClassifier::new(mock_provider(response), Duration::from_secs(2));
        let result = classifier
            .classify("hi", &[], &default_params(), None)
            .await
            .unwrap();
        assert!(matches!(result.mode, ExecutionMode::Direct));
        assert_eq!(result.source, AnalysisSource::LlmClassifier);
    }

    #[test]
    fn extract_json_finds_object() {
        let input = r#"text {"key":"value"} more"#;
        assert_eq!(
            common::utils::extract_json_object(input),
            Some(r#"{"key":"value"}"#)
        );
    }

    #[test]
    fn extract_json_handles_no_json() {
        assert_eq!(common::utils::extract_json_object("no json here"), None);
    }

    #[test]
    fn reactive_max_iterations_uses_tool_calls() {
        let json = r#"{"mode":"reactive","estimated_tool_calls":8,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.8,"reasoning":"Many tools"}"#;
        let result = IntentClassifier::parse_classification_json(json);
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 8 }
        ));
    }

    // ── Analyzer tests ──────────────────────────────────────────

    #[tokio::test]
    async fn greeting_bypasses_llm() {
        let analyzer = IntentAnalyzer::new(
            Arc::new(PanickingProvider),
            "model",
            &OrchestratorConfig::default(),
        );
        let result = analyzer.analyze("hello", &[]).await;
        assert!(matches!(result.mode, ExecutionMode::Direct));
        assert_eq!(result.source, AnalysisSource::Heuristic);
    }

    #[tokio::test]
    async fn ambiguous_uses_llm() {
        let response = r#"{"mode":"reactive","estimated_tool_calls":2,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.8,"reasoning":"Needs search"}"#;
        let analyzer = IntentAnalyzer::new(
            Arc::new(MockClassifierProvider::new(response)),
            "model",
            &OrchestratorConfig::default(),
        );
        let result = analyzer
            .analyze(
                "I need help with understanding the codebase",
                &["web_search"],
            )
            .await;
        assert!(matches!(result.mode, ExecutionMode::Reactive { .. }));
        assert_eq!(result.source, AnalysisSource::LlmClassifier);
    }

    #[tokio::test]
    async fn low_confidence_llm_falls_back_to_reactive() {
        let response = r#"{"mode":"planned","estimated_tool_calls":1,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.3,"reasoning":"Unsure"}"#;
        let analyzer = IntentAnalyzer::new(
            Arc::new(MockClassifierProvider::new(response)),
            "model",
            &OrchestratorConfig::default(),
        );
        let result = analyzer.analyze("I need help with something", &[]).await;
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 10 }
        ));
    }

    #[tokio::test]
    async fn task_crud_bypasses_llm() {
        let analyzer = IntentAnalyzer::new(
            Arc::new(PanickingProvider),
            "model",
            &OrchestratorConfig::default(),
        );
        let result = analyzer
            .analyze("create a task to buy groceries", &[])
            .await;
        assert!(matches!(result.mode, ExecutionMode::Reactive { .. }));
        assert_eq!(result.source, AnalysisSource::Heuristic);
    }
}
