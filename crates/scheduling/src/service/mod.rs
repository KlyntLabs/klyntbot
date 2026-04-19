//! Cron service helpers retained post-4.4c for `row_to_job`.
//!
//! `CronService` has been removed. Use `CronExecutor` + `TemporalScheduler` instead.
//! `row_to_job` is kept here because `CronExecutor` (same crate) needs it.

use crate::types::{CronJob, CronJobState, CronOrigin, CronSchedule};
use storage::CronJobRow;

/// Convert a storage `CronJobRow` into a scheduling `CronJob`.
///
/// **Transitional visibility:** This is `pub` only until a full layering cleanup
/// moves conversion helpers onto `CronJobRow` directly.
/// Do not add new cross-crate callers.
///
/// Note: rows with corrupt schedule JSON will have `enabled` forced to `false`
/// in the returned `CronJob` as a defensive execution-path measure.
#[doc(hidden)]
pub fn row_to_job(row: CronJobRow) -> CronJob {
    let (schedule, schedule_corrupt) = match serde_json::from_value(row.schedule) {
        Ok(s) => (s, false),
        Err(e) => {
            tracing::error!(
                "Corrupt schedule for cron job '{}': {}; disabling job",
                row.id,
                e
            );
            (
                CronSchedule::Every {
                    every_ms: 86_400_000,
                },
                true,
            )
        }
    };
    let payload = serde_json::from_value(row.payload).unwrap_or_default();
    CronJob {
        id: row.id.clone(),
        name: row.name,
        enabled: row.enabled && !schedule_corrupt,
        origin: match row.origin.as_str() {
            "system" => CronOrigin::System,
            "user" => CronOrigin::User,
            "ai" => CronOrigin::Ai,
            "plugin" => CronOrigin::Plugin,
            other => {
                tracing::warn!(
                    "Unknown cron origin '{}' for job '{}', defaulting to User",
                    other,
                    row.id
                );
                CronOrigin::User
            }
        },
        schedule,
        payload,
        state: CronJobState {
            next_run_at_ms: row.next_run_at_ms,
            last_run_at_ms: row.last_run_at_ms,
            last_status: row.last_status,
            last_error: row.last_error,
        },
        created_at_ms: row.created_at_ms,
        updated_at_ms: row.updated_at_ms,
        delete_after_run: row.delete_after_run,
        intent_window: row
            .intent_window
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        intent_pending_since_ms: row.intent_pending_since_ms,
    }
}
