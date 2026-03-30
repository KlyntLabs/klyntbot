//! Actions that the DeadlineScheduler can fire when a deadline is reached.

use serde::{Deserialize, Serialize};

/// An action to be executed when a scheduled deadline fires.
///
/// Each variant carries enough context for the handler to act without
/// additional DB lookups where possible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeadlineAction {
    /// A task deadline reminder — the task is due soon or now.
    TaskReminder {
        /// The task's unique identifier.
        task_id: String,
        /// Human-readable label shown in the notification.
        label: String,
    },

    /// An in-progress focus session is about to expire.
    FocusWarning {
        /// The task the focus session is tracking.
        task_id: String,
        /// Hours remaining before the focus session expires.
        hours_left: u32,
    },

    /// A focus session has expired; transition the task out of focus state.
    FocusExpire {
        /// The task whose focus session has expired.
        task_id: String,
    },

    /// Spawn a new recurring task instance from a template.
    SpawnRecurring {
        /// The recurring-task template to instantiate.
        template_id: String,
    },
}

impl DeadlineAction {
    /// Returns a stable string that uniquely identifies this action instance.
    ///
    /// Used as the key in the scheduler's deduplication map: scheduling the
    /// same action twice (same key) replaces the earlier entry rather than
    /// creating a duplicate.
    pub fn dedup_key(&self) -> String {
        match self {
            DeadlineAction::TaskReminder { task_id, .. } => {
                format!("task_reminder:{task_id}")
            }
            DeadlineAction::FocusWarning {
                task_id,
                hours_left,
            } => {
                format!("focus_warning:{task_id}:{hours_left}h")
            }
            DeadlineAction::FocusExpire { task_id } => {
                format!("focus_expire:{task_id}")
            }
            DeadlineAction::SpawnRecurring { template_id } => {
                format!("spawn_recurring:{template_id}")
            }
        }
    }
}
