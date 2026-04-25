//! `CodeState` — coarse-grained coding session state (Tier B3).
//!
//! Captured by the context-engine rewriter and stored on `UserSituationSnapshot`
//! as a lightweight string snapshot. This keeps L3 (context engine) free of L5
//! (coding memory) dependencies while still giving the Reforge phase signal
//! about what the user is currently experiencing.

use serde::{Deserialize, Serialize};

/// High-level state of the user's coding session.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodeState {
    /// No active coding session or idle.
    Idle,
    /// User is currently looking at a stack trace.
    StackTraceActive {
        /// The type of error (e.g. "NullPointerException", "TypeError").
        error_type: String,
    },
    /// Tests are running and some are failing.
    RedTestsRunning,
    /// Tests are running and all passing.
    GreenTestsRunning,
    /// Build is failing.
    BuildFailing,
    /// User is in the middle of a refactor.
    Refactoring,
    /// Debugger is active.
    Debugging,
}

/// String snapshot stored on `UserSituationSnapshot` to avoid L5 deps in L3.
pub type CodeStateSnapshot = String;

impl CodeState {
    /// Serialize to a snapshot string.
    #[must_use]
    pub fn to_snapshot(&self) -> CodeStateSnapshot {
        serde_json::to_string(self).unwrap_or_else(|_| "idle".into())
    }

    /// Deserialize from a snapshot string.
    pub fn from_snapshot(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }
}
