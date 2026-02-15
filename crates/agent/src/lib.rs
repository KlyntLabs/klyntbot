//! Klyntbot Agent - Core agent orchestration
//!
//! This crate provides the AgentLoop and related agent functionality.

pub mod agent_loop;
pub mod calendar_reconcile;
pub mod calendar_sync_adapter;
pub mod confidence;
pub mod context;
pub mod cron_handler_adapter;
pub mod enrichment;
pub mod events;
pub mod memory;
pub mod notifications;
pub mod recurring_tasks;
pub mod reminders;
pub mod skills;
pub mod subagent;

pub use agent_loop::{AgentLoop, StreamingHandle};
pub use calendar_reconcile::{reconcile_calendar_events, ReconcileAction, ReconcileReport};
pub use calendar_sync_adapter::CalendarSyncAdapter;
pub use confidence::{ConfidenceAssessment, ConfidenceEvaluator, DecisionAction, DecisionLogger};
pub use context::ContextBuilder;
pub use cron_handler_adapter::CronHandlerAdapter;
pub use enrichment::EnrichmentEngine;
pub use events::AgentEvent;
pub use memory::MemoryStore;
pub use notifications::NotificationDispatcher;
pub use recurring_tasks::RecurringTaskSpawner;
pub use reminders::{CalendarEvent, ReminderEngine};
pub use skills::SkillManager;
pub use subagent::SubagentManager;
