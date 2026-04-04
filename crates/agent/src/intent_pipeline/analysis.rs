//! Intent analysis — 2-layer heuristic cascade for complexity assessment.
//!
//! 1. `analyze_heuristic()` — ultra-fast Aho-Corasick + HashSet keyword classification
//! 2. Embedding heuristic fallback — cosine similarity against pre-computed intent centroids
//!
//! Public API:
//! - `pub fn analyze_heuristic(message: &str) -> Option<IntentAnalysis>`
//! - `pub struct IntentAnalyzer` + `analyze` + `with_strategy_repo`

use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use aho_corasick::AhoCorasick;
use config::OrchestratorConfig;
use tokio::sync::Mutex;
use tracing::debug;

use super::types::{
    AnalysisSource, ComplexityLevel, ComplexitySignals, FailureRisk, IntentAnalysis,
};

/// Confidence floor applied when multi-agent triggers are detected post-classification.
const ORCHESTRATION_MIN_CONFIDENCE: f32 = 0.75;

/// TTL for cached adaptive threshold (seconds).
const THRESHOLD_CACHE_TTL_SECS: u64 = 300;

// ===========================================================================
// Layer 1: Aho-Corasick matchers (OnceLock global)
// ===========================================================================

/// All heavy pattern-matching structures, initialized once via `OnceLock`.
struct AcMatchers {
    // ── Greeting ──
    greeting_starts: AhoCorasick,
    greeting_exact: HashSet<&'static str>,

    // ── Domain substring matchers ──
    task_mgmt: AhoCorasick,
    finance_patterns: AhoCorasick,
    notes_patterns: AhoCorasick,
    automation_patterns: AhoCorasick,

    // ── Word-level sets (need word boundary) ──
    task_nouns: HashSet<&'static str>,
    task_verbs: HashSet<&'static str>,
    finance_nouns: HashSet<&'static str>,
    finance_verbs: HashSet<&'static str>,
    notes_nouns: HashSet<&'static str>,
    notes_verbs: HashSet<&'static str>,

    // ── Direct question ──
    direct_question: AhoCorasick,

    // ── Action / tool / workflow ──
    action_keywords: AhoCorasick,
    complex_workflow: AhoCorasick,
    tool_indicators: HashSet<&'static str>,
    domain_verbs: HashSet<&'static str>,

    // ── Sequential language ──
    sequential: AhoCorasick,

    // ── Failure risk ──
    high_risk: AhoCorasick,
    medium_risk: AhoCorasick,

    // ── State / retry ──
    state_indicators: AhoCorasick,
    retry_indicators: AhoCorasick,

    // ── Multi-agent trigger groups ──
    agent_task_triggers: AhoCorasick,
    agent_finance_triggers: AhoCorasick,
    agent_automation_triggers: AhoCorasick,
    agent_communication_triggers: AhoCorasick,

    // ── Negation / hypothetical ──
    negation: AhoCorasick,
    hypothetical: AhoCorasick,
}

fn ac(patterns: &[&str]) -> AhoCorasick {
    AhoCorasick::new(patterns).expect("valid AC patterns")
}

