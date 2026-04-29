//! kimi-cli adapter — poll-only ingestion driven by `KimiPoller`.
//!
//! kimi-cli writes per-session JSONL files at
//! `~/.kimi/sessions/<work_dir_hash>/<session_uuid>/wire.jsonl`. The poller
//! tails those files and emits `AgentEvent`s into the daemon's event channel.
//!
//! Legacy Tier-1 hook + Tier-2 Wire-socket adapters were removed in favor of
//! this single poll-only path, mirroring the Codex precedent.

/// `WireRecord` → `AgentEventV1` mapping.
pub mod mapper;
/// Poller that tails per-session `wire.jsonl` files.
pub mod poller;
/// Wire-file frame parser (`metadata` header + `WireRecord` lines).
pub mod wire_file;
/// Workspace `<hash> → cwd` resolver reading `~/.kimi/kimi.json`.
pub mod workdir;
