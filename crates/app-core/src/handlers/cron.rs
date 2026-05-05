//! Cron job handlers — transport-agnostic business logic.

use desktop_shared::errors::ApiError;
use desktop_shared::types::{
    CronJobCreateParams, CronJobResponse, CronJobStateResponse, CronJobUpdateParams,
    CronPayloadResponse, CronStatusResponse,
};
use jiff::Timestamp;
use uuid::Uuid;

use crate::state::{AppCore, HandlerResult};

fn map_cron_err(e: storage::error::StorageError) -> ApiError {
    match e {
        storage::error::StorageError::NotFound(_) => {
            ApiError::new("NOT_FOUND", e.to_string())
        }
        other => ApiError::new("CRON_ERROR", other.to_string()),
    }
}

/// Generate a short 8-character cron job ID from a new UUIDv4.
pub(crate) fn new_cron_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

/// Convert a `CronJobRow` to the IPC response type.
fn to_response(row: &storage::CronJobRow) -> CronJobResponse {
    // Re-use the scheduling crate's row→domain converter for field extraction.
    let job = scheduling::row_to_job(row.clone());
    CronJobResponse {
        id: job.id.clone(),
        name: job.name.clone(),
        // Source `enabled` from the raw DB row, not from `row_to_job`, which forces
        // `enabled=false` on rows with corrupt schedule JSON (execution-path defensive
        // behavior). The list/read path should reflect what the DB actually stores.
        enabled: row.enabled,
        origin: match &job.origin {
            scheduling::CronOrigin::System => "system",
            scheduling::CronOrigin::User => "user",
            scheduling::CronOrigin::Ai => "ai",
            scheduling::CronOrigin::Plugin => "plugin",
        }
        .to_string(),
        schedule: serde_json::to_value(&job.schedule)
            .expect("CronSchedule serialization is infallible"),
        payload: CronPayloadResponse {
            kind: job.payload.kind.clone(),
            message: job.payload.message.clone(),
            deliver: job.payload.deliver,
            channel: job.payload.channel.clone(),
            to: job.payload.to.clone(),
        },
        state: CronJobStateResponse {
            next_run_at_ms: job.state.next_run_at_ms,
            last_run_at_ms: job.state.last_run_at_ms,
            last_status: job.state.last_status.clone(),
            last_error: job.state.last_error.clone(),
        },
        created_at_ms: job.created_at_ms,
        updated_at_ms: job.updated_at_ms,
        delete_after_run: job.delete_after_run,
    }
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn cron_list(
        &self,
        include_disabled: bool,
    ) -> Result<Vec<CronJobResponse>, ApiError> {
        let mut rows = self
            .cron_repo
            .list()
            .await
            .map_err(map_cron_err)?;

        if !include_disabled {
            rows.retain(|r| r.enabled);
        }

        // Sort by next_run_at_ms ascending (None/null sorts last).
        rows.sort_by_key(|r| r.next_run_at_ms.unwrap_or(i64::MAX));

        Ok(rows.iter().map(to_response).collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn cron_status(&self) -> Result<CronStatusResponse, ApiError> {
        let rows = self
            .cron_repo
            .list()
            .await
            .map_err(map_cron_err)?;

        let jobs = rows.len();
        let next_wake_at_ms = rows
            .iter()
            .filter(|r| r.enabled)
            .filter_map(|r| r.next_run_at_ms)
            .min();

        Ok(CronStatusResponse {
            // TODO(4.4c): wire to TemporalScheduler::is_running() once CronService is retired.
            // For now, we're always "enabled" because CronBridge runs as part of TemporalScheduler
            // which is started unconditionally in AppCore construction.
            enabled: true,
            jobs,
            next_wake_at_ms,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn cron_enable(&self, id: String, enabled: bool) -> HandlerResult<CronJobResponse> {
        self.cron_repo
            .set_enabled(&id, enabled)
            .await
            .map_err(|e| match e {
                storage::error::StorageError::NotFound(_) => {
                    ApiError::new("NOT_FOUND", format!("cron job '{id}'"))
                }
                other => ApiError::new("CRON_ERROR", other.to_string()),
            })?;

        self.cron_bridge
            .reconcile_all()
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", e.to_string()))?;

        let row = self
            .cron_repo
            .get(&id)
            .await
            .map_err(map_cron_err)?;

        Ok((to_response(&row), vec![]))
    }

    /// Manually trigger a cron job: set `next_run_at_ms` to now and reconcile
    /// so the TemporalScheduler fires it within the next loop iteration (≤30 s).
    #[tracing::instrument(skip(self), err)]
    pub async fn cron_run(&self, id: String) -> Result<bool, ApiError> {
        let mut row = self
            .cron_repo
            .get_opt(&id)
            .await
            .map_err(map_cron_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("cron job '{id}'")))?;

        let now_ms = Timestamp::now().as_millisecond();
        row.next_run_at_ms = Some(now_ms);
        row.updated_at_ms = now_ms;

        self.cron_repo
            .upsert(&row)
            .await
            .map_err(map_cron_err)?;

        self.cron_bridge
            .reconcile_all()
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", e.to_string()))?;

        // Wake the scheduler so it fires immediately instead of waiting up to 30 s.
        if let Some(ref ts) = self.temporal_scheduler {
            ts.wake();
        }

        Ok(true)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn cron_delete(&self, id: String) -> Result<bool, ApiError> {
        // Guard: system jobs cannot be deleted.
        if let Some(row) = self
            .cron_repo
            .get_opt(&id)
            .await
            .map_err(map_cron_err)?
        {
            if row.origin == "system" {
                return Err(ApiError::new("FORBIDDEN", "Cannot delete system cron jobs"));
            }
        }

        let deleted = self
            .cron_repo
            .delete(&id)
            .await
            .map_err(map_cron_err)?;

        if deleted {
            self.cron_bridge
                .reconcile_all()
                .await
                .map_err(|e| ApiError::new("CRON_ERROR", e.to_string()))?;
        }

        Ok(deleted)
    }

    #[tracing::instrument(skip(self))]
    pub async fn cron_create(&self, params: CronJobCreateParams) -> HandlerResult<CronJobResponse> {
        let schedule: scheduling::CronSchedule = serde_json::from_value(params.schedule)
            .map_err(|e| ApiError::new("INVALID_PARAMS", format!("invalid schedule: {e}")))?;

        let now_ms = Timestamp::now().as_millisecond();
        let job_id = new_cron_id();

        let row = storage::CronJobRow {
            id: job_id,
            name: params.name,
            enabled: true,
            origin: "user".to_string(),
            schedule: serde_json::to_value(&schedule)
                .expect("CronSchedule serialization is infallible"),
            payload: serde_json::json!({
                "kind": "agent_turn",
                "message": params.message,
                "deliver": params.deliver,
                "channel": params.channel,
                "to": params.to,
            }),
            next_run_at_ms: None,
            last_run_at_ms: None,
            last_status: None,
            last_error: None,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            delete_after_run: params.delete_after_run,
            intent_window: None,
            intent_pending_since_ms: None,
        };

        let saved = self
            .cron_repo
            .upsert(&row)
            .await
            .map_err(map_cron_err)?;

        self.cron_bridge
            .reconcile_all()
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", e.to_string()))?;

        Ok((to_response(&saved), vec![]))
    }

    #[tracing::instrument(skip(self))]
    pub async fn cron_update(&self, params: CronJobUpdateParams) -> HandlerResult<CronJobResponse> {
        // TODO(4.4c): update semantics will simplify once CronService is removed —
        // this handler currently inserts a new row and deletes the old to preserve
        // CronService-era ID replacement behavior. In 4.4c, switch to in-place UPDATE
        // which preserves the ID and keeps existing scheduled_fires rows intact.
        let existing = self
            .cron_repo
            .get_opt(&params.id)
            .await
            .map_err(map_cron_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("cron job '{}'", params.id)))?;

        if existing.origin == "system" {
            return Err(ApiError::new("FORBIDDEN", "Cannot edit system cron jobs"));
        }

        let now_ms = Timestamp::now().as_millisecond();

        // Parse existing payload for defaults.
        let ex_payload: scheduling::CronPayload = serde_json::from_value(existing.payload.clone())
            .map_err(|e| {
                ApiError::new(
                    "CRON_ERROR",
                    format!("corrupt payload for job {}: {e}", params.id),
                )
            })?;

        let name = params.name.unwrap_or_else(|| existing.name.clone());
        let schedule_val = match params.schedule {
            Some(s) => {
                // validate round-trip
                let _: scheduling::CronSchedule =
                    serde_json::from_value(s.clone()).map_err(|e| {
                        ApiError::new("INVALID_PARAMS", format!("invalid schedule: {e}"))
                    })?;
                s
            }
            None => existing.schedule.clone(),
        };
        let message = params.message.unwrap_or_else(|| ex_payload.message.clone());
        let deliver = params.deliver.unwrap_or(ex_payload.deliver);
        // params.channel / params.to are Option<Option<String>> (outer = was it in the patch?).
        let channel = match params.channel {
            Some(c) => c,
            None => ex_payload.channel,
        };
        let to = match params.to {
            Some(t) => t,
            None => ex_payload.to,
        };

        // Build a new row with a fresh ID (update semantics: new row replaces old).
        let new_id = new_cron_id();
        let new_row = storage::CronJobRow {
            id: new_id,
            name,
            enabled: existing.enabled,
            origin: existing.origin.clone(),
            schedule: schedule_val,
            payload: serde_json::json!({
                "kind": ex_payload.kind,
                "message": message,
                "deliver": deliver,
                "channel": channel,
                "to": to,
            }),
            next_run_at_ms: None,
            last_run_at_ms: None,
            last_status: None,
            last_error: None,
            created_at_ms: existing.created_at_ms,
            updated_at_ms: now_ms,
            delete_after_run: existing.delete_after_run,
            intent_window: existing.intent_window.clone(),
            intent_pending_since_ms: existing.intent_pending_since_ms,
        };

        // Insert new row first, then delete old — avoids data loss if insert fails.
        let saved = self
            .cron_repo
            .upsert(&new_row)
            .await
            .map_err(map_cron_err)?;

        self.cron_repo
            .delete(&params.id)
            .await
            .map_err(map_cron_err)?;

        self.cron_bridge
            .reconcile_all()
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", e.to_string()))?;

        Ok((to_response(&saved), vec![]))
    }
}
