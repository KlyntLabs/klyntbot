use serde::{Deserialize, Serialize};

use crate::events::{EnhancementStagePayload, TransparencyData};

/// Generation counter — monotonically increasing per (session_key).
///
/// Used by the frontend to filter stale events: any event whose generation
/// is less than the current generation for that thread is silently ignored.
pub type Generation = u32;

/// Unified thread event v2 — replaces the 50+ stringly-typed `agent:*` events.
///
/// Every variant carries a `generation` so the frontend can distinguish
/// events belonging to the current turn from events delayed by a race.
///
/// The `Terminal` variant is guaranteed to fire on *every* exit path
/// (Done, Error, Cancelled) so consumers always have a single hook point
/// for cleanup.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "event", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ThreadEvent {
    // ── Streaming content ──────────────────────────────────────────────
    ContentChunk {
        generation: Generation,
        session_key: String,
        data: String,
    },

    // ── Tool lifecycle ─────────────────────────────────────────────────
    ToolStart {
        generation: Generation,
        session_key: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },
    ToolEnd {
        generation: Generation,
        session_key: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        success: bool,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        estimated_tokens: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },

    // ─️ Entity & memory ────────────────────────────────────────────────
    EntityCreated {
        generation: Generation,
        session_key: String,
        entity_type: String,
        entity_id: String,
    },
    MemoryAccess {
        generation: Generation,
        session_key: String,
        action: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        results_count: u32,
    },
    MemoryPromoted {
        generation: Generation,
        session_key: String,
        fact_id: String,
        from_scope: String,
        to_scope: String,
        subject: String,
        predicate: String,
    },

    // ── Pipeline & execution ───────────────────────────────────────────
    PipelineStarted {
        generation: Generation,
        session_key: String,
    },
    ExecutionStarted {
        generation: Generation,
        session_key: String,
        engine: String,
        max_iterations: usize,
    },
    ContextAssembled {
        generation: Generation,
        session_key: String,
        total_tokens: usize,
        duration_ms: u64,
    },
    RetrievalEnhanced {
        generation: Generation,
        session_key: String,
        stages: Vec<EnhancementStagePayload>,
        total_latency_ms: u64,
        total_llm_calls: u32,
    },
    IterationStart {
        generation: Generation,
        session_key: String,
        iteration: usize,
        max_iterations: usize,
    },
    ClassificationComplete {
        generation: Generation,
        session_key: String,
        strategy: String,
        confidence: f32,
        source: String,
    },

    // ── Agent meta ─────────────────────────────────────────────────────
    AgentSelected {
        generation: Generation,
        session_key: String,
        name: String,
        description: String,
    },
    SkillLoaded {
        generation: Generation,
        session_key: String,
        name: String,
        trigger: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },
    LearningEvent {
        generation: Generation,
        session_key: String,
        event_type: String,
        detail: String,
    },
    SubagentSpawned {
        generation: Generation,
        session_key: String,
        label: String,
        profile: String,
    },
    DelegationStarted {
        generation: Generation,
        session_key: String,
        from_agent: String,
        to_agent: String,
        query: String,
        depth: u32,
    },
    DelegationCompleted {
        generation: Generation,
        session_key: String,
        from_agent: String,
        to_agent: String,
        success: bool,
        duration_ms: u64,
    },

    // ── Plan ───────────────────────────────────────────────────────────
    PlanGenerated {
        generation: Generation,
        session_key: String,
        steps: Vec<String>,
        raw_plan: String,
    },
    PlanStepCompleted {
        generation: Generation,
        session_key: String,
        step_index: usize,
        description: String,
        tool_name: String,
    },

    // ── Usage & cost ───────────────────────────────────────────────────
    UsageReport {
        generation: Generation,
        session_key: String,
        prompt_tokens: u32,
        completion_tokens: u32,
        cache_read_tokens: u32,
        cache_write_tokens: u32,
        estimated_cost_usd: f64,
        model: String,
        response_time_ms: u64,
    },
    ConfidenceAssessed {
        generation: Generation,
        session_key: String,
        score: f32,
        action: String,
    },
    BudgetWarning {
        generation: Generation,
        session_key: String,
        monthly_spend_usd: f64,
        monthly_budget_usd: f64,
        usage_percent: f64,
    },

    // ── Interaction ────────────────────────────────────────────────────
    InteractionRequest {
        generation: Generation,
        session_key: String,
        request_id: String,
        #[specta(type = crate::specta_helpers::JsonValue)]
        request: common::InteractionRequest,
    },

    // ── Heartbeat — emitted every 30s during active turns ─────────────
    Heartbeat {
        generation: Generation,
        session_key: String,
        server_time: i64,
    },

    // ── Terminal — guaranteed on every exit path ───────────────────────
    Terminal {
        generation: Generation,
        session_key: String,
        kind: TerminalKind,
        /// Transparency data accumulated during the turn.
        #[serde(skip_serializing_if = "Option::is_none")]
        transparency: Option<TransparencyData>,
    },
}

