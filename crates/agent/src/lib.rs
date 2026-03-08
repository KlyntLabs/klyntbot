//! Klyntbot Agent - Core agent orchestration
//!
//! This crate provides the AgentLoop and related agent functionality.

pub mod agent_loop;
pub mod agent_profile;
pub mod agent_runtime;
pub mod agent_task_handler;

pub mod cognitive_embedder;
pub mod cognitive_handlers;
pub mod confidence;
pub mod context_sources;
pub mod conversation_recall_handler;
pub mod cron_handler_adapter;
pub mod enrichment;
pub mod events;
#[cfg(test)]
mod events_tests;
pub mod execution;
pub mod finance_adapter;
pub mod intent_pipeline;
pub mod learning;
pub mod learning_handler;
pub mod llm_summary_provider;
pub mod memory_maintenance_service;
pub mod notifications;
pub mod output;
pub mod persona;
pub mod productivity_handler;
pub mod progress_handler;
pub mod recurring_tasks;
pub mod reminders;
pub mod session_cleanup_service;
pub mod subagent;
pub mod todo_embedding_handler;

pub use agent_loop::{AgentLoop, StreamingHandle};
pub use agent_runtime::{AgentRuntime, RuntimeResult};
pub use confidence::{ConfidenceAssessment, ConfidenceEvaluator, DecisionAction, DecisionLogger};
pub use context_sources::ConfidenceSource;
pub use cognitive_embedder::TextEmbedderImpl;
pub use conversation_recall_handler::ConversationRecallHandlerImpl;
pub use cron_handler_adapter::CronHandlerAdapter;
pub use enrichment::EnrichmentEngine;
pub use events::AgentEvent;
pub use execution::{CycleOutcome, ExecutionCore, ExecutionParams, ToolExecutionResult};
pub use finance_adapter::FinanceHandlerImpl;
pub use learning::LearningService;
pub use learning_handler::LearningHandlerImpl;
pub use notifications::NotificationDispatcher;
pub use persona::{PersonaChain, PersonaManager, PersonaScope};
pub use productivity_handler::ProductivityHandlerImpl;
pub use progress_handler::ProgressHandlerImpl;
pub use recurring_tasks::RecurringTaskSpawner;
pub use reminders::ReminderEngine;
pub use subagent::{SubagentManager, SubagentProfile};
