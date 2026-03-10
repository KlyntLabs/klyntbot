//! Cron job handlers — transport-agnostic business logic.

use desktop_shared::errors::ApiError;
use desktop_shared::types::{
    CronJobCreateParams, CronJobResponse, CronJobStateResponse, CronJobUpdateParams,
    CronPayloadResponse, CronStatusResponse,
};

use crate::state::{AppCore, HandlerResult};

/// Convert a scheduling::CronJob to the IPC response type.
fn to_response(job: &scheduling::CronJob) -> CronJobResponse {
    CronJobResponse {
        id: job.id.clone(),
        name: job.name.clone(),
        enabled: job.enabled,
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
    pub async fn cron_list(
        &self,
        include_disabled: bool,
    ) -> Result<Vec<CronJobResponse>, ApiError> {
        let jobs = self.cron_service.list_jobs(include_disabled).await;
        Ok(jobs.iter().map(to_response).collect())
    }

    pub async fn cron_status(&self) -> Result<CronStatusResponse, ApiError> {
        let status = self.cron_service.status().await;
        Ok(CronStatusResponse {
            enabled: status.enabled,
            jobs: status.jobs,
            next_wake_at_ms: status.next_wake_at_ms,
        })
    }

    pub async fn cron_enable(&self, id: String, enabled: bool) -> HandlerResult<CronJobResponse> {
        let job = self
            .cron_service
            .enable_job(&id, enabled)
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", &format!("cron job '{id}'")))?;
        Ok((to_response(&job), vec![]))
    }

    pub async fn cron_run(&self, id: String) -> Result<bool, ApiError> {
        self.cron_service
            .run_job(&id, true)
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))
    }

    pub async fn cron_delete(&self, id: String) -> Result<bool, ApiError> {
        let jobs = self.cron_service.list_jobs(true).await;
        if let Some(job) = jobs.iter().find(|j| j.id == id) {
            if job.origin == scheduling::CronOrigin::System {
                return Err(ApiError::new("FORBIDDEN", "Cannot delete system cron jobs"));
            }
        }
        self.cron_service
            .remove_job(&id)
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))
    }

    pub async fn cron_create(&self, params: CronJobCreateParams) -> HandlerResult<CronJobResponse> {
        let schedule: scheduling::CronSchedule = serde_json::from_value(params.schedule)
            .map_err(|e| ApiError::new("INVALID_PARAMS", &format!("invalid schedule: {e}")))?;

        let job = self
            .cron_service
            .add_job(
                params.name,
                schedule,
                params.message,
                params.deliver,
                params.channel,
                params.to,
                params.delete_after_run,
                scheduling::CronOrigin::User,
            )
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))?;

        Ok((to_response(&job), vec![]))
    }

    pub async fn cron_update(&self, params: CronJobUpdateParams) -> HandlerResult<CronJobResponse> {
        let jobs = self.cron_service.list_jobs(true).await;
        let existing = jobs
            .iter()
            .find(|j| j.id == params.id)
            .ok_or_else(|| ApiError::new("NOT_FOUND", &format!("cron job '{}'", params.id)))?;

        if existing.origin == scheduling::CronOrigin::System {
            return Err(ApiError::new("FORBIDDEN", "Cannot edit system cron jobs"));
        }

        let name = params.name.unwrap_or_else(|| existing.name.clone());
        let schedule = match params.schedule {
            Some(s) => serde_json::from_value(s)
                .map_err(|e| ApiError::new("INVALID_PARAMS", &format!("invalid schedule: {e}")))?,
            None => existing.schedule.clone(),
        };
        let message = params
            .message
            .unwrap_or_else(|| existing.payload.message.clone());
        let deliver = params.deliver.unwrap_or(existing.payload.deliver);
        let channel = match params.channel {
            Some(c) => c,
            None => existing.payload.channel.clone(),
        };
        let to = match params.to {
            Some(t) => t,
            None => existing.payload.to.clone(),
        };
        let origin = existing.origin.clone();

        // Add new job first, then remove old — avoids data loss if add fails
        let job = self
            .cron_service
            .add_job(
                name,
                schedule,
                message,
                deliver,
                channel,
                to,
                existing.delete_after_run,
                origin,
            )
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))?;

        self.cron_service
            .remove_job(&params.id)
            .await
            .map_err(|e| ApiError::new("CRON_ERROR", &e.to_string()))?;

        Ok((to_response(&job), vec![]))
    }
}
