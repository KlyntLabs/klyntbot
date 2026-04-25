//! `coding-ingest` — transport + adapters that accept `AgentEvent` streams
//! from external coding CLIs and the native `klynt-cli` source.

#![deny(missing_docs)]

/// CLI adapter stubs — see Task 6.
pub mod adapters;
/// Daemon stub — see Task 8.
pub mod daemon;
/// `desktop.lock` heartbeat helpers.
pub mod desktop_lock;
/// `AgentEvent` contract.
pub mod event;
/// Path-based privacy exclusion filter.
pub mod excludes;
/// `HookClient` — socket-first-else-buffer dispatcher.
pub mod hook_client;
/// Hook CLI entry point — shared by `klyntbot-hook` binary and desktop's `--hook` mode.
pub mod hook_cli;
/// `RepoScope` — repo identity attached to events.
pub mod scope;
/// Cwd → `RepoScope` resolver (cached).
pub mod scope_resolver;
/// `ingest_event_log` persistence.
pub mod store;
/// Transport stubs — see Task 7.
pub mod transport;
/// Touch-file rate-limited stderr warnings.
pub mod warn;

pub use daemon::OpHandler;

pub use event::{
    AgentEvent, AgentEventV1, AgentSource, DiagnosticsDelta, EventKind, FileOp, SkillScore,
    SymbolRef, TestFailure, TokenUsage,
};
pub use scope::RepoScope;
