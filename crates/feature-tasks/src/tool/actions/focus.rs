//! Focus, unfocus, and log_time action handlers.

use common::{Result, ToolError};
use jiff::{SignedDuration, Timestamp};
use tools_core::ParamExtractor;

use super::super::TaskTool;

impl TaskTool {
    pub(crate) async fn handle_focus(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let deadline =
            Some(Timestamp::now() + SignedDuration::from_hours(self.focus_deadline_hours as i64));

        // Accept optional energy_level for the focus session
        let energy_level = p.optional_str("energy_level")?;

        if self
            .repo
            .focus(id, self.max_focus_slots as i64, deadline)
            .await?
        {
            let te_rows = self.repo.list_time_entries(id).await?;
            let has_running = te_rows.iter().any(|e| e.ended_at.is_none());
            if !has_running {
                self.repo
                    .add_time_entry(id, "focus", Timestamp::now(), None, None, energy_level)
                    .await?;
            }

            // Log activity
            if self.config.auto_log_activity {
                let _ = self
                    .repo
                    .log_activity(id, "focus_started", None, None, None, "user", None)
                    .await;
            }

            // Notify focus_alarms subscriber to materialize 6h/3h/1h warnings + expire.
            if let Some(ref bus) = self.domain_bus {
                bus.publish(bus::DomainEvent::TaskFocusChanged {
                    task_id: id.to_string(),
                    focus_deadline: deadline.map(|d| d.to_string()),
                });
            }

            Ok(format!("Focused on task: {}", id))
        } else {
            Err(ToolError::ExecutionFailed(format!(
                "Cannot focus task {} (not found, already focused, or max slots reached)",
                id
            ))
            .into())
        }
    }

    pub(crate) async fn handle_unfocus(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        // Close any open time entries before unfocusing
        let te_rows = self.repo.list_time_entries(id).await?;
        for entry in &te_rows {
            if entry.ended_at.is_none() {
                self.repo.close_time_entry(id, entry.id).await?;
            }
        }

        if self.repo.unfocus(id).await? {
            // Log activity
            if self.config.auto_log_activity {
                let _ = self
                    .repo
                    .log_activity(id, "focus_ended", None, None, None, "user", None)
                    .await;
            }

            // Notify focus_alarms subscriber to cancel any pending warnings.
            if let Some(ref bus) = self.domain_bus {
                bus.publish(bus::DomainEvent::TaskFocusChanged {
                    task_id: id.to_string(),
                    focus_deadline: None,
                });
            }

            Ok(format!("Unfocused task: {}", id))
        } else {
            Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
        }
    }

    pub(crate) async fn handle_log_time(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let duration_minutes = p.required_u64("duration_minutes")?;
        let duration_secs = (duration_minutes * 60) as i64;
        let started_at = Timestamp::now() - SignedDuration::from_secs(duration_secs);
        let note = p.optional_str("note")?;
        let energy_level = p.optional_str("energy_level")?;

        self.repo
            .add_time_entry(
                id,
                "manual",
                started_at,
                Some(duration_secs),
                note,
                energy_level,
            )
            .await?;

        Ok(format!(
            "Logged {} minutes to task: {}",
            duration_minutes, id
        ))
    }
}