/// Terminal sub-variants — one of these is always present when a turn ends.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminalKind {
    Done {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },
    Error {
        message: String,
    },
    Cancelled {
        partial_content: String,
        partial_reasoning: String,
    },
}

impl ThreadEvent {
    /// The session key this event belongs to.
    pub fn session_key(&self) -> &str {
        match self {
            ThreadEvent::ContentChunk { session_key, .. } => session_key,
            ThreadEvent::ToolStart { session_key, .. } => session_key,
            ThreadEvent::ToolEnd { session_key, .. } => session_key,
            ThreadEvent::EntityCreated { session_key, .. } => session_key,
            ThreadEvent::MemoryAccess { session_key, .. } => session_key,
            ThreadEvent::MemoryPromoted { session_key, .. } => session_key,
            ThreadEvent::PipelineStarted { session_key, .. } => session_key,
            ThreadEvent::ExecutionStarted { session_key, .. } => session_key,
            ThreadEvent::ContextAssembled { session_key, .. } => session_key,
            ThreadEvent::RetrievalEnhanced { session_key, .. } => session_key,
            ThreadEvent::IterationStart { session_key, .. } => session_key,
            ThreadEvent::ClassificationComplete { session_key, .. } => session_key,
            ThreadEvent::AgentSelected { session_key, .. } => session_key,
            ThreadEvent::SkillLoaded { session_key, .. } => session_key,
            ThreadEvent::LearningEvent { session_key, .. } => session_key,
            ThreadEvent::SubagentSpawned { session_key, .. } => session_key,
            ThreadEvent::DelegationStarted { session_key, .. } => session_key,
            ThreadEvent::DelegationCompleted { session_key, .. } => session_key,
            ThreadEvent::PlanGenerated { session_key, .. } => session_key,
            ThreadEvent::PlanStepCompleted { session_key, .. } => session_key,
            ThreadEvent::UsageReport { session_key, .. } => session_key,
            ThreadEvent::ConfidenceAssessed { session_key, .. } => session_key,
            ThreadEvent::BudgetWarning { session_key, .. } => session_key,
            ThreadEvent::InteractionRequest { session_key, .. } => session_key,
            ThreadEvent::Heartbeat { session_key, .. } => session_key,
            ThreadEvent::Terminal { session_key, .. } => session_key,
        }
    }

    /// The generation this event belongs to.
    pub fn generation(&self) -> Generation {
        match self {
            ThreadEvent::ContentChunk { generation, .. } => *generation,
            ThreadEvent::ToolStart { generation, .. } => *generation,
            ThreadEvent::ToolEnd { generation, .. } => *generation,
            ThreadEvent::EntityCreated { generation, .. } => *generation,
            ThreadEvent::MemoryAccess { generation, .. } => *generation,
            ThreadEvent::MemoryPromoted { generation, .. } => *generation,
            ThreadEvent::PipelineStarted { generation, .. } => *generation,
            ThreadEvent::ExecutionStarted { generation, .. } => *generation,
            ThreadEvent::ContextAssembled { generation, .. } => *generation,
            ThreadEvent::RetrievalEnhanced { generation, .. } => *generation,
            ThreadEvent::IterationStart { generation, .. } => *generation,
            ThreadEvent::ClassificationComplete { generation, .. } => *generation,
            ThreadEvent::AgentSelected { generation, .. } => *generation,
            ThreadEvent::SkillLoaded { generation, .. } => *generation,
            ThreadEvent::LearningEvent { generation, .. } => *generation,
            ThreadEvent::SubagentSpawned { generation, .. } => *generation,
            ThreadEvent::DelegationStarted { generation, .. } => *generation,
            ThreadEvent::DelegationCompleted { generation, .. } => *generation,
            ThreadEvent::PlanGenerated { generation, .. } => *generation,
            ThreadEvent::PlanStepCompleted { generation, .. } => *generation,
            ThreadEvent::UsageReport { generation, .. } => *generation,
            ThreadEvent::ConfidenceAssessed { generation, .. } => *generation,
            ThreadEvent::BudgetWarning { generation, .. } => *generation,
            ThreadEvent::InteractionRequest { generation, .. } => *generation,
            ThreadEvent::Heartbeat { generation, .. } => *generation,
            ThreadEvent::Terminal { generation, .. } => *generation,
        }
    }

    /// True if this is a terminal event (turn is done/error/cancelled).
    pub fn is_terminal(&self) -> bool {
        matches!(self, ThreadEvent::Terminal { .. })
    }
}

impl tauri_specta::Event for ThreadEvent {
    const NAME: &'static str = "thread:event";
}
