//! `coding-ingest` — transport + adapters that accept `AgentEvent` streams
//! from external coding CLIs and the native `klynt-cli` source.
//!
//! Phase 1 lands the module surface; implementations follow in later tasks.

#![deny(missing_docs)]

/// CLI adapter stubs — see Task 6.
pub mod adapters;
/// Daemon stub — see Task 8.
pub mod daemon;
/// `AgentEvent` contract — see Task 3/4.
pub mod event;
/// `RepoScope` — see Task 3.
pub mod scope;
/// Transport stubs — see Task 7.
pub mod transport;
