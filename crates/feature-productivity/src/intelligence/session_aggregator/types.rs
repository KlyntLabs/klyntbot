use std::collections::HashMap;

use jiff::Timestamp;

use crate::types::IntelligenceSessionType;

/// Maximum sliding window size (240 ticks x 5s = 20 minutes).
pub(super) const MAX_WINDOW: usize = 240;

/// Number of ticks in the Building state before promoting to Active.
/// 180 ticks x 5s = 15 minutes of sustained activity.
pub(super) const BUILDING_THRESHOLD: usize = 180;

/// Minimum purity to promote from Building to Active.
pub(super) const BUILDING_PURITY_MIN: f64 = 0.75;

/// If Building state exceeds this many ticks without meeting the threshold, reset to Idle.
/// 240 ticks x 5s = 20 minutes.
pub(super) const BUILDING_TIMEOUT: usize = 240;

/// Purity below this in Active state triggers Ending.
pub(super) const ACTIVE_PURITY_DROP: f64 = 0.50;

/// Number of ticks in Ending state before actually ending.
/// 24 ticks x 5s = 2 minutes grace period.
pub(super) const ENDING_GRACE_TICKS: usize = 24;

/// A classified tick ready for the session FSM.
#[derive(Debug, Clone)]
pub struct ClassifiedTick {
    pub timestamp: Timestamp,
    pub app_name: String,
    pub category: String,
    pub session_type: IntelligenceSessionType,
    pub confidence: f64,
    pub is_idle: bool,
    pub is_context_switch: bool,
}

#[derive(Debug, Clone)]
pub(super) enum SessionState {
    Idle,
    Building {
        category: String,
        session_type: IntelligenceSessionType,
        since: Timestamp,
        tick_count: usize,
        matching_ticks: usize,
    },
    Active {
        session_id: String,
        session_type: IntelligenceSessionType,
        category: String,
        started_at: Timestamp,
        tick_count: usize,
        matching_ticks: usize,
        context_switches: i64,
        distraction_count: i64,
        app_counts: HashMap<String, u32>,
        last_app: String,
    },
    Ending {
        session_id: String,
        session_type: IntelligenceSessionType,
        category: String,
        started_at: Timestamp,
        ending_since: Timestamp,
        ending_ticks: usize,
        tick_count: usize,
        matching_ticks: usize,
        context_switches: i64,
        distraction_count: i64,
        app_counts: HashMap<String, u32>,
        last_app: String,
    },
}
