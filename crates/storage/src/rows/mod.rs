//! Row structs for `sqlx::FromRow` deserialization.

pub mod action;
pub mod agent_task;
pub mod area;
pub mod calendar;
pub mod cron;
pub mod finance;
pub mod key_result;
pub mod learning;
pub mod memory;
pub mod objective;
pub mod project;
pub mod session;
pub mod session_context;
pub mod usage;

#[cfg(test)]
mod serialization_tests;
