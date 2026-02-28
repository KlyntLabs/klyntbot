//! Row structs for `sqlx::FromRow` deserialization.

pub mod agent_task;
pub mod calendar;
pub mod cron;
pub mod finance;
pub mod goal;
pub mod learning;
pub mod memory;
pub mod plan;
pub mod project;
pub mod session;
pub mod todo;
pub mod usage;

#[cfg(test)]
mod serialization_tests;
