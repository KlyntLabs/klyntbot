//! opencode adapter — SQLite WAL polling over opencode's local DB.
//!
//! Opt-in only. The daemon spawns an `OpencodePoller` task that diffs
//! the messages table every 500 ms.

pub mod normalize;
pub mod poller;
pub mod schema;

use super::IngestAdapter;
use crate::event::AgentEvent;
use common::{KlyntbotError, Result};

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

    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>> {
        // hook_event is the table row id or "unknown" when replaying.
        let _ = hook_event;
        let row: schema::MessageRow = match serde_json::from_slice(raw) {
            Ok(r) => r,
            // Opencode is poll-only; the hook surface is a no-op fallback.
            Err(_) => return Ok(None),
        };
        normalize::row_to_event(row).map(|o| o.map(AgentEvent::V1))
    }
}
