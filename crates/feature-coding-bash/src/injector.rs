use std::sync::Arc;

use bus::context_updates::{ContextUpdate, ContextUpdateReason, UpdatePriority};
use bus::injection::{DynamicInjector, InjectorContext};
use jiff::Timestamp;

use crate::render::active_jobs_reminder;
use crate::JobSupervisor;

pub struct BackgroundJobsInjector {
    supervisor: Arc<JobSupervisor>,
}

impl BackgroundJobsInjector {
    pub fn new(supervisor: Arc<JobSupervisor>) -> Self {
        Self { supervisor }
    }
}

impl DynamicInjector for BackgroundJobsInjector {
    fn name(&self) -> &str {
        "background-jobs"
    }

    fn collect(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate> {
        let chain = ctx.agent_chain();
        if chain.is_empty() {
            return vec![];
        }
        let active = self.supervisor.list_active(ctx.thread_id(), chain);
        if active.is_empty() {
            return vec![];
        }
        let body = active_jobs_reminder(&active);
        vec![ContextUpdate {
            reason: ContextUpdateReason::CodingJobsChanged,
            content: Some(body),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: Timestamp::now(),
        }]
    }
}
