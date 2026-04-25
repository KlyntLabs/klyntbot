//! Turn boundary detection.
//!
//! The Distiller processes one turn at a time. A turn begins with an
//! `EventKind::UserPrompt` and ends when any of the following fires the
//! boundary:
//!
//! - `AssistantMsg { token_usage: Some(_), .. }` — provider-reported usage
//!   is the authoritative "turn done" signal.
//! - `SessionEnd { .. }` — session ended before an `AssistantMsg` arrived
//!   (user quit mid-turn, crash). Flush anyway.
//! - A subsequent `UserPrompt` with a different `turn_id` arrives — the
//!   prior turn must flush.
//! - `fire_idle_turns(timeout)` — sweeper called by the Distiller clock,
//!   flushes turns whose last event is older than `timeout`.

use coding_ingest::event::{AgentEvent, EventKind};
use std::collections::HashMap;
use std::time::Instant;

/// Identifies a turn pending distillation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRef {
    /// Session id.
    pub session_id: String,
    /// Turn id — `None` for out-of-turn events (e.g. `SessionEnd`).
    pub turn_id: Option<String>,
}

/// What happened after accepting an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnBoundary {
    /// No boundary — continue buffering.
    None,
    /// Boundary fired; caller should trigger `distill_turn`.
    Fire {
        /// Session id.
        session_id: String,
        /// Turn id of the flushed turn (may be `None` for SessionEnd flushes).
        turn_id: Option<String>,
    },
}

#[derive(Debug)]
struct TurnState {
    last_seen_at: Instant,
}

/// Detects turn boundaries as events stream in.
#[derive(Debug, Default)]
pub struct TurnBuffer {
    active: HashMap<(String, Option<String>), TurnState>,
}

impl TurnBuffer {
    /// Construct an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
        }
    }

    /// Accept an event and return whether a boundary fires.
    pub fn accept(&mut self, event: &AgentEvent) -> TurnBoundary {
        let AgentEvent::V1(v1) = event;
        let key = (v1.session_id.clone(), v1.turn_id.clone());

        match &v1.kind {
            EventKind::AssistantMsg {
                token_usage: Some(_),
                ..
            } => {
                self.active.remove(&key);
                TurnBoundary::Fire {
                    session_id: key.0,
                    turn_id: key.1,
                }
            }
            EventKind::SessionEnd { .. } => {
                // Flush every active turn for this session — caller iterates fires.
                // Convention: emit a Fire for the most recent turn; caller should
                // additionally call `fire_idle_turns(Duration::ZERO)` after SessionEnd
                // to sweep any remaining.
                let still_active: Vec<_> = self
                    .active
                    .keys()
                    .filter(|(s, _)| s == &v1.session_id)
                    .cloned()
                    .collect();
                for k in &still_active {
                    self.active.remove(k);
                }
                TurnBoundary::Fire {
                    session_id: v1.session_id.clone(),
                    turn_id: None,
                }
            }
            EventKind::UserPrompt { .. } => {
                // If any distinct prior turn exists for this session, flush the most recent.
                let prior: Option<(String, Option<String>)> = self
                    .active
                    .keys()
                    .find(|(s, t)| s == &v1.session_id && t != &v1.turn_id)
                    .cloned();
                self.active.insert(
                    key,
                    TurnState {
                        last_seen_at: Instant::now(),
                    },
                );
                match prior {
                    Some((s, t)) => {
                        self.active.remove(&(s.clone(), t.clone()));
                        TurnBoundary::Fire {
                            session_id: s,
                            turn_id: t,
                        }
                    }
                    None => TurnBoundary::None,
                }
            }
            _ => {
                self.active
                    .entry(key)
                    .and_modify(|st| st.last_seen_at = Instant::now())
                    .or_insert(TurnState {
                        last_seen_at: Instant::now(),
                    });
                TurnBoundary::None
            }
        }
    }

    /// Sweep — return every turn whose last event is older than `timeout`.
    /// Caller is expected to invoke `distill_turn` for each returned `TurnRef`.
    pub fn fire_idle_turns(&mut self, timeout: std::time::Duration) -> Vec<TurnRef> {
        let now = Instant::now();
        let mut out = Vec::new();
        let stale: Vec<_> = self
            .active
            .iter()
            .filter_map(|(k, st)| {
                if now.saturating_duration_since(st.last_seen_at) >= timeout {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();
        for k in stale {
            self.active.remove(&k);
            out.push(TurnRef {
                session_id: k.0,
                turn_id: k.1,
            });
        }
        out
    }

    /// Test-only helper: current active turn count.
    #[cfg(test)]
    pub fn active_len(&self) -> usize {
        self.active.len()
    }
}