fn hs(words: &[&'static str]) -> HashSet<&'static str> {
    words.iter().copied().collect()
}

static MATCHERS: OnceLock<AcMatchers> = OnceLock::new();

fn matchers() -> &'static AcMatchers {
    MATCHERS.get_or_init(|| {
        // ── Greeting ──
        let greeting_starts_raw = &[
            "hello ",
            "hello!",
            "hey ",
            "hey!",
            "howdy ",
            "howdy!",
            "greetings ",
            "greetings!",
            "good morning",
            "good afternoon",
            "good evening",
        ];
        let greeting_starts = ac(greeting_starts_raw);
        let greeting_exact = hs(&["hi", "hello", "hey", "howdy", "greetings"]);

        // ── Task management ──
        let task_mgmt_raw = &[
            "create a task",
            "add a task",
            "add a todo",
            "create a todo",
            "new task",
            "new todo",
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
            "plan my day",
            "plan day",
            "daily plan",
            "decompose task",
            "decompose this",
            "break down task",
            "execute task",
            "forecast",
            "accuracy report",
            "estimation accuracy",
            "suggest",
            "suggestion",
        ];
        let task_mgmt = ac(task_mgmt_raw);
        let task_nouns = hs(&[
            "task", "tasks", "todo", "todos", "area", "areas", "project", "projects",
        ]);
        let task_verbs = hs(&[
            "create", "add", "make", "update", "edit", "modify", "complete", "finish", "done",
            "delete", "remove", "list", "show", "check", "get",
        ]);

        // ── Finance ──
        let finance_raw = &[
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
            // Question-phrased finance queries
            "doing on spending",
            "doing on budget",
            "doing on expenses",
            "doing on investments",
            "how much did",
            "how much have",
            "how much do",
            "net worth",
            "savings rate",
            "cash flow",
        ];
        let finance_patterns = ac(finance_raw);
        let finance_nouns = hs(&[
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
        ]);
        let finance_verbs = hs(&[
            "log", "track", "add", "show", "check", "list", "create", "delete", "remove", "get",
        ]);

        // ── Notes ──
        let notes_raw = &[
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
        let notes_patterns = ac(notes_raw);
        let notes_nouns = hs(&["note", "notes", "memo", "memos"]);
        let notes_verbs = hs(&[
            "add", "create", "take", "show", "list", "delete", "remove", "search", "find", "get",
        ]);

        // ── Automation ──
        let automation_raw = &[
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
        let automation_patterns = ac(automation_raw);

        // ── Direct question ──
        let direct_raw = &[
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
        let direct_question = ac(direct_raw);

        // ── Action keywords (substring) ──
        let action_raw = &[
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
        let action_keywords = ac(action_raw);

        // ── Complex workflow ──
        let complex_raw = &[
            "create a plan",
            "plan and implement",
            "plan to",
            "make a plan",
            "design and build",
            "architect and",
        ];
        let complex_workflow = ac(complex_raw);

        // ── Tool indicators (word-level) ──
        let tool_indicators = hs(&[
            "search", "find", "list", "show", "create", "update", "delete", "add", "remove",
            "fetch", "get", "read", "write", "send", "check", "set", "organize", "review",
        ]);

        // ── Domain verbs (word-level) ──
        let domain_verbs = hs(&[
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
        ]);

        // ── Sequential ──
        let sequential_raw = &[
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
        let sequential = ac(sequential_raw);

        // ── Failure risk ──
        let high_risk_raw = &[
            "deploy",
            "production",
            "database",
            "migration",
            "payment",
            "delete all",
        ];
        let high_risk = ac(high_risk_raw);
        let medium_risk_raw = &[
            "api", "external", "network", "download", "upload", "install", "booking", "book ",
        ];
        let medium_risk = ac(medium_risk_raw);

        // ── State / retry ──
        let state_raw = &[
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
        let state_indicators = ac(state_raw);
        let retry_raw = &[
            "retry",
            "try again",
            "if it fails",
            "fallback",
            "keep trying",
            "until it works",
        ];
        let retry_indicators = ac(retry_raw);

        // ── Multi-agent trigger groups ──
        let agent_task_raw = &[
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
        ];
        let agent_task_triggers = ac(agent_task_raw);

        let agent_finance_raw = &[
            "transaction",
            "budget",
            "expense",
            "income",
            "balance",
            "account",
            "finance",
            "spending",
            "investment",
        ];
        let agent_finance_triggers = ac(agent_finance_raw);

        let agent_automation_raw = &[
            "cron",
            "automate",
            "schedule a",
            "every day",
            "recurring",
            "remind",
            "reminder",
            "remind me",
        ];
        let agent_automation_triggers = ac(agent_automation_raw);

        let agent_communication_raw = &["send", "email", "message", "notify", "slack", "telegram"];
        let agent_communication_triggers = ac(agent_communication_raw);

        // ── Negation / hypothetical ──
        let negation_raw = &[
            "don't ",
            "dont ",
            "do not ",
            "never ",
            "stop ",
            "cancel ",
            "don\u{2019}t ",
            "won't ",
            "wont ",
            "will not ",
            "can't ",
            "cannot ",
            "shouldn't ",
            "should not ",
            "wouldn't ",
            "would not ",
        ];
        let negation = ac(negation_raw);

        let hypothetical_raw = &[
            "what if ",
            "what about ",
            "would it be ",
            "could i ",
            "should i ",
            "how about ",
            "imagine ",
            "suppose ",
        ];
        let hypothetical = ac(hypothetical_raw);

        AcMatchers {
            greeting_starts,
            greeting_exact,
            task_mgmt,
            finance_patterns,
            notes_patterns,
            automation_patterns,
            task_nouns,
            task_verbs,
            finance_nouns,
            finance_verbs,
            notes_nouns,
            notes_verbs,
            direct_question,
            action_keywords,
            complex_workflow,
            tool_indicators,
            domain_verbs,
            sequential,
            high_risk,
            medium_risk,
            state_indicators,
            retry_indicators,
            agent_task_triggers,
            agent_finance_triggers,
            agent_automation_triggers,
            agent_communication_triggers,
            negation,
            hypothetical,
        }
    })
}

// ===========================================================================
// Layer 1: Heuristic classification
// ===========================================================================

/// Attempt to classify a message using keyword/pattern heuristics.
///
/// Returns `Some(IntentAnalysis)` for clear-cut intents, `None` when ambiguous
/// (should fall through to LLM classifier).
pub fn analyze_heuristic(message: &str) -> Option<IntentAnalysis> {
    let msg = message.trim().to_lowercase();
    let word_count = msg.split_whitespace().count();
    let m = matchers();

    // 0. Negation / hypothetical detection.
    //    If the message is framed as negation or hypothetical AND contains domain
    //    keywords, defer to LLM — heuristics can't parse "don't create a task"
    //    vs "don't forget to create a task" correctly.
    let has_negation = m.negation.is_match(&msg);
    let has_hypothetical = m.hypothetical.is_match(&msg);

    // 1. Greeting pattern — fast path, zero complexity.
    //    Negation doesn't apply to greetings ("don't say hi" is nonsensical).
    if is_greeting(&msg, m) {
        return Some(simple_analysis("Greeting detected", 0.95));
    }

    // Hypothetical + domain content → scenario reasoning (Reactive mode with flag).
    // This lets the runtime inject the scenario planning prompt.
    if has_hypothetical && has_any_domain_keyword(&msg, m) {
        let signals = ComplexitySignals {
            estimated_tool_calls: 3, // baseline + first-order + second-order
            has_sequential_deps: true,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
            has_hypothetical: true,
        };
        return Some(heuristic_analysis(
            "Hypothetical scenario with domain context",
            0.80,
            signals,
        ));
    }

    // Negation with domain content still defers to LLM (can't parse "don't create" correctly).
    if has_negation && has_any_domain_keyword(&msg, m) {
        return None;
    }

    // 1b. Multi-agent request → defer to LLM for orchestration.
    if has_multi_agent_triggers(&msg, m) {
        return None;
    }

    // 2. Domain-specific patterns → Reactive
    let domain_match = if is_task_management(&msg, m) {
        Some("Task management operation")
    } else if is_finance_operation(&msg, m) {
        Some("Finance operation")
    } else if is_notes_operation(&msg, m) {
        Some("Notes operation")
    } else if is_automation_operation(&msg, m) {
        Some("Automation/scheduling operation")
    } else {
        None
    };
    if let Some(reasoning) = domain_match {
        let signals = ComplexitySignals {
            estimated_tool_calls: count_tool_indicators(&msg, m).max(1),
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
            has_hypothetical: false,
        };
        return Some(heuristic_analysis(reasoning, 0.90, signals));
    }

    // 3. Very short non-keyword messages → Simple
    if msg.len() < 20 && word_count <= 4 && !has_any_action_keyword(&msg, m) {
        return Some(simple_analysis("Short message, no action keywords", 0.85));
    }

    // 4. Direct response patterns (questions, explanations)
    if is_direct_question(&msg, m) {
        if has_action_keyword(&msg, m) {
            return None; // "what is the best way to implement X" → ambiguous
        }
        return Some(simple_analysis("Question/explanation pattern", 0.90));
    }

    // 5. Complex workflow keywords → Complex
    if has_complex_workflow_keyword(&msg, m) {
        let signals = analyze_complexity(&msg, m);
        return Some(heuristic_analysis(
            "Complex workflow language detected",
            0.85,
            signals,
        ));
    }

    // 6. Structural complexity analysis for remaining messages
    let signals = analyze_complexity(&msg, m);
    let score = signals.complexity_score();

    if signals.has_sequential_deps {
        return None;
    }

    if has_tool_keyword(&msg, m) && score <= 1 {
        return Some(heuristic_analysis(
            "Simple tool-assisted operation",
            0.85,
            signals,
        ));
    }

    if has_action_keyword(&msg, m) && score <= 2 {
        return Some(heuristic_analysis(
            "Code/action keyword detected",
            0.80,
            signals,
        ));
    }

    if score >= 4 {
        return Some(heuristic_analysis(
            "High complexity score from heuristic analysis",
            0.75,
            signals,
        ));
    }

    // Not enough signal — defer to embedding fallback / LLM
    None
}

// ---------------------------------------------------------------------------
// Heuristic helpers (AC-backed)
// ---------------------------------------------------------------------------

fn references_mcp_tools(message: &str, tool_names: &[&str]) -> bool {
    let msg = message.trim().to_lowercase();
    let server_names: HashSet<&str> = tool_names
        .iter()
        .filter_map(|name| mcp::sanitize::extract_server_name(name))
        .collect();

    if server_names.is_empty() {
        return false;
    }

    msg.contains("mcp") || server_names.iter().any(|name| msg.contains(name))
}

fn is_greeting(msg: &str, m: &AcMatchers) -> bool {
    if msg.len() >= 20 {
        return false;
    }
    // Exact match
    if m.greeting_exact.contains(msg) {
        return true;
    }
    // Prefix match: AC finds if any greeting prefix is at position 0
    for mat in m.greeting_starts.find_iter(msg) {
        if mat.start() == 0 {
            return true;
        }
    }
    false
}

fn is_task_management(msg: &str, m: &AcMatchers) -> bool {
    // Substring pattern match
    if m.task_mgmt.is_match(msg) {
        return true;
    }
    // Possessive check
    if msg.starts_with("my ") && words_contain_any(msg, &m.task_nouns) {
        return true;
    }
    // Combinatorial: verb + noun
    words_contain_any(msg, &m.task_verbs) && words_contain_any(msg, &m.task_nouns)
}

fn is_finance_operation(msg: &str, m: &AcMatchers) -> bool {
    if m.finance_patterns.is_match(msg) {
        return true;
    }
    let has_noun = words_contain_any(msg, &m.finance_nouns);
    if words_contain_any(msg, &m.finance_verbs) && has_noun {
        return true;
    }
    if msg.starts_with("my ") && has_noun {
        return true;
    }
    // Question-phrased queries with a finance noun ("how are we doing on spending?")
    if has_noun && msg.contains('?') {
        return true;
    }
    false
}

fn is_notes_operation(msg: &str, m: &AcMatchers) -> bool {
    if m.notes_patterns.is_match(msg) {
        return true;
    }
    let has_verb = words_contain_any(msg, &m.notes_verbs);
    let has_noun = words_contain_any(msg, &m.notes_nouns);
    if has_verb && has_noun {
        return true;
    }
    false
}

fn is_automation_operation(msg: &str, m: &AcMatchers) -> bool {
    m.automation_patterns.is_match(msg)
}

fn is_direct_question(msg: &str, m: &AcMatchers) -> bool {
    m.direct_question.is_match(msg)
}

fn has_tool_keyword(msg: &str, m: &AcMatchers) -> bool {
    count_tool_indicators(msg, m) > 0
}

fn has_action_keyword(msg: &str, m: &AcMatchers) -> bool {
    m.action_keywords.is_match(msg)
}

fn has_any_action_keyword(msg: &str, m: &AcMatchers) -> bool {
    has_tool_keyword(msg, m)
        || has_action_keyword(msg, m)
        || has_complex_workflow_keyword(msg, m)
        || has_domain_verb(msg, m)
}

/// Check if the message contains any domain-specific keyword (across all domains).
fn has_any_domain_keyword(msg: &str, m: &AcMatchers) -> bool {
    m.task_mgmt.is_match(msg)
        || m.finance_patterns.is_match(msg)
        || m.notes_patterns.is_match(msg)
        || m.automation_patterns.is_match(msg)
        || words_contain_any(msg, &m.task_nouns)
        || words_contain_any(msg, &m.finance_nouns)
        || words_contain_any(msg, &m.notes_nouns)
}

fn has_domain_verb(msg: &str, m: &AcMatchers) -> bool {
    msg.split_whitespace().any(|w| {
        m.domain_verbs
            .contains(w.trim_matches(|c: char| !c.is_alphabetic()))
    })
}

fn has_complex_workflow_keyword(msg: &str, m: &AcMatchers) -> bool {
    m.complex_workflow.is_match(msg)
}

/// Count indicators of tool usage. Uses word splitting for precision.
fn count_tool_indicators(msg: &str, m: &AcMatchers) -> u8 {
    let words: Vec<&str> = msg.split_whitespace().collect();
    let mut count: u8 = 0;
    for (i, word) in words.iter().enumerate() {
        let clean = word.trim_matches(|c: char| !c.is_alphabetic());
        if m.tool_indicators.contains(clean) {
            count = count.saturating_add(1);
        }
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

fn detect_sequential_language(msg: &str, m: &AcMatchers) -> bool {
    m.sequential.find_iter(msg).count() >= 2
}

fn has_multi_agent_triggers(msg: &str, m: &AcMatchers) -> bool {
    let mut matched = 0u8;
    if m.agent_task_triggers.is_match(msg) {
        matched += 1;
    }
    if m.agent_finance_triggers.is_match(msg) {
        matched += 1;
    }
    if m.agent_automation_triggers.is_match(msg) {
        matched += 1;
    }
    if m.agent_communication_triggers.is_match(msg) {
        matched += 1;
    }
    matched >= 2
}

fn assess_failure_risk(msg: &str, m: &AcMatchers) -> FailureRisk {
    if m.high_risk.is_match(msg) {
        return FailureRisk::High;
    }
    if m.medium_risk.is_match(msg) {
        return FailureRisk::Medium;
    }
    FailureRisk::Low
}

fn detect_state_requirements(msg: &str, m: &AcMatchers) -> bool {
    m.state_indicators.is_match(msg)
}

fn detect_retry_indicators(msg: &str, m: &AcMatchers) -> bool {
    m.retry_indicators.is_match(msg)
}

fn analyze_complexity(msg: &str, m: &AcMatchers) -> ComplexitySignals {
    ComplexitySignals {
        estimated_tool_calls: count_tool_indicators(msg, m),
        has_sequential_deps: detect_sequential_language(msg, m),
        failure_risk: assess_failure_risk(msg, m),
        requires_state_tracking: detect_state_requirements(msg, m),
        requires_retries: detect_retry_indicators(msg, m),
        has_hypothetical: false,
    }
}

/// Helper: check if any word in `msg` (after punctuation trim) is in the set.
fn words_contain_any(msg: &str, set: &HashSet<&str>) -> bool {
    msg.split_whitespace()
        .any(|w| set.contains(w.trim_matches(|c: char| !c.is_alphabetic())))
}

// ---------------------------------------------------------------------------
// Analysis constructors
// ---------------------------------------------------------------------------

pub fn compute_iteration_budget(signals: &ComplexitySignals) -> u32 {
    signals.iteration_budget()
}

fn simple_analysis(reasoning: &str, confidence: f32) -> IntentAnalysis {
    IntentAnalysis {
        complexity: ComplexityLevel::Simple,
        signals: ComplexitySignals {
            estimated_tool_calls: 0,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
            has_hypothetical: false,
        },
        confidence,
        source: AnalysisSource::Heuristic,
        reasoning: reasoning.to_string(),
        needs_orchestration: false,
    }
}

fn heuristic_analysis(
    reasoning: &str,
    confidence: f32,
    signals: ComplexitySignals,
) -> IntentAnalysis {
    let score = signals.complexity_score();
    IntentAnalysis {
        complexity: IntentAnalysis::complexity_from_score(score),
        signals,
        confidence,
        source: AnalysisSource::Heuristic,
        reasoning: reasoning.to_string(),
        needs_orchestration: false,
    }
}

// ===========================================================================
// Layer 2: Embedding heuristic fallback
// ===========================================================================

/// Intent categories for embedding-based classification.
#[derive(Debug, Clone, Copy, PartialEq)]
enum IntentCategory {
    TaskManagement,
    Finance,
    Notes,
    Automation,
    DirectQuestion,
    Greeting,
}

impl IntentCategory {
    fn exemplars(&self) -> &[&str] {
        match self {
            Self::TaskManagement => &[
                "create a task to review the PR",
                "show my pending tasks",
                "plan my day",
                "add a todo for groceries",
                "decompose this into subtasks",
                "what tasks are due today",
            ],
            Self::Finance => &[
                "log an expense for lunch",
                "show my budget summary",
                "track this month's spending",
                "add income from freelance work",
                "what's my account balance",
            ],
            Self::Notes => &[
                "take a note about the meeting",
                "show my recent notes",
                "jot down this idea",
                "create a memo about the design decision",
            ],
            Self::Automation => &[
                "remind me to call the dentist tomorrow",
                "set up a recurring task every monday",
                "schedule a daily standup reminder",
                "automate the weekly report",
            ],
            Self::DirectQuestion => &[
                "what is machine learning",
                "explain how rust lifetimes work",
                "tell me about the company history",
                "who invented the telephone",
            ],
            Self::Greeting => &[
                "hello how are you",
                "hey there",
                "good morning",
                "hi what's up",
            ],
        }
    }

    fn all() -> &'static [IntentCategory] {
        &[
            Self::TaskManagement,
            Self::Finance,
            Self::Notes,
            Self::Automation,
            Self::DirectQuestion,
            Self::Greeting,
        ]
    }

    fn to_analysis(self, confidence: f32) -> IntentAnalysis {
        match self {
            Self::Greeting | Self::DirectQuestion => IntentAnalysis {
                complexity: ComplexityLevel::Simple,
                signals: ComplexitySignals {
                    estimated_tool_calls: 0,
                    has_sequential_deps: false,
                    failure_risk: FailureRisk::Low,
                    requires_state_tracking: false,
                    requires_retries: false,
                    has_hypothetical: false,
                },
                confidence,
                source: AnalysisSource::Embedding,
                reasoning: format!("Embedding match: {:?}", self),
                needs_orchestration: false,
            },
            _ => {
                let signals = ComplexitySignals {
                    estimated_tool_calls: 1,
                    has_sequential_deps: false,
                    failure_risk: FailureRisk::Low,
                    requires_state_tracking: false,
                    requires_retries: false,
                    has_hypothetical: false,
                };
                let score = signals.complexity_score();
                IntentAnalysis {
                    complexity: IntentAnalysis::complexity_from_score(score),
                    signals,
                    confidence,
                    source: AnalysisSource::Embedding,
                    reasoning: format!("Embedding match: {:?}", self),
                    needs_orchestration: false,
                }
            }
        }
    }
}

/// Pre-computed centroid (mean embedding) for an intent category.
struct IntentCentroid {
    category: IntentCategory,
    centroid: Vec<f32>,
}

// Re-use canonical implementation from common crate.
fn cosine_similarity_f32(a: &[f32], b: &[f32]) -> f32 {
    common::helpers::cosine_similarity(a, b) as f32
}

fn mean_embedding(embeddings: &[Vec<f32>]) -> Vec<f32> {
    if embeddings.is_empty() {
        return vec![];
    }
    let dim = embeddings[0].len();
    let mut mean = vec![0.0f32; dim];
    for emb in embeddings {
        for (i, v) in emb.iter().enumerate() {
            if i < dim {
                mean[i] += v;
            }
        }
    }
    let n = embeddings.len() as f32;
    for v in &mut mean {
        *v /= n;
    }
    mean
}

// ===========================================================================
// 2-Layer analyzer (heuristic + embedding)
// ===========================================================================

/// Two-stage intent analyzer: heuristics → embedding fallback.
pub struct IntentAnalyzer {
    strategy_repo: Option<storage::StrategyRepo>,
    config: OrchestratorConfig,
    /// Optional text embedder for Layer 2 embedding fallback.
    embedder: Option<std::sync::Arc<dyn cognitive::TextEmbedder>>,
    /// Cached intent centroids (computed lazily on first embedding call).
    centroid_cache: Mutex<Option<Vec<IntentCentroid>>>,
    /// Cached adaptive threshold with TTL.
    threshold_cache: Mutex<Option<(Instant, f32)>>,
    /// Optional parameter overrides from the autotuner (static, set at construction).
    overrides: Option<common::TrialParams>,
    /// Live autotuner reference — reads champion params on every `analyze()` call.
    /// Takes precedence over `overrides` when present and active.
    autotuner: Option<std::sync::Arc<crate::autotuner::AutoTunerOrchestrator>>,
}

impl IntentAnalyzer {
    pub fn new(config: &OrchestratorConfig) -> Self {
        Self {
            strategy_repo: None,
            config: config.clone(),
            embedder: None,
            centroid_cache: Mutex::new(None),
            threshold_cache: Mutex::new(None),
            overrides: None,
            autotuner: None,
        }
    }

    pub fn with_strategy_repo(mut self, repo: storage::StrategyRepo) -> Self {
        self.strategy_repo = Some(repo);
        self
    }

    pub fn with_embedder(mut self, embedder: std::sync::Arc<dyn cognitive::TextEmbedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Set parameter overrides for this analyzer instance (static, set at construction).
    pub fn with_overrides(mut self, params: common::TrialParams) -> Self {
        self.overrides = Some(params);
        self
    }

    /// Set a live autotuner reference for per-message champion param reads.
    pub fn with_autotuner(
        mut self,
        orchestrator: std::sync::Arc<crate::autotuner::AutoTunerOrchestrator>,
    ) -> Self {
        self.autotuner = Some(orchestrator);
        self
    }

    /// Analyze a user message and return the complexity assessment.
    ///
    /// 2-layer cascade:
    /// 1. AC heuristics (0ms)
    /// 2. Embedding fallback (< 5ms if embedder available)
    ///
    /// If both layers are inconclusive, falls back to Moderate complexity.
    pub async fn analyze(&self, message: &str, tool_names: &[&str]) -> IntentAnalysis {
        let threshold = self.effective_heuristic_threshold().await;

        // Layer 1: Heuristics (0ms)
        let mut analysis = if let Some(analysis) = analyze_heuristic(message) {
            if analysis.confidence >= threshold {
                debug!(
                    complexity = ?analysis.complexity,
                    confidence = analysis.confidence,
                    threshold = threshold,
                    "Heuristic classification accepted"
                );
                analysis
            } else {
                debug!(
                    confidence = analysis.confidence,
                    threshold = threshold,
                    "Heuristic confidence below threshold, trying embedding fallback"
                );
                // Layer 2: Embedding fallback
                if let Some(emb_analysis) = self.analyze_with_embedding(message).await {
                    if emb_analysis.confidence >= threshold {
                        debug!(
                            complexity = ?emb_analysis.complexity,
                            confidence = emb_analysis.confidence,
                            "Embedding fallback accepted"
                        );
                        emb_analysis
                    } else {
                        // Both layers inconclusive — use best partial or fallback
                        emb_analysis
                    }
                } else {
                    // Embedding unavailable — use heuristic partial
                    analysis
                }
            }
        } else {
            // Layer 2: Embedding fallback for heuristic misses
            if let Some(emb_analysis) = self.analyze_with_embedding(message).await {
                if emb_analysis.confidence >= threshold {
                    debug!(
                        complexity = ?emb_analysis.complexity,
                        confidence = emb_analysis.confidence,
                        "Embedding fallback classified ambiguous message"
                    );
                    emb_analysis
                } else {
                    emb_analysis
                }
            } else {
                // Both layers miss — use fallback (Moderate complexity)
                IntentAnalysis::fallback()
            }
        };

        // Post-classification: MCP override — bump to at least Moderate
        if matches!(analysis.complexity, ComplexityLevel::Simple)
            && references_mcp_tools(message, tool_names)
        {
            debug!("Overriding Simple → Moderate: message references MCP tools");
            analysis.complexity = ComplexityLevel::Moderate;
        }

        // Post-classification: multi-agent trigger force
        if !analysis.needs_orchestration {
            let msg_lower = message.trim().to_lowercase();
            let m = matchers();
            if has_multi_agent_triggers(&msg_lower, m) {
                debug!(
                    "Post-classification: forcing needs_orchestration from multi-agent triggers"
                );
                analysis.needs_orchestration = true;
                analysis.confidence = analysis.confidence.max(ORCHESTRATION_MIN_CONFIDENCE);
                if matches!(analysis.complexity, ComplexityLevel::Simple) {
                    analysis.complexity = ComplexityLevel::Moderate;
                }
            }
        }

        analysis
    }

    // ── Layer 2: Embedding fallback ──

    async fn analyze_with_embedding(&self, message: &str) -> Option<IntentAnalysis> {
        let embedder = self.embedder.as_ref()?;

        // Embed the message
        let msg_embedding = match embedder.embed(message).await {
            Ok(e) if !e.is_empty() => e,
            _ => return None,
        };

        // Ensure centroids are computed
        let mut cache = self.centroid_cache.lock().await;
        if cache.is_none() {
            let mut centroids = Vec::new();
            for category in IntentCategory::all() {
                let mut exemplar_embeddings = Vec::new();
                for exemplar in category.exemplars() {
                    if let Ok(emb) = embedder.embed(exemplar).await {
                        if !emb.is_empty() {
                            exemplar_embeddings.push(emb);
                        }
                    }
                }
                if !exemplar_embeddings.is_empty() {
                    centroids.push(IntentCentroid {
                        category: *category,
                        centroid: mean_embedding(&exemplar_embeddings),
                    });
                }
            }
            *cache = Some(centroids);
        }

        let centroids = cache.as_ref()?;
        if centroids.is_empty() {
            return None;
        }

        // Find best matching category
        let mut best_score = f32::NEG_INFINITY;
        let mut best_category = None;
        for centroid in centroids {
            let score = cosine_similarity_f32(&msg_embedding, &centroid.centroid);
            if score > best_score {
                best_score = score;
                best_category = Some(centroid.category);
            }
        }

        let category = best_category?;

        // Convert cosine similarity [0, 1] to confidence.
        // Empirically, cosine similarity > 0.75 is a strong match for fastembed.
        // Map [0.6, 0.95] → [0.6, 0.95] confidence with a slight boost for high similarity.
        let confidence = if best_score > 0.9 {
            0.92
        } else if best_score > 0.8 {
            0.80 + (best_score - 0.8) * 1.2
        } else if best_score > 0.7 {
            0.70 + (best_score - 0.7) * 1.0
        } else {
            best_score * 0.9
        };

        debug!(
            category = ?category,
            cosine_similarity = best_score,
            confidence = confidence,
            "Embedding fallback classification"
        );

        Some(category.to_analysis(confidence))
    }

    // ── Adaptive threshold ──

    async fn effective_heuristic_threshold(&self) -> f32 {
        // Live autotuner champion params take highest precedence.
        if let Some(ref orchestrator) = self.autotuner {
            if let Some(params) = orchestrator.current_champion_params().await {
                if let Some(threshold) = params.heuristic_confidence_threshold {
                    return threshold as f32;
                }
            }
        }

        // Static override from autotuner trial (set at construction, e.g. shadow mode).
        if let Some(ref overrides) = self.overrides {
            if let Some(threshold) = overrides.heuristic_confidence_threshold {
                return threshold as f32;
            }
        }

        let base = self.config.heuristic_confidence_threshold;

        let repo = match &self.strategy_repo {
            Some(r) => r,
            None => return base,
        };

        // Check TTL cache
        {
            let cache = self.threshold_cache.lock().await;
            if let Some((cached_at, value)) = *cache {
                if cached_at.elapsed() < Duration::from_secs(THRESHOLD_CACHE_TTL_SECS) {
                    return value;
                }
            }
        }

        // Compute adaptive threshold from historical accuracy.
        // If heuristic accuracy is high, we can lower the threshold (accept more).
        // If accuracy is low, raise the threshold (defer more to LLM).
        let since = chrono::Utc::now() - chrono::Duration::days(30);
        let threshold = match repo.get_accuracy("heuristic", since).await {
            Ok(Some(accuracy)) if accuracy > 0.0 => {
                // accuracy 0.95+ → threshold drops by 0.05 (accept more heuristic results)
                // accuracy 0.70  → threshold stays at base
                // accuracy 0.50  → threshold rises by 0.05 (defer more to LLM)
                let delta = (accuracy - 0.80) * 0.25; // ±0.05 for ±0.20 accuracy swing
                                                      // Cap at 0.90: domain heuristics return 0.90 confidence, so a threshold
                                                      // above 0.90 rejects ALL domain classifications and forces the LLM
                                                      // classifier, which can incorrectly trigger needs_orchestration.
                let adjusted = (base + delta).clamp(0.60, 0.90);
                debug!(
                    base_threshold = base,
                    historical_accuracy = accuracy,
                    adjusted_threshold = adjusted,
                    "Adaptive heuristic threshold"
                );
                adjusted
            }
            _ => base,
        };

        *self.threshold_cache.lock().await = Some((Instant::now(), threshold));
        threshold
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Heuristic tests ─────────────────────────────────────────

    #[test]
    fn greeting_is_simple() {
        let result = analyze_heuristic("hello");
        assert!(result.is_some());
        assert_eq!(result.unwrap().complexity, ComplexityLevel::Simple);
    }

    #[test]
    fn task_crud_is_recognized() {
        let result = analyze_heuristic("create a task to buy groceries");
        assert!(result.is_some());
        assert!(result.unwrap().signals.estimated_tool_calls > 0);
    }

    #[test]
    fn task_query_is_recognized() {
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
                result.is_some() && result.as_ref().unwrap().signals.estimated_tool_calls > 0,
                "Expected recognized domain command for '{}', got {:?}",
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
    fn simple_search_is_recognized_as_tool_use() {
        let result = analyze_heuristic("search for tasks about database migration");
        assert!(result.is_some());
        assert!(result.unwrap().signals.estimated_tool_calls > 0);
    }

    #[test]
    fn what_is_question_is_simple() {
        let result = analyze_heuristic("what is the capital of France?");
        assert!(result.is_some());
        assert_eq!(result.unwrap().complexity, ComplexityLevel::Simple);
    }

    #[test]
    fn greeting_variants() {
        for greeting in &["hi", "hey", "hello!", "good morning", "  hi  "] {
            let result = analyze_heuristic(greeting);
            assert!(
                result.is_some() && result.as_ref().unwrap().complexity == ComplexityLevel::Simple,
                "Expected Simple for '{}', got {:?}",
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
        assert!(analysis.signals.estimated_tool_calls > 0);
    }

    #[test]
    fn ambiguous_defers_to_embedding() {
        let result = analyze_heuristic("what is the best way to implement error handling in Rust?");
        assert!(
            result.is_none(),
            "Conflicting signals should defer to embedding"
        );
    }

    #[test]
    fn short_message_no_keywords_is_simple() {
        let result = analyze_heuristic("ok cool");
        assert!(result.is_some());
        assert_eq!(result.unwrap().complexity, ComplexityLevel::Simple);
    }

    #[test]
    fn complex_workflow_keyword_is_recognized() {
        let result = analyze_heuristic("create a plan for the database refactor");
        assert!(result.is_some());
        assert!(result.unwrap().signals.estimated_tool_calls > 0);
    }

    #[test]
    fn heuristic_source_is_always_heuristic() {
        let result = analyze_heuristic("hello").unwrap();
        assert_eq!(result.source, AnalysisSource::Heuristic);
    }

    #[test]
    fn signals_populated_for_non_simple() {
        let result = analyze_heuristic("search for tasks about database migration").unwrap();
        assert!(result.signals.estimated_tool_calls >= 1);
    }

    #[test]
    fn sequential_language_detection() {
        let m = matchers();
        assert!(detect_sequential_language(
            "first do X, then do Y, finally do Z",
            m
        ));
        assert!(!detect_sequential_language("just do this one thing", m));
    }

    #[test]
    fn failure_risk_assessment() {
        let m = matchers();
        assert_eq!(
            assess_failure_risk("deploy to production", m),
            FailureRisk::High
        );
        assert_eq!(
            assess_failure_risk("call the external api", m),
            FailureRisk::Medium
        );
        assert_eq!(assess_failure_risk("list my tasks", m), FailureRisk::Low);
    }

    // ── Analyzer tests ──────────────────────────────────────────

    #[tokio::test]
    async fn greeting_classified_as_simple() {
        let analyzer = IntentAnalyzer::new(&OrchestratorConfig::default());
        let result = analyzer.analyze("hello", &[]).await;
        assert_eq!(result.complexity, ComplexityLevel::Simple);
        assert_eq!(result.source, AnalysisSource::Heuristic);
    }

    #[tokio::test]
    async fn ambiguous_falls_back_to_moderate() {
        let analyzer = IntentAnalyzer::new(&OrchestratorConfig::default());
        // Ambiguous message — L1 heuristics return None, L2 has no embedder → fallback
        let result = analyzer
            .analyze("I need help with something complex", &[])
            .await;
        // Without embedder, falls back to Moderate (fallback)
        assert_eq!(result.complexity, ComplexityLevel::Moderate);
    }

    #[tokio::test]
    async fn task_crud_classified_by_heuristic() {
        let analyzer = IntentAnalyzer::new(&OrchestratorConfig::default());
        let result = analyzer
            .analyze("create a task to buy groceries", &[])
            .await;
        assert!(result.signals.estimated_tool_calls > 0);
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
            has_hypothetical: false,
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
            has_hypothetical: false,
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
            has_hypothetical: false,
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
            has_hypothetical: false,
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
    async fn analyzer_overrides_simple_to_moderate_for_mcp_requests() {
        let analyzer = IntentAnalyzer::new(&OrchestratorConfig::default());

        let mcp_tools = &["mcp_linear_list_issues", "mcp_linear_get_issue"];
        let result = analyzer.analyze("hello", mcp_tools).await;
        assert_eq!(result.complexity, ComplexityLevel::Simple);

        let result = analyzer
            .analyze("help me check my linear issues", mcp_tools)
            .await;
        assert_ne!(
            result.complexity,
            ComplexityLevel::Simple,
            "Expected non-Simple for MCP-referencing message, got {:?}",
            result.complexity
        );
    }

    #[test]
    fn multi_agent_triggers_defer() {
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
                "Expected heuristic to defer multi-agent message: {msg}"
            );
        }
    }

    #[test]
    fn single_agent_triggers_not_deferred() {
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

    // ── A-001: Short domain commands are recognized by heuristic ────
    // In the budget-bounded model, Simple doesn't prevent tool use — the execute
    // loop always provides tools and the model self-selects. These tests verify
    // that domain commands are *recognized* (Some) with estimated tool calls > 0.

    #[test]
    fn short_task_commands_are_recognized() {
        for cmd in &["my todos", "my todo", "plan my day", "list tasks"] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some() && result.as_ref().unwrap().signals.estimated_tool_calls > 0,
                "Expected recognized domain command for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    #[test]
    fn short_finance_commands_are_recognized() {
        for cmd in &[
            "log expense",
            "track spending",
            "show budget",
            "my budget",
            "log income",
        ] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some() && result.as_ref().unwrap().signals.estimated_tool_calls > 0,
                "Expected recognized domain command for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    #[test]
    fn short_notes_commands_are_recognized() {
        for cmd in &["my notes", "take notes", "note this"] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some() && result.as_ref().unwrap().signals.estimated_tool_calls > 0,
                "Expected recognized domain command for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    #[test]
    fn short_automation_commands_are_recognized() {
        for cmd in &["remind me", "schedule task"] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some() && result.as_ref().unwrap().signals.estimated_tool_calls > 0,
                "Expected recognized domain command for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    #[test]
    fn short_inert_messages_still_simple() {
        for cmd in &["ok cool", "thanks", "yes", "got it", "nice"] {
            let result = analyze_heuristic(cmd);
            assert!(
                result.is_some() && result.as_ref().unwrap().complexity == ComplexityLevel::Simple,
                "Expected Simple for '{}', got {:?}",
                cmd,
                result
            );
        }
    }

    // ── Negation handling tests ──

    #[test]
    fn negation_with_domain_defers() {
        let result = analyze_heuristic("don't create a task");
        assert!(result.is_none(), "Negated domain command should defer");

        let result = analyze_heuristic("don't forget the budget");
        assert!(result.is_none(), "Negated finance reference should defer");
    }

    #[test]
    fn negation_without_domain_still_classified() {
        let result = analyze_heuristic("don't worry");
        assert!(
            result.is_some(),
            "Negation without domain keywords should still classify"
        );
    }

    #[test]
    fn hypothetical_with_domain_routes_complex() {
        let result = analyze_heuristic("what if I create a task for this");
        assert!(
            result.is_some(),
            "Hypothetical + domain should return Some (scenario reasoning)"
        );
        let analysis = result.unwrap();
        assert_ne!(
            analysis.complexity,
            ComplexityLevel::Simple,
            "should not be Simple for scenario reasoning"
        );
        assert!(
            analysis.signals.has_hypothetical,
            "has_hypothetical signal should be set"
        );
    }

    #[test]
    fn no_negation_still_works() {
        let result = analyze_heuristic("create a task to buy groceries");
        assert!(result.is_some());
        assert!(result.unwrap().signals.estimated_tool_calls > 0);
    }

    // ── AC matchers initialization test ──

    #[test]
    fn ac_matchers_initialize_successfully() {
        let m = matchers();
        assert!(!m.greeting_exact.is_empty());
        assert!(m.task_mgmt.is_match("create a task"));
        assert!(m.finance_patterns.is_match("log expense"));
        assert!(m.notes_patterns.is_match("add note"));
        assert!(m.automation_patterns.is_match("remind me"));
        assert!(m.negation.is_match("don't do this"));
        assert!(m.hypothetical.is_match("what if we try"));
    }

    // ── Embedding helpers tests ──

    #[test]
    fn cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity_f32(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity_f32(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_empty_vectors() {
        assert_eq!(cosine_similarity_f32(&[], &[]), 0.0);
    }

    #[test]
    fn mean_embedding_computes_correctly() {
        let embeddings = vec![vec![1.0, 2.0, 3.0], vec![3.0, 4.0, 5.0]];
        let mean = mean_embedding(&embeddings);
        assert_eq!(mean.len(), 3);
        assert!((mean[0] - 2.0).abs() < 1e-6);
        assert!((mean[1] - 3.0).abs() < 1e-6);
        assert!((mean[2] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn mean_embedding_empty_returns_empty() {
        assert!(mean_embedding(&[]).is_empty());
    }

    // ── Hypothetical scenario routing tests ──

    #[test]
    fn test_what_if_budget_routes_not_simple() {
        let result = analyze_heuristic("what if I increase my budget by 500 per month");
        assert!(result.is_some());
        let analysis = result.unwrap();
        assert!(analysis.signals.has_hypothetical);
        assert_ne!(analysis.complexity, ComplexityLevel::Simple);
    }

    #[test]
    fn test_what_if_task_routes_not_simple() {
        let result =
            analyze_heuristic("what if I push the deadline back 2 weeks for the migration task");
        assert!(result.is_some());
        let analysis = result.unwrap();
        assert!(analysis.signals.has_hypothetical);
    }

    #[test]
    fn test_hypothetical_with_domain_keyword_returns_not_simple() {
        let result = analyze_heuristic("what if I deprioritize the API migration task");
        assert!(result.is_some(), "hypothetical + domain should return Some");
        let analysis = result.unwrap();
        assert_ne!(
            analysis.complexity,
            ComplexityLevel::Simple,
            "should not be Simple"
        );
        assert!(
            analysis.signals.has_hypothetical,
            "has_hypothetical signal should be set"
        );
    }

    #[test]
    fn test_simple_hypothetical_without_domain_defers() {
        let result = analyze_heuristic("what if it rains tomorrow");
        assert!(result.is_none(), "hypothetical without domain should defer");
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
}
