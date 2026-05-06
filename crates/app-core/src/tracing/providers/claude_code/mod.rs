//! Claude Code (`~/.claude/projects/`) TracingProvider implementation.

pub mod cache;
pub mod categorize;
pub mod discovery;
pub mod import;
pub mod loader;
mod provider_impl;
pub mod stats;
pub mod subagent_loader;
pub mod summary;

pub use provider_impl::ClaudeCodeTracingProvider;
