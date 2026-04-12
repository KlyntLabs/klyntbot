use crate::budget::BudgetReport;
use crate::inventory::ContextInventory;
use providers::Message;

/// Default number of memory entries to retrieve.
pub(crate) const DEFAULT_MEMORY_RETRIEVAL_LIMIT: usize = 5;

/// Determines how the agent should process a request.
#[derive(Debug, Clone)]
pub enum ExecutionStrategy {
    /// Simple question/answer — no tool use needed.
    DirectResponse,
    /// May use tools up to `max_iterations` rounds.
    ToolAssisted { max_iterations: u32 },
    /// Full autonomous multi-step execution.
    AutonomousTask { max_iterations: u32 },
    /// Need more info from the user before proceeding.
    Clarification { reason: String },
}

/// Input to the context assembly pipeline.
pub struct ContextRequest {
    /// The user's message text (used for embedding-based memory lookup).
    pub message_text: String,
    /// Full conversation history.
    pub history: Vec<Message>,
    /// System prompt to prepend.
    pub system_prompt: String,
    /// Chosen execution strategy (affects budget allocation).
    pub strategy: ExecutionStrategy,
    /// Tool definitions as JSON schemas.
    pub tool_definitions: Vec<serde_json::Value>,
    /// Model context window size (varies per model).
    pub context_window: usize,
    /// Optional session key for per-session circuit-breaker tracking in InsightForge.
    pub session_key: Option<String>,
    /// Contextual signals for query enhancement (active skill, task, situation, etc.)
    pub retrieval_context: Option<crate::rewriter::RetrievalContext>,
    /// Enhancement budget derived from depth mode (defaults to Normal).
    pub enhancement_budget: crate::enhancement::EnhancementBudget,
    /// Number of recent turns to keep verbatim (from DepthMode).
    /// None = use the engine's compression config default.
    pub tier0_count: Option<usize>,
}

/// The assembled context ready to send to the LLM.
#[derive(Clone)]
pub struct AssembledContext {
    /// Ordered messages: system, memories, summaries, recent history.
    pub messages: Vec<Message>,
    /// Estimated total token count.
    pub token_count: usize,
    /// Budget allocation report.
    pub budget_report: BudgetReport,
    /// Inventory of loaded vs. deferred context sources.
    pub inventory: ContextInventory,
    /// Remaining token budget available for expansion.
    pub budget_remaining: usize,
    /// Context version — incremented on each expand() call.
    pub version: u32,
    /// Number of memory entries retrieved from the memory retriever/InsightForge.
    /// Used by the autotuner to compute the `memory_relevance` metric.
    pub retrieved_memory_count: usize,
    /// Enhancement pipeline trace (None if pipeline not configured).
    pub enhancement_trace: Option<crate::enhancement::EnhancementTrace>,
}
