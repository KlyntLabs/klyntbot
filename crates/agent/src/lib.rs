//! Klyntbot Agent - Core agent orchestration
//!
//! This crate provides the AgentLoop and related agent functionality.

pub mod agent_loop;
pub mod calendar_reconcile;
pub mod calendar_sync_adapter;
pub mod chat;
pub mod confidence;
pub mod context;
pub mod conversation_embedding_handler;
pub mod cron_handler_adapter;
pub mod enrichment;
pub mod events;
pub mod execution;
pub mod finance_adapter;
pub mod goal_handler;
pub mod learning;
pub mod learning_handler;
pub mod memory;
pub mod notifications;
pub mod orchestrator;
pub mod output;
pub mod pipeline;
pub mod plan_completion_handler;
pub mod plan_executor;
pub mod plan_handler;
mod plan_runner;
pub mod recurring_tasks;
pub mod reminders;
pub mod skills;
pub mod subagent;

pub use agent_loop::{AgentLoop, StreamingHandle};
pub use calendar_reconcile::{reconcile_calendar_events, ReconcileAction, ReconcileReport};
pub use calendar_sync_adapter::CalendarSyncAdapter;
pub use confidence::{ConfidenceAssessment, ConfidenceEvaluator, DecisionAction, DecisionLogger};
pub use context::ContextBuilder;
pub use conversation_embedding_handler::ConversationEmbeddingHandlerImpl;
pub use cron_handler_adapter::CronHandlerAdapter;
pub use enrichment::EnrichmentEngine;
pub use events::AgentEvent;
pub use execution::{CycleOutcome, ExecutionCore, ExecutionParams, ToolExecutionResult};
pub use finance_adapter::FinanceHandlerImpl;
pub use goal_handler::GoalHandlerImpl;
pub use learning::{LearningService, OutcomeRecorder, OutcomeStore};
pub use learning_handler::LearningHandlerImpl;
pub use memory::MemoryStore;
pub use notifications::NotificationDispatcher;
pub use pipeline::{AgentPipeline, PipelineConfig, PipelineResult};
pub use plan_completion_handler::PlanCompletionHandlerImpl;
pub use plan_executor::{build_step_context, regenerate_from, run_step, StepExecutionResult};
pub use plan_handler::PlanHandlerImpl;
pub use recurring_tasks::RecurringTaskSpawner;
pub use reminders::{CalendarEvent, ReminderEngine};
pub use skills::SkillManager;
pub use subagent::SubagentManager;
