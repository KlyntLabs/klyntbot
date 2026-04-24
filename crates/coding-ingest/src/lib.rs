//! `coding-ingest` — transport + adapters that accept `AgentEvent` streams
//! from external coding CLIs and the native `klynt-cli` source.

#![deny(missing_docs)]

/// CLI adapter stubs — see Task 6.
pub mod adapters;
/// Daemon stub — see Task 8.
pub mod daemon;
/// `AgentEvent` contract.
pub mod event;
/// `RepoScope` — repo identity attached to events.
pub mod scope;
/// Transport stubs — see Task 7.
pub mod transport;
/// `ingest_event_log` persistence.
pub mod store;
/// Touch-file rate-limited stderr warnings.
pub mod warn;
/// `HookClient` — socket-first-else-buffer dispatcher.
pub mod hook_client;
/// `desktop.lock` heartbeat helpers.
pub mod desktop_lock;
/// Path-based privacy exclusion filter.
pub mod excludes;
/// Cwd → `RepoScope` resolver (cached).
pub mod scope_resolver;

pub use event::{
    AgentEvent, AgentEventV1, AgentSource, DiagnosticsDelta, EventKind, FileOp,
    SkillScore, SymbolRef, TestFailure, TokenUsage,
};
pub use scope::RepoScope;
