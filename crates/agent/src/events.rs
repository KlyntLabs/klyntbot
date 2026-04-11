//! Agent event types for real-time streaming updates.
//!
//! These events are emitted by the agent loop during processing,
//! allowing consumers (like the CLI) to display real-time progress.

use serde::Serialize;

/// Events emitted by the agent loop during processing.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    /// A chunk of content streamed from the LLM.
    ContentChunk { data: String },

    /// Pipeline processing has begun (emitted immediately, before classification).
    PipelineStarted,

    /// A tool execution has started.
    ToolStart {
        name: String,
        args: serde_json::Value,
        /// Which agent initiated this tool (set during delegation).
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },

    /// A tool execution has completed.
    ToolEnd {
        name: String,
        success: bool,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
        /// Truncated result of the tool execution (max 2KB).
        result: Option<String>,
        /// Which agent initiated this tool (set during delegation).
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },

    /// A new agent iteration has started.
    IterationStart { iteration: usize, max: usize },

    /// Context assembly step completed.
    ContextAssembled {
        #[serde(rename = "totalTokens")]
        total_tokens: usize,
        budget: usize,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// Query enhancement pipeline completed — emits the trace for observability.
    RetrievalEnhanced {
        stages: Vec<context_engine::enhancement::StageTrace>,
        #[serde(rename = "totalLatencyMs")]
        total_latency_ms: u64,
        #[serde(rename = "totalLlmCalls")]
        total_llm_calls: u32,
    },

    /// Execution engine selected and starting.
    ExecutionStarted {
        engine: String,
        #[serde(rename = "maxIterations")]
        max_iterations: usize,
    },

    /// Processing is complete with the final accumulated content.
    Done {
        content: String,
        /// The persisted assistant message ID (for targeted metadata updates).
        #[serde(rename = "messageId", skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },

    /// Internal confidence assessment completed (not shown to user in CLI).
    ConfidenceAssessed { score: f32, action: String },

    /// An error occurred during processing.
    Error { message: String },

    /// An entity was created by a tool (task, project, area, etc.).
    EntityCreated(common::EntityCard),

    /// Token usage report after cost tracking.
    UsageReport {
        #[serde(rename = "promptTokens")]
        prompt_tokens: u32,
        #[serde(rename = "completionTokens")]
        completion_tokens: u32,
        #[serde(rename = "cacheReadTokens")]
        cache_read_tokens: u32,
        #[serde(rename = "cacheWriteTokens")]
        cache_write_tokens: u32,
        #[serde(rename = "estimatedCostUsd")]
        estimated_cost_usd: f64,
        model: String,
        #[serde(rename = "responseTimeMs")]
        response_time_ms: u64,
    },

    /// Monthly LLM cost budget warning (emitted at 80% and 100%).
    BudgetWarning {
        #[serde(rename = "monthlySpendUsd")]
        monthly_spend_usd: f64,
        #[serde(rename = "monthlyBudgetUsd")]
        monthly_budget_usd: f64,
        #[serde(rename = "usagePercent")]
        usage_percent: f64,
    },

    /// A memory search or operation was performed.
    MemoryAccess {
        action: String,
        query: Option<String>,
        #[serde(rename = "resultsCount")]
        results_count: u32,
    },

    /// A skill was loaded into the system prompt.
    SkillLoaded {
        name: String,
        trigger: String,
        /// Which agent this skill belongs to (set during delegation).
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },

    /// A learning event occurred (threshold adjustment, pattern detection).
    LearningEvent {
        #[serde(rename = "eventType")]
        event_type: String,
        detail: String,
    },

    /// An agent profile was selected to handle the current message.
    AgentSelected { name: String, description: String },

    /// A subagent was spawned.
    SubagentSpawned { label: String, profile: String },

    /// An agent delegation has started (agent-to-agent handoff).
    DelegationStarted {
        #[serde(rename = "fromAgent")]
        from_agent: String,
        #[serde(rename = "toAgent")]
        to_agent: String,
        query: String,
        depth: u32,
    },

    /// An agent delegation has completed.
    DelegationCompleted {
        #[serde(rename = "fromAgent")]
        from_agent: String,
        #[serde(rename = "toAgent")]
        to_agent: String,
        success: bool,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// An MCP server connection status changed during startup.
    McpServerStatus {
        #[serde(rename = "serverName")]
        server_name: String,
        /// One of: "starting", "ready", "failed", "skipped"
        status: String,
        /// Number of tools discovered (only for "ready" status).
        #[serde(rename = "toolCount", skip_serializing_if = "Option::is_none")]
        tool_count: Option<usize>,
        /// Error message (only for "failed" status).
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// All MCP server connections have completed.
    McpStartupComplete {
        ready: usize,
        failed: usize,
        skipped: usize,
    },

    /// Chain-of-thought planning has started for a complex task.
    PlanningStarted {
        #[serde(rename = "complexityScore")]
        complexity_score: u8,
    },

    /// A structured execution plan was generated.
    PlanGenerated {
        steps: Vec<String>,
        #[serde(rename = "rawPlan")]
        raw_plan: String,
    },

    /// A plan step was completed during execution.
    PlanStepCompleted {
        #[serde(rename = "stepIndex")]
        step_index: usize,
        description: String,
        #[serde(rename = "toolName")]
        tool_name: String,
    },

    /// A persona in a squad completed its analysis.
    PersonaPerspective {
        #[serde(rename = "personaId")]
        persona_id: String,
        #[serde(rename = "personaName")]
        persona_name: String,
        #[serde(rename = "personaIcon")]
        persona_icon: String,
        #[serde(rename = "personaRole")]
        persona_role: String,
        content: String,
        /// What the judge asked this persona to address (targeted phase only).
        #[serde(rename = "challenge", skip_serializing_if = "Option::is_none")]
        challenge: Option<String>,
    },

    /// A debate round started.
    DebateRoundStarted {
        round: u32,
        #[serde(rename = "totalRounds")]
        total_rounds: u32,
        /// "opening" | "discussion" | "targeted" | "final"
        phase: String,
    },

    /// Emitted after the judge evaluates each round.
    DebateJudgeDecision {
        round: u32,
        #[serde(rename = "consensusScore")]
        consensus_score: f64,
        decision: String,
        #[serde(rename = "speakingOrder")]
        speaking_order: Vec<String>,
        reasoning: String,
    },

    /// A debate round completed with all persona responses.
    DebateRoundCompleted {
        round: u32,
        #[serde(rename = "consensusScore")]
        consensus_score: f64,
    },

    /// Consensus was reached — debate terminates early.
    ConsensusReached {
        round: u32,
        #[serde(rename = "consensusScore")]
        consensus_score: f64,
        summary: String,
    },

    /// Live context was injected mid-execution (e.g., memory promoted during ReAct loop).
    ContextReassembled {
        updates: Vec<crate::execution::live_context_refresher::ContextReassembledUpdate>,
        #[serde(rename = "tokensAdded")]
        tokens_added: usize,
    },

    /// Mid-loop context compression was triggered during a reactive execution.
    ContextCompressed {
        #[serde(rename = "beforeTokens")]
        before_tokens: usize,
        #[serde(rename = "afterTokens")]
        after_tokens: usize,
        iteration: usize,
    },

    /// A memory was promoted from one scope to a higher scope.
    MemoryPromoted {
        #[serde(rename = "factId")]
        fact_id: String,
        #[serde(rename = "fromScope")]
        from_scope: String,
        #[serde(rename = "toScope")]
        to_scope: String,
        subject: String,
        predicate: String,
    },

    /// AutoTuner nightly cycle report.
    AutoTunerReport(::autotuner::AutoTunerReport),
    /// AutoTuner promoted a new champion.
    AutoTunerPromotion(::autotuner::AutoTunerPromotion),
    /// AutoTuner auto-reverted after regression.
    AutoTunerRollback(::autotuner::AutoTunerRollback),

    /// The agent detected a repeating tool call pattern (warning level).
    LoopDetected {
        iteration: usize,
        #[serde(rename = "toolsSummary")]
        tools_summary: String,
        suggestion: String,
    },

    /// The agent hit the hard-stop threshold for repeated tool calls.
    LoopHardStop {
        iteration: usize,
        #[serde(rename = "toolsSummary")]
        tools_summary: String,
    },

    // ── Budget HUD (Deep/Ultra mode) ─────────────────────────
    /// Emitted after each turn with current budget state.
    /// UI renders the live budget HUD from this.
    BudgetUpdate {
        tokens_remaining_pct: f32,
        turns_used: u32,
        max_turns: u32,
        cost_usd: f64,
        depth: String,
    },

    /// User extended the budget mid-conversation.
    BudgetExtended {
        additional_turns: u32,
        new_max_turns: u32,
    },

    // ── Depth suggestion (adaptive layer) ────────────────────
    /// Mirror/history suggests a different depth mode.
    DepthSuggestion { recommended: String, reason: String },

    // ── Enrichment progress (Phase 4) ────────────────────────
    /// Post-response enrichment started (Mirror, Coaching, NoteTree, FSRS).
    EnrichmentStarted { phase: String },

    /// Post-response enrichment completed.
    EnrichmentComplete { phase: String, summary: String },

    // ── Turn tracking ────────────────────────────────────────
    /// Emitted at the end of each execute-loop turn.
    TurnComplete {
        turn: u32,
        budget_remaining_pct: f32,
    },
}
