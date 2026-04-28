//! opencode adapter — SQLite WAL polling over opencode's local DB.
//!
//! Opt-in only. The daemon spawns an `OpencodePoller` task that diffs
//! the messages table every 500 ms.

pub mod normalize;
pub mod poller;
pub mod schema;

use super::IngestAdapter;
use crate::event::AgentEvent;
use common::Result;

/// Adapter for opencode SQLite polling.
///
/// The `parse` method is used when replaying captured rows;
/// real-time ingestion is driven by [`OpencodePoller`].
#[derive(Debug, Default, Clone, Copy)]
pub struct OpencodeAdapter;

impl IngestAdapter for OpencodeAdapter {
    fn source_name(&self) -> &'static str {
        "opencode"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        // Opencode is poll-only — there is no hook surface. The poller in
        // `daemon.rs` reads `message` + `part` rows directly and feeds them
        // through `normalize::message_to_events`.
        Ok(None)
    }
}
