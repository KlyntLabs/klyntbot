//! `coding-ingest` — transport + adapters that accept `AgentEvent` streams
//! from external coding CLIs and the native `klynt-cli` source.

#![deny(missing_docs)]

/// CLI adapter stubs — see Task 6.
pub mod adapters;
/// Per-framework coverage parsers (lcov, cobertura, tarpaulin, llvm-cov).
pub mod coverage;
/// Daemon stub — see Task 8.
pub mod daemon;
/// `desktop.lock` heartbeat helpers.
pub mod desktop_lock;
/// `AgentEvent` contract.
pub mod event;
/// Path-based privacy exclusion filter.
pub mod excludes;
/// Git invalidation handler trait.
pub mod git_invalidation;
/// Hook CLI entry point — shared by `klyntbot-hook` binary and desktop's `--hook` mode.
pub mod hook_cli;
/// `HookClient` — socket-first-else-buffer dispatcher.
pub mod hook_client;
/// Pending invalidations queue.
pub mod pending_invalidations;
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

pub use adapters::codex::CodexAdapter;
pub use adapters::git_post_commit::GitPostCommitAdapter;
// kimi-cli is poll-only — see adapters::kimi_cli::poller::KimiPoller (added in Task 7).
pub use adapters::opencode::poller::OpencodePoller;
pub use adapters::opencode::OpencodeAdapter;
pub use daemon::OpHandler;

pub use event::{
    AgentEvent, AgentEventV1, AgentSource, DiagnosticsDelta, EventKind, FileOp, SkillScore,
    SymbolRef, TestFailure, TokenUsage,
};
pub use scope::RepoScope;
