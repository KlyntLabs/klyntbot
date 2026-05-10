//! Klynt tracing provider — reads Klynt's coding-mode sessions from
//! `sessions` + `session_messages` directly (in-process, SQLite-backed).
//!
//! Sibling of `kimi` (also SQLite-backed via `coding_ingest_events`) and
//! `claude_code` (JSONL-backed). Follows the same `TracingProvider` trait.

mod context_loader;
mod discovery;
mod loader;
mod provider_impl;
mod state_loader;
mod stats;
mod subagent_loader;
mod summary;

pub use provider_impl::KlyntTracingProvider;
