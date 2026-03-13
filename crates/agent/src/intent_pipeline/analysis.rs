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

use super::types::{AnalysisSource, ComplexitySignals, ExecutionMode, FailureRisk, IntentAnalysis};

/// Minimum iteration budget for orchestration and MCP-tool overrides.
pub const ORCHESTRATION_MIN_ITERATIONS: u32 = 15;

/// Confidence floor applied when multi-agent triggers are detected post-classification.
/// Distinct from `heuristic_confidence_threshold` (which governs LLM classifier bypass).
const ORCHESTRATION_MIN_CONFIDENCE: f32 = 0.75;

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
        return Some(direct_analysis("Greeting detected", 0.95));
    }

    // 1b. Multi-agent request → defer to LLM for orchestration.
    // Triggers from 2+ agent domains is sufficient — the LLM classifier
    // will decide whether orchestration is needed and set needs_orchestration.
    if has_multi_agent_triggers(&msg) {
        return None;
    }

    // 2. Task management patterns → Reactive (CRUD, low iteration budget)
    //    Must be checked BEFORE short-message heuristic because short phrases
    //    like "plan my day" or "list tasks" are actionable and need tools.
    if is_task_management(&msg) {
        let signals = ComplexitySignals {
            estimated_tool_calls: count_tool_indicators(&msg).max(1),
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        let budget = compute_iteration_budget(&signals);
        return Some(reactive_analysis(
            budget,
            "Task management operation",
            0.90,
            signals,
        ));
    }

    // 2b. Finance domain patterns → Reactive (CRUD, queries)
    if is_finance_operation(&msg) {
        let signals = ComplexitySignals {
            estimated_tool_calls: count_tool_indicators(&msg).max(1),
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        let budget = compute_iteration_budget(&signals);
        return Some(reactive_analysis(
            budget,
            "Finance operation",
            0.90,
            signals,
        ));
    }

    // 2c. Notes domain patterns → Reactive
    if is_notes_operation(&msg) {
        let signals = ComplexitySignals {
            estimated_tool_calls: count_tool_indicators(&msg).max(1),
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        let budget = compute_iteration_budget(&signals);
        return Some(reactive_analysis(budget, "Notes operation", 0.90, signals));
    }

    // 2d. Automation/scheduling domain patterns → Reactive
    if is_automation_operation(&msg) {
        let signals = ComplexitySignals {
            estimated_tool_calls: count_tool_indicators(&msg).max(1),
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        let budget = compute_iteration_budget(&signals);
        return Some(reactive_analysis(
            budget,
            "Automation/scheduling operation",
            0.90,
            signals,
        ));
    }

    // 3. Very short non-keyword messages → Direct
    if msg.len() < 20 && word_count <= 4 && !has_any_action_keyword(&msg) {
        return Some(direct_analysis("Short message, no action keywords", 0.85));
    }

    // 4. Direct response patterns (questions, explanations)
    if is_direct_question(&msg) {
        // Check for conflicting action signals
        if has_action_keyword(&msg) {
            return None; // "what is the best way to implement X" → ambiguous
        }
        return Some(direct_analysis("Question/explanation pattern", 0.90));
    }

    // 5. Complex workflow keywords → Reactive with high iteration budget
    if has_complex_workflow_keyword(&msg) {
        let signals = analyze_complexity(&msg);
        let budget = compute_iteration_budget(&signals);
        return Some(reactive_analysis(
            budget,
            "Complex workflow language detected — using high iteration budget",
            0.85,
            signals,
        ));
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
        let budget = compute_iteration_budget(&signals);
        return Some(reactive_analysis(
            budget,
            "Simple tool-assisted operation",
            0.85,
            signals,
        ));
    }

    // Code/action keywords without complexity → Reactive
    if has_action_keyword(&msg) && score <= 2 {
        let budget = compute_iteration_budget(&signals);
        return Some(reactive_analysis(
            budget,
            "Code/action keyword detected",
            0.80,
            signals,
        ));
    }

    // High complexity with clear autonomous signals → Reactive with high budget
    if score >= 4 {
        let budget = compute_iteration_budget(&signals);
        return Some(reactive_analysis(
            budget,
            "High complexity score from heuristic analysis",
            0.75,
            signals,
        ));
    }

    // Not enough signal — defer to LLM
    None
}

// ---------------------------------------------------------------------------
// Heuristic helpers
// ---------------------------------------------------------------------------

/// Check if the user message references MCP tools by server name.
///
/// Extracts server names from registered MCP tool names (e.g., `mcp_linear_list_issues`
/// → "linear") and checks if the message mentions any of them or the word "mcp".
/// This ensures MCP-related requests are routed to Reactive mode with tool execution.
fn references_mcp_tools(message: &str, tool_names: &[&str]) -> bool {
    let msg = message.trim().to_lowercase();

    // Collect unique MCP server names from registered tools
    let server_names: std::collections::HashSet<&str> = tool_names
        .iter()
        .filter_map(|name| mcp::sanitize::extract_server_name(name))
        .collect();

    if server_names.is_empty() {
        return false;
    }

    // Check if message mentions "mcp" or any server name
    msg.contains("mcp") || server_names.iter().any(|name| msg.contains(name))
}

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
        // CRUD patterns
        "create a task",
        "add a task",
        "add a todo",
        "create a todo",
        "new task",
        "new todo",
        // Query patterns
        "what task",
        "my task",
        "show task",
        "list task",
        "check task",
        "any task",
        "task list",
        "todo list",
        "how many task",
        "pending task",
        // Planning & agentic patterns
        "plan my day",
        "plan day",
        "daily plan",
        "decompose task",
        "decompose this",
        "break down task",
        "execute task",
        // Forecast & suggestion patterns
        "forecast",
        "accuracy report",
        "estimation accuracy",
        "suggest",
        "suggestion",
    ];

    if TASK_MGMT.iter().any(|k| msg.contains(k)) {
        return true;
    }

    // Possessive check: "my tasks", "my todos" — ownership implies query intent
    const TASK_NOUNS: &[&str] = &["task", "tasks", "todo", "todos"];
    if msg.starts_with("my ") && TASK_NOUNS.iter().any(|n| msg.contains(n)) {
        return true;
    }

    // Combinatorial check: action verb + task noun anywhere in message.
    // Catches "create 3 tasks", "add more todos", "delete the task", etc.
    const TASK_VERBS: &[&str] = &[
        "create", "add", "make", "update", "edit", "modify", "complete", "finish", "done",
        "delete", "remove", "list", "show", "check", "get",
    ];
    let has_verb = TASK_VERBS.iter().any(|v| msg.contains(v));
    let has_noun = TASK_NOUNS.iter().any(|n| msg.contains(n));
    has_verb && has_noun
}

fn is_finance_operation(msg: &str) -> bool {
    const FINANCE_PATTERNS: &[&str] = &[
        "log expense",
        "track expense",
        "add expense",
        "log income",
        "track income",
        "add income",
        "log transaction",
        "add transaction",
        "show budget",
        "my budget",
        "check budget",
        "show balance",
        "my balance",
        "check balance",
        "track spending",
        "my spending",
        "show spending",
        "budget report",
        "finance report",
        "expense report",
    ];

    if FINANCE_PATTERNS.iter().any(|k| msg.contains(k)) {
        return true;
    }

    // Combinatorial: finance verb + finance noun
    const FINANCE_VERBS: &[&str] = &[
        "log", "track", "add", "show", "check", "list", "create", "delete", "remove", "get",
    ];
    const FINANCE_NOUNS: &[&str] = &[
        "expense",
        "expenses",
        "income",
        "transaction",
        "transactions",
        "budget",
        "balance",
        "spending",
        "investment",
        "investments",
    ];
    let has_verb = FINANCE_VERBS.iter().any(|v| msg.contains(v));
    let has_noun = FINANCE_NOUNS.iter().any(|n| msg.contains(n));
    if has_verb && has_noun {
        return true;
    }

    // Possessive: "my budget", "my expenses"
    if msg.starts_with("my ") && FINANCE_NOUNS.iter().any(|n| msg.contains(n)) {
        return true;
    }

    false
}

fn is_notes_operation(msg: &str) -> bool {
    const NOTES_PATTERNS: &[&str] = &[
        "add note",
        "create note",
        "new note",
        "take note",
        "my notes",
        "show notes",
        "list notes",
        "note this",
        "note that",
        "jot down",
    ];

    if NOTES_PATTERNS.iter().any(|k| msg.contains(k)) {
        return true;
    }

    // Combinatorial: notes verb + notes noun
    const NOTES_VERBS: &[&str] = &[
        "add", "create", "take", "show", "list", "delete", "remove", "search", "find", "get",
    ];
    const NOTES_NOUNS: &[&str] = &["note", "notes", "memo", "memos"];
    let has_verb = NOTES_VERBS.iter().any(|v| msg.contains(v));
    let has_noun = NOTES_NOUNS.iter().any(|n| msg.contains(n));
    if has_verb && has_noun {
        return true;
    }

    false
}

fn is_automation_operation(msg: &str) -> bool {
    const AUTOMATION_PATTERNS: &[&str] = &[
        "remind me",
        "set reminder",
        "add reminder",
        "create reminder",
        "schedule task",
        "schedule a",
        "set up cron",
        "automate this",
        "automate that",
        "recurring task",
    ];
    AUTOMATION_PATTERNS.iter().any(|k| msg.contains(k))
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
    count_tool_indicators(msg) > 0
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
    has_tool_keyword(msg)
        || has_action_keyword(msg)
        || has_complex_workflow_keyword(msg)
        || has_domain_verb(msg)
}

/// Domain-specific verbs not covered by tool/action/workflow keyword lists.
/// Acts as a safety net: if a short message contains one of these, the
/// short-message guard won't swallow it — it'll fall through to later
/// classification steps or the LLM classifier.
fn has_domain_verb(msg: &str) -> bool {
    const DOMAIN_VERBS: &[&str] = &[
        "log",
        "track",
        "plan",
        "remind",
        "schedule",
        "budget",
        "note",
        "annotate",
        "complete",
        "decompose",
        "execute",
        "forecast",
        "summarize",
        "record",
    ];
    let words: Vec<&str> = msg.split_whitespace().collect();
    words
        .iter()
        .any(|w| DOMAIN_VERBS.contains(&w.trim_matches(|c: char| !c.is_alphabetic())))
}

fn has_complex_workflow_keyword(msg: &str) -> bool {
    const COMPLEX_WORKFLOW: &[&str] = &[
        "create a plan",
        "plan and implement",
        "plan to",
        "make a plan",
        "design and build",
        "architect and",
    ];
    COMPLEX_WORKFLOW.iter().any(|k| msg.contains(k))
}

/// Count indicators of tool usage in the message.
/// Counts each word-level occurrence so "create X and create Y" = 2.
/// Uses word splitting instead of substring matching to avoid false positives
/// (e.g. "forget" matching "get", "asset" matching "set").
fn count_tool_indicators(msg: &str) -> u8 {
    const INDICATORS: &[&str] = &[
        "search", "find", "list", "show", "create", "update", "delete", "add", "remove", "fetch",
        "get", "read", "write", "send", "check", "set", "organize", "review",
    ];
    let words: Vec<&str> = msg.split_whitespace().collect();
    let mut count: u8 = 0;
    for (i, word) in words.iter().enumerate() {
        let clean = word.trim_matches(|c: char| !c.is_alphabetic());
        if INDICATORS.contains(&clean) {
            count = count.saturating_add(1);
        }
        // Multi-word: "look up"
        if clean == "look" {
            if let Some(next) = words.get(i + 1) {
                if next.trim_matches(|c: char| !c.is_alphabetic()) == "up" {
                    count = count.saturating_add(1);
                }
            }
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

/// Check if a message contains triggers from 2+ different agent domains.
fn has_multi_agent_triggers(msg: &str) -> bool {
    let agent_trigger_groups: &[&[&str]] = &[
        // task agent triggers (includes calendar — handled via MCP)
        &[
            "todo",
            "task",
            "create a task",
            "my tasks",
            "focus",
            "project",
            "area",
            "objective",
            "calendar",
            "schedule",
            "meeting",
            "event",
            "appointment",
        ],
        // finance agent triggers
        &[
            "transaction",
            "budget",
            "expense",
            "income",
            "balance",
            "account",
            "finance",
            "spending",
            "investment",
        ],
        // automation agent triggers
        &[
            "cron",
            "automate",
            "schedule a",
            "every day",
            "recurring",
            "remind",
            "reminder",
            "remind me",
        ],
        // communication agent triggers
        &["send", "email", "message", "notify", "slack", "telegram"],
    ];

    let matched_groups = agent_trigger_groups
        .iter()
        .filter(|triggers| triggers.iter().any(|t| msg.contains(t)))
        .count();

    matched_groups >= 2
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

/// Compute the iteration budget for a Reactive execution from complexity signals.
///
/// Delegates to [`ComplexitySignals::iteration_budget()`].
pub fn compute_iteration_budget(signals: &ComplexitySignals) -> u32 {
    signals.iteration_budget()
}

fn direct_analysis(reasoning: &str, confidence: f32) -> IntentAnalysis {
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
        needs_orchestration: false,
    }
}

fn reactive_analysis(
    max_iterations: u32,
    reasoning: &str,
    confidence: f32,
    signals: ComplexitySignals,
) -> IntentAnalysis {
    IntentAnalysis {
        mode: ExecutionMode::Reactive { max_iterations },
        signals,
        confidence,
        source: AnalysisSource::Heuristic,
        reasoning: reasoning.to_string(),
        needs_orchestration: false,
    }
}

// ===========================================================================
// LLM-based classifier
// ===========================================================================

const CLASSIFICATION_PROMPT: &str = r#"Classify this user message and assess its complexity.

Respond ONLY with valid JSON:
{
  "mode": "direct" | "reactive",
  "estimated_tool_calls": <0-10>,
  "has_sequential_deps": <true|false>,
  "failure_risk": "low" | "medium" | "high",
  "requires_state_tracking": <true|false>,
  "requires_retries": <true|false>,
  "relevant_tools": ["tool1", "tool2"],
  "needs_orchestration": <true|false>,
  "confidence": <0.0-1.0>,
  "reasoning": "<brief explanation>"
}

Mode guide:
- "direct": Greetings, factual Q&A, explanations — no tools needed
- "reactive": Tasks needing tools — search, CRUD, lookups, multi-step workflows

For "needs_orchestration": true if the request involves multiple distinct domains
(e.g., "check transactions then create a task" spans finance + tasks).

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
        let json_str = match common::helpers::extract_json_object(content) {
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
            _ => ExecutionMode::Reactive {
                max_iterations: compute_iteration_budget(&signals),
            },
        };

        let needs_orchestration = v["needs_orchestration"].as_bool().unwrap_or(false);

        IntentAnalysis {
            mode,
            signals,
            confidence,
            source: AnalysisSource::LlmClassifier,
            reasoning,
            needs_orchestration,
        }
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
        // Use dedicated classifier provider if available, otherwise use main provider
        let classifier_provider = provider
            .classifier_provider()
            .unwrap_or_else(|| provider.clone());
        let classifier_model = config.llm_classifier_model.as_deref().unwrap_or(model);
        Self {
            classifier: IntentClassifier::new(classifier_provider, timeout),
            classifier_params: ChatParams::new(classifier_model),
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
        let mut analysis = if let Some(analysis) = analyze_heuristic(message) {
            if analysis.confidence >= self.config.heuristic_confidence_threshold {
                debug!(
                    mode = ?analysis.mode,
                    confidence = analysis.confidence,
                    "Heuristic classification accepted"
                );
                analysis
            } else {
                debug!(
                    confidence = analysis.confidence,
                    threshold = self.config.heuristic_confidence_threshold,
                    "Heuristic confidence below threshold, falling through to LLM"
                );
                self.classify_with_llm(message, tool_names).await
            }
        } else {
            self.classify_with_llm(message, tool_names).await
        };

        // Post-classification: override Direct → Reactive when MCP tools are referenced.
        // MCP tools are user-configured integrations that always require tool execution.
        if matches!(analysis.mode, ExecutionMode::Direct)
            && references_mcp_tools(message, tool_names)
        {
            debug!("Overriding Direct → Reactive: message references MCP tools");
            analysis.mode = ExecutionMode::Reactive {
                max_iterations: compute_iteration_budget(&analysis.signals)
                    .max(ORCHESTRATION_MIN_ITERATIONS),
            };
        }

        // Post-classification: multi-agent trigger detection is deterministic.
        // If 2+ agent domains are present, force orchestration regardless of
        // whether the LLM succeeded or fell back. Also boost confidence so the
        // confidence gate in AgentRuntime doesn't short-circuit to clarification.
        if !analysis.needs_orchestration {
            let msg_lower = message.trim().to_lowercase();
            if has_multi_agent_triggers(&msg_lower) {
                debug!(
                    "Post-classification: forcing needs_orchestration from multi-agent triggers"
                );
                analysis.needs_orchestration = true;
                analysis.confidence = analysis.confidence.max(ORCHESTRATION_MIN_CONFIDENCE);
                // Ensure Reactive mode with enough budget for orchestration
                if matches!(analysis.mode, ExecutionMode::Direct) {
                    analysis.mode = ExecutionMode::Reactive {
                        max_iterations: compute_iteration_budget(&analysis.signals)
                            .max(ORCHESTRATION_MIN_ITERATIONS),
                    };
                }
            }
        }

        analysis
    }

    /// Run LLM classifier (Stage 2) with fallback handling.
    async fn classify_with_llm(&self, message: &str, tool_names: &[&str]) -> IntentAnalysis {
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
                    IntentAnalysis {
                        mode: ExecutionMode::Reactive {
                            max_iterations: compute_iteration_budget(&result.signals),
                        },
                        source: AnalysisSource::LlmClassifier,
                        ..result
                    }
                } else {
                    result
                }
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
    fn task_query_is_reactive() {
        for query in &[
            "what task do we have?",
            "What tasks are pending?",
            "show my tasks",
            "list tasks for today",
            "check tasks",
            "any tasks related to auth?",
            "todo list",
        ] {
            let result = analyze_heuristic(query);
            assert!(
                result.is_some()
                    && matches!(
                        result.as_ref().unwrap().mode,
                        ExecutionMode::Reactive { .. }
                    ),
                "Expected Reactive for '{}', got {:?}",
                query,
                result
            );
        }
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
        assert!(matches!(analysis.mode, ExecutionMode::Reactive { .. }));
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
    fn complex_workflow_keyword_is_reactive() {
        let result = analyze_heuristic("create a plan for the database refactor");
        assert!(result.is_some());
        assert!(matches!(
            result.unwrap().mode,
            ExecutionMode::Reactive { .. }
        ));
    }

    #[test]
    fn heuristic_source_is_always_heuristic() {
        let result = analyze_heuristic("hello").unwrap();
        assert_eq!(result.source, AnalysisSource::Heuristic);
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
        let response = r#"{"mode":"reactive","estimated_tool_calls":5,"has_sequential_deps":true,"failure_risk":"high","requires_state_tracking":true,"requires_retries":false,"confidence":0.9,"reasoning":"Multi-step booking"}"#;
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
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 20 }
        ));
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
            ExecutionMode::Reactive { max_iterations: 15 }
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
            ExecutionMode::Reactive { max_iterations: 15 }
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
            common::helpers::extract_json_object(input),
            Some(r#"{"key":"value"}"#)
        );
    }

    #[test]
    fn extract_json_handles_no_json() {
        assert_eq!(common::helpers::extract_json_object("no json here"), None);
    }

    #[test]
    fn reactive_max_iterations_uses_tool_calls() {
        let json = r#"{"mode":"reactive","estimated_tool_calls":8,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.8,"reasoning":"Many tools"}"#;
        let result = IntentClassifier::parse_classification_json(json);
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 29 }
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
        let response = r#"{"mode":"reactive","estimated_tool_calls":1,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.3,"reasoning":"Unsure"}"#;
        let analyzer = IntentAnalyzer::new(
            Arc::new(MockClassifierProvider::new(response)),
            "model",
            &OrchestratorConfig::default(),
        );
        let result = analyzer.analyze("I need help with something", &[]).await;
        assert!(matches!(
            result.mode,
            ExecutionMode::Reactive { max_iterations: 15 }
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

    // ── Iteration budget tests ─────────────────────────────────

    #[test]
    fn compute_budget_single_tool_call() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 1,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(compute_iteration_budget(&signals), 15);
    }

    #[test]
    fn compute_budget_many_tool_calls() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 8,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(compute_iteration_budget(&signals), 29);
    }

    #[test]
    fn compute_budget_capped_at_30() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 20,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(compute_iteration_budget(&signals), 30);
    }

    #[test]
    fn compute_budget_zero_tool_calls_gets_floor() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 0,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(compute_iteration_budget(&signals), 15);
    }

    // ── MCP tool reference detection ──

    #[test]
    fn references_mcp_tools_detects_server_name() {
        let tools = &["mcp_linear_list_issues", "mcp_linear_get_issue", "task"];
        assert!(references_mcp_tools("show my linear issues", tools));
        assert!(references_mcp_tools("use Linear MCP", tools));
    }

    #[test]
    fn references_mcp_tools_detects_mcp_keyword() {
        let tools = &["mcp_linear_list_issues"];
        assert!(references_mcp_tools(
            "help me use mcp to check issues",
            tools
        ));
    }

    #[test]
    fn references_mcp_tools_no_match_without_mcp_tools() {
        let tools = &["task", "calendar", "grep"];
        assert!(!references_mcp_tools("show my linear issues", tools));
    }

    #[test]
    fn references_mcp_tools_no_match_unrelated_message() {
        let tools = &["mcp_linear_list_issues"];
        assert!(!references_mcp_tools("what is the weather today", tools));
    }

    #[tokio::test]
    async fn analyzer_overrides_direct_to_reactive_for_mcp_requests() {
        // Use PanickingProvider — heuristic should handle "hello" as Direct,
        // but the MCP override should flip it to Reactive when tools reference "linear"
        let analyzer = IntentAnalyzer::new(
            Arc::new(PanickingProvider),
            "model",
            &OrchestratorConfig::default(),
        );

        // "hello" normally classifies as Direct via heuristic
        let mcp_tools = &["mcp_linear_list_issues", "mcp_linear_get_issue"];
        let result = analyzer.analyze("hello", mcp_tools).await;
        // No override — "hello" doesn't mention "linear" or "mcp"
        assert!(matches!(result.mode, ExecutionMode::Direct));

        // Now with a message that references MCP
        let result = analyzer
            .analyze("help me check my linear issues", mcp_tools)
            .await;
        assert!(
            matches!(result.mode, ExecutionMode::Reactive { .. }),
            "Expected Reactive for MCP-referencing message, got {:?}",
            result.mode
        );
    }

    #[test]
    fn multi_agent_triggers_defer_to_llm() {
        // Messages spanning 2+ agent domains should always defer to LLM classifier
        let cases = [
            "Help me check what transaction i already have? If not exists help me create a task",
            "check my transactions then create a task for the missing ones",
            "what meetings do I have today and are there any related tasks",
            "send an email about the budget report",
        ];
        for msg in &cases {
            let result = analyze_heuristic(msg);
            assert!(
                result.is_none(),
                "Expected heuristic to defer multi-agent message to LLM: {msg}"
            );
        }
    }

    #[test]
    fn single_agent_triggers_not_deferred() {
        // Messages targeting a single domain should NOT defer
        let result = analyze_heuristic("create a task to review the code");
        assert!(
            result.is_some(),
            "Single-domain task message should not defer"
        );

        let result = analyze_heuristic("check my account balance");
        assert!(
            result.is_some(),
            "Single-domain finance message should not defer"
        );
    }

    // ── A-001: Short domain commands must route to Reactive ────

    #[test]
    fn short_task_commands_are_reactive() {
        for cmd in &["my todos", "my todo", "plan my day", "list tasks"] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some()
                    && matches!(
                        result.as_ref().unwrap().mode,
                        ExecutionMode::Reactive { .. }
                    ),
                "Expected Reactive for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    #[test]
    fn short_finance_commands_are_reactive() {
        for cmd in &[
            "log expense",
            "track spending",
            "show budget",
            "my budget",
            "log income",
        ] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some()
                    && matches!(
                        result.as_ref().unwrap().mode,
                        ExecutionMode::Reactive { .. }
                    ),
                "Expected Reactive for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    #[test]
    fn short_notes_commands_are_reactive() {
        for cmd in &["my notes", "take notes", "note this"] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some()
                    && matches!(
                        result.as_ref().unwrap().mode,
                        ExecutionMode::Reactive { .. }
                    ),
                "Expected Reactive for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    #[test]
    fn short_automation_commands_are_reactive() {
        for cmd in &["remind me", "schedule task"] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some()
                    && matches!(
                        result.as_ref().unwrap().mode,
                        ExecutionMode::Reactive { .. }
                    ),
                "Expected Reactive for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    #[test]
    fn short_inert_messages_still_direct() {
        // These should NOT be affected — they're genuinely non-actionable
        for cmd in &["ok cool", "thanks", "yes", "got it", "nice"] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some()
                    && matches!(result.as_ref().unwrap().mode, ExecutionMode::Direct),
                "Expected Direct for '{}', got {:?}",
                cmd,
                result
            );
        }
    }
}
