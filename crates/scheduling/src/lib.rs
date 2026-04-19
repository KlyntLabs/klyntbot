//! Klyntbot Cron - Cron job scheduling
//!
//! This crate provides cron job scheduling and management.

pub mod deadline;
pub mod deadline_actions;
pub mod error;
pub mod service;
pub mod temporal;
pub mod types;

pub use deadline::{DeadlineHandler, DeadlineScheduler};
pub use deadline_actions::DeadlineAction;
pub use error::CronError;
// `service` module is kept for the `row_to_job` helper used by `CronExecutor` (same crate).
// CronService itself is removed — use `CronExecutor` + `TemporalScheduler` instead.
pub use service::row_to_job;
pub use temporal::cron_executor::{CronExecutor, CronHandler};
pub use temporal::fire_store::{FireSpec, FireStore};
pub use temporal::recurrence::{RecurrenceEngine, RecurrenceTemplate};
pub use temporal::rules::{AlarmRule, RuleError};
pub use types::{CronJob, CronJobState, CronOrigin, CronPayload, CronSchedule};
