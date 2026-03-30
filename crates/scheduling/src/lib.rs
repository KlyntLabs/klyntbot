//! Klyntbot Cron - Cron job scheduling
//!
//! This crate provides cron job scheduling and management.

pub mod deadline;
pub mod deadline_actions;
pub mod error;
pub mod service;
pub mod types;

pub use deadline::{DeadlineHandler, DeadlineScheduler};
pub use deadline_actions::DeadlineAction;
pub use error::CronError;
pub use service::{
    classify_missed_job, evaluate_trigger, CronService, JobCallback, MissedJobClass,
    PresenceSnapshot,
};
pub use types::{
    CronJob, CronJobState, CronOrigin, CronPayload, CronSchedule, CronServiceStatus, CronStore,
};
