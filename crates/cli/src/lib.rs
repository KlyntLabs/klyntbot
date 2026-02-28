//! Klyntbot CLI - CLI commands
//!
//! This crate provides the command-line interface.

pub mod commands;
pub mod heartbeat;
pub mod plugin_cmd;
pub mod serve;
pub mod status;
pub mod wizard;

// Re-export commonly used items for convenience
pub use commands::*;
pub use wizard::*;
