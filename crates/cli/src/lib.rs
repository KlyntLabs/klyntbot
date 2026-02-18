//! Klyntbot CLI - CLI commands and REPL
//!
//! This crate provides the command-line interface and interactive REPL.

pub mod calendar;
pub mod channels;
pub mod chat;
pub mod commands;
pub mod config_cmd;
pub mod cron;
pub mod goal;
pub mod interactive;
pub mod learning_cmd;
pub mod plan;
pub mod project;
pub mod provider_cmd;
pub mod serve;
pub mod skills;
pub mod status;
pub mod todo;
pub mod usage_cmd;
pub mod wizard;

// Re-export commonly used items for convenience
pub use commands::*;
pub use wizard::*;
