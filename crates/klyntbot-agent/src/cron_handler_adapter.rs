//! CronHandler trait adapter for CronService
//!
//! This module provides the glue code to make CronService from klyntbot-cron
//! compatible with the CronHandler trait from klyntbot-tools.

use std::sync::Arc;
use async_trait::async_trait;
use klyntbot_tools::cron_tool::{AddCronJobParams, CronHandler, CronJobInfo};
use klyntbot_core::Result;
use klyntbot_cron::{CronService, CronSchedule};

/// Wrapper around CronService that implements CronHandler
pub struct CronHandlerAdapter {
    service: Arc<CronService>,
}

impl CronHandlerAdapter {
    pub fn new(service: Arc<CronService>) -> Self {
        Self { service }
    }
}

/// Convert tool CronSchedule to service CronSchedule
fn convert_schedule(schedule: klyntbot_tools::cron_tool::CronSchedule) -> CronSchedule {
    match schedule {
        klyntbot_tools::cron_tool::CronSchedule::Every { every_ms } => {
            CronSchedule::Every { every_ms }
        }
        klyntbot_tools::cron_tool::CronSchedule::Cron { expr, tz } => {
            CronSchedule::Cron { expr, tz }
        }
    }
}

#[async_trait]
impl CronHandler for CronHandlerAdapter {
    async fn add_job(&self, params: AddCronJobParams) -> Result<CronJobInfo> {
        let schedule = convert_schedule(params.schedule);

        let job = self
            .service
            .add_job(
                params.name.clone(),
                schedule,
                params.message,
                !params.internal, // deliver if not internal
                params.channel,
                params.to,
                false, // delete_after_run
            )
            .await?;

        Ok(CronJobInfo {
            id: job.id,
            name: job.name,
            next_run_at_ms: job.state.next_run_at_ms.map(|ms| ms as u64),
            last_status: job.state.last_status,
        })
    }

    async fn list_jobs(&self, include_internal: bool) -> Vec<CronJobInfo> {
        let jobs = self.service.list_jobs(!include_internal).await; // include_disabled = !include_internal

        jobs.into_iter()
            .map(|job| CronJobInfo {
                id: job.id,
                name: job.name,
                next_run_at_ms: job.state.next_run_at_ms.map(|ms| ms as u64),
                last_status: job.state.last_status,
            })
            .collect()
    }

    async fn remove_job(&self, job_id: &str) -> Result<bool> {
        self.service.remove_job(job_id).await
    }
}
