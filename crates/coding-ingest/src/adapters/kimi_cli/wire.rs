//! Deprecated tier-2 Wire socket stub.
//!
//! Kept ONLY so `crate::adapters::kimi_cli::spawn_wire` keeps resolving
//! while Task 9 swaps `daemon.rs` over to `KimiPoller`. Deleted in Task 9.7.

use crate::event::AgentEvent;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// No-op replacement — returns immediately. The legacy tier-2 socket
/// adapter is gone; the new poll-only path lives in `poller.rs`.
pub fn spawn_wire(
    _socket_path: PathBuf,
    _tx: mpsc::UnboundedSender<AgentEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {})
}
