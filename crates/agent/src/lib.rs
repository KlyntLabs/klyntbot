//! Klyntbot Agent - Core agent orchestration
//!
//! This crate provides the AgentLoop and related agent functionality.
//!
//! - `adapters/` — trait implementations bridging lower-layer contracts
//! - Core modules: agent_loop, agent_runtime, execution, etc.

// ── Grouped modules ─────────────────────────────────────────────────────────
pub mod adapters;
pub mod autotuner;
pub mod domain_searchers;
pub mod handlers;
pub mod services;

// ── Core modules ─────────────────────────────────────────────────────────────
pub mod agent_loop;
pub mod agent_profile;
pub mod agent_runtime;
pub mod confidence;
pub mod content_registry;
pub mod context_sources;
pub mod engines;
pub mod events;
#[cfg(test)]
mod events_tests;
pub mod execution;
pub mod learning;
#[cfg(test)]
mod notes_integration_tests;
pub mod output;
pub mod subagent;

// ── Skill loader re-export (moved into agent_profile/) ──────────────────────
pub use agent_profile::skill_loader;

// ── Module re-exports (backward-compatible paths) ────────────────────────────
pub use adapters::{
    agent_task, cognitive_embedder, cognitive_handlers, conversation_recall, cron, finance,
    learning as learning_handler, llm_summary, mirror_handlers, productivity, progress,
    task_embedding,
};
pub use services::{memory_maintenance, recurring_tasks, session_cleanup};

// ── Type re-exports ──────────────────────────────────────────────────────────
pub use agent_loop::{AgentLoop, StreamingHandle};
pub use agent_runtime::{AgentRuntime, RuntimeConfig, RuntimeResult};
pub use cognitive_embedder::TextEmbedderImpl;
pub use confidence::{ConfidenceAssessment, ConfidenceEvaluator, DecisionAction, DecisionLogger};
pub use conversation_recall::ConversationRecallHandlerImpl;
pub use cron::CronHandlerAdapter;
pub use events::AgentEvent;
pub use execution::{CycleOutcome, ExecutionCore, ExecutionParams, ToolExecutionResult};
pub use finance::FinanceHandlerImpl;
pub use learning::LearningService;
pub use learning_handler::LearningHandlerImpl;
pub use productivity::ProductivityHandlerImpl;
pub use progress::ProgressHandlerImpl;
pub use recurring_tasks::RecurringTaskSpawner;
pub use subagent::{SubagentManager, SubagentProfile};

/// Helper: wrap an error into `KlyntbotError::Provider(InvalidResponse(...))`.
pub(crate) fn provider_err(label: &str, e: impl std::fmt::Display) -> common::KlyntbotError {
    common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
        "{label}: {e}"
    )))
}
