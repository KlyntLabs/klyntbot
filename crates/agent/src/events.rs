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

    /// Pipeline classification step completed.
    ClassificationComplete {
        strategy: String,
        confidence: f32,
        source: String,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// Context assembly step completed.
    ContextAssembled {
        #[serde(rename = "totalTokens")]
        total_tokens: usize,
        budget: usize,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
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
    },
}
