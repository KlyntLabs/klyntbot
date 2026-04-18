//! Unified temporal scheduler: persistent, wall-clock-anchored, VALARM-style rules.
pub mod fire_store;
pub mod misfire;
pub mod rrule;
pub mod rules;
pub mod scheduler;

pub use fire_store::{FireSpec, FireStore};
pub use misfire::{Decision, MisfirePolicy};
pub use rrule::{evaluate_next_n, Frequency, RRuleSpec};
pub use scheduler::{SchedulerConfig, TemporalScheduler};
