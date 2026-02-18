//! Klyntbot CLI - CLI commands and REPL
//!
//! This crate provides the command-line interface and interactive REPL.

pub mod chat;
pub mod commands;
pub mod interactive;
pub mod serve;
pub mod status;
pub mod wizard;

// Re-export commonly used items for convenience
pub use commands::*;
pub use wizard::*;
