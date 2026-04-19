//! CronHandler trait adapter for CronExecutor
//!
//! This module provides the glue code to make CronExecutor from scheduling
//! compatible with the CronHandler trait from klyntbot-tools.

use async_trait::async_trait;
use common::Result;
use scheduling::temporal::cron_executor::CronExecutor;
use scheduling::{CronOrigin, CronSchedule};
use std::sync::Arc;
use storage::repos::cron::CronRepo;
use tools::cron_tool::{AddCronJobParams, CronHandler, CronJobInfo};
use uuid::Uuid;

/// Wrapper around CronExecutor + CronRepo that implements CronHandler
pub struct CronHandlerAdapter {
    executor: Arc<CronExecutor>,
    repo: CronRepo,
}

impl CronHandlerAdapter {
    pub fn new(executor: Arc<CronExecutor>, repo: CronRepo) -> Self {
        Self {
            executor: Arc::clone(&executor),
            repo,
        }
    }
}

/// Convert tool CronSchedule to scheduling CronSchedule
fn convert_schedule(schedule: tools::cron_tool::CronSchedule) -> CronSchedule {
    match schedule {
        tools::cron_tool::CronSchedule::Every { every_ms } => CronSchedule::Every { every_ms },
        tools::cron_tool::CronSchedule::Cron { expr, tz } => CronSchedule::Cron { expr, tz },
    }
}

#[async_trait]
impl CronHandler for CronHandlerAdapter {
    async fn add_job(&self, params: AddCronJobParams) -> Result<CronJobInfo> {
        use jiff::Timestamp;
        use storage::rows::cron::CronJobRow;

        let schedule = convert_schedule(params.schedule);
        let origin = match params.origin.as_str() {
            "system" => CronOrigin::System,
            "user" => CronOrigin::User,
            "ai" => CronOrigin::Ai,
            "plugin" => CronOrigin::Plugin,
            other => {
                tracing::warn!("Unknown cron origin '{}', defaulting to Ai", other);
                CronOrigin::Ai
            }
        };
        let origin_str = match origin {
            CronOrigin::System => "system",
            CronOrigin::User => "user",
            CronOrigin::Ai => "ai",
            CronOrigin::Plugin => "plugin",
        };

        let now_ms = Timestamp::now().as_millisecond();
        let id = Uuid::new_v4().to_string()[..8].to_string();
        let row = CronJobRow {
            id: id.clone(),
            name: params.name.clone(),
            enabled: true,
            origin: origin_str.to_string(),
            schedule: serde_json::to_value(&schedule)
                .expect("CronSchedule serialization is infallible"),
            payload: serde_json::json!({
                "message": params.message,
                "deliver": !params.internal,
                "channel": params.channel,
                "to": params.to,
            }),
            next_run_at_ms: None,
            last_run_at_ms: None,
            last_status: None,
            last_error: None,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            delete_after_run: false,
            intent_window: None,
            intent_pending_since_ms: None,
        };
        self.repo
            .upsert(&row)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("cron add_job failed: {e}")))?;

        Ok(CronJobInfo {
            id,
            name: params.name,
            next_run_at_ms: None,
            last_status: None,
        })
    }

    async fn list_jobs(&self, include_internal: bool) -> Vec<CronJobInfo> {
        let rows = match self.repo.list().await {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.into_iter()
            .filter(|row| include_internal || row.origin != "system")
            .map(|row| CronJobInfo {
                id: row.id,
                name: row.name,
                next_run_at_ms: row.next_run_at_ms.map(|ms| ms as u64),
                last_status: row.last_status,
            })
            .collect()
    }

    async fn remove_job(&self, job_id: &str) -> Result<bool> {
        self.repo
            .delete(job_id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("cron remove_job failed: {e}")))
    }

    async fn enable_job(&self, job_id: &str, enabled: bool) -> Result<bool> {
        match self.repo.set_enabled(job_id, enabled).await {
            Ok(()) => Ok(true),
            Err(storage::error::StorageError::NotFound(_)) => Ok(false),
            Err(e) => Err(common::KlyntbotError::Storage(format!(
                "cron enable_job failed: {e}"
            ))),
        }
    }

    async fn run_job(&self, job_id: &str) -> Result<bool> {
        self.executor.run_now(job_id).await
    }
}
