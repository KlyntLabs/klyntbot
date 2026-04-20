//! feature-alarms: Standalone, free-floating alarm tool.
//!
//! Spec §8.2 / §8.4. Backed entirely by `scheduled_fires` rows with
//! `kind = "standalone_alarm"` — no dedicated table. The dedup_prefix
//! `standalone:{alarm_id}:` makes snooze + cancel one-line cancel-by-prefix
//! operations.
//!
//! Tasks-attached alarms live in `feature-tasks::alarms`; this crate is
//! exclusively for "remind me in 10 minutes about X" style invocations
//! that have no task or other domain entity to anchor them.

mod tool;

pub use tool::AlarmTool;
