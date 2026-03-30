//! Deadline action types — what fires when a deadline is reached.

use serde::{Deserialize, Serialize};

/// An action to execute when a deadline fires.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DeadlineAction {
    /// Send a reminder notification for a task approaching its due date.
    TaskReminder {
        task_id: String,
        /// Human-readable label, e.g. "2h before due"
        label: String,
    },

    /// Warn about an approaching focus deadline (6h, 3h, 1h thresholds).
    FocusWarning {
        task_id: String,
        hours_left: u32,
    },

    /// Auto-expire a focus session that has passed its deadline.
    FocusExpire {
        task_id: String,
    },

    /// Spawn a recurring task instance from a template.
    SpawnRecurring {
        template_id: String,
    },
}

impl DeadlineAction {
    /// Unique key for deduplication — prevents scheduling the same action twice.
    pub fn dedup_key(&self) -> String {
        match self {
            Self::TaskReminder { task_id, label } => format!("reminder:{task_id}:{label}"),
            Self::FocusWarning { task_id, hours_left } => {
                format!("focus_warn:{task_id}:{hours_left}h")
            }
            Self::FocusExpire { task_id } => format!("focus_expire:{task_id}"),
            Self::SpawnRecurring { template_id } => format!("spawn:{template_id}"),
        }
    }
}
