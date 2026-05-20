//! Unified thread runtime — shared infrastructure for assistant and coding modes.
//!
//! Both modes implement `ThreadRuntime` and share:
//! - `ActiveTurns` — value-identity DashMap keyed by turn_id
//! - `StreamGuard` — guaranteed cleanup on drop
//! - `RuntimeMetrics` — TTFT, TTLT, tool count

use std::sync::Arc;

use dashmap::DashMap;
use desktop_shared::commands::{ChatMessageResponse, SessionContextInput};
use desktop_shared::errors::ApiError;

pub mod assistant;

/// Re-export the existing entry type under the unified name.
pub use crate::handlers::chat::ActiveStreamEntry as ActiveTurnEntry;

/// Generation counter — monotonically increasing per (thread_id).
pub type Generation = u32;

/// Identifies a single turn across both assistant and coding modes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TurnHandle {
    pub thread_id: String,
    pub turn_id: String,
    pub generation: Generation,
}

/// Shared map of active turns, keyed by turn_id.
pub type ActiveTurns = Arc<DashMap<String, ActiveTurnEntry>>;

/// Metrics collected during a turn.
#[derive(Debug, Clone, Default)]
pub struct RuntimeMetrics {
    /// Time-to-first-token (ms).
    pub ttft_ms: Option<u64>,
    /// Time-to-last-token (ms).
    pub ttlt_ms: Option<u64>,
    /// Number of tool calls executed.
    pub tool_count: u32,
}

/// Unified request to start a turn.
pub struct StartTurnRequest {
    pub thread_id: String,
    pub content: String,
    pub context: Option<SessionContextInput>,
    pub mode: Option<String>,
    pub model: Option<String>,
}

/// Unified outcome of starting a turn.
///
/// Mode-specific fields are `Option` — the caller knows which mode it requested
/// and unwraps the relevant field.
pub struct StartTurnOutcome {
    pub handle: TurnHandle,
    pub user_message: Option<ChatMessageResponse>,
    pub stream_info: Option<crate::handlers::chat::ChatStreamInfo>,
}

/// Thread runtime trait — implemented by both assistant and coding modes.
#[async_trait::async_trait]
pub trait ThreadRuntime: Send + Sync {
    /// Start a new turn.
    async fn start_turn(&self, req: StartTurnRequest) -> Result<StartTurnOutcome, ApiError>;

    /// Cancel an active turn by its turn_id.
    async fn cancel_turn(&self, turn_id: &str) -> Result<(), ApiError>;

    /// Check if a turn is still active.
    fn is_active(&self, turn_id: &str) -> bool;

    /// Access the active turns map.
    fn active_turns(&self) -> &ActiveTurns;
}

/// Value-identity stream guard — ensures `active_turns` and `pending_interactions`
/// are cleaned up even on panic or early return.
///
/// Only removes the entry if it still belongs to this guard (guard_id match).
pub struct StreamGuard {
    pub key: String,
    pub guard_id: u64,
    pub streams: ActiveTurns,
    pub pending: Arc<PendingInteractions>,
}

pub type PendingInteractions =
    dashmap::DashMap<String, (String, tokio::sync::oneshot::Sender<common::FormResponse>)>;

impl Drop for StreamGuard {
    fn drop(&mut self) {
        // Value-identity removal: only delete the entry if it still belongs to us.
        if let Some(entry) = self.streams.get(&self.key) {
            if entry.guard_id == self.guard_id {
                drop(entry); // release the read lock before write
                self.streams.remove(&self.key);
            }
        }
        self.pending.remove(&self.key);
    }
}

static STREAM_GUARD_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Generate the next unique guard id.
pub fn next_guard_id() -> u64 {
    STREAM_GUARD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
