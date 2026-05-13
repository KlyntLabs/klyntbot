use std::sync::Arc;

use bus::context_updates::{ContextUpdate, ContextUpdateReason, UpdatePriority};
use bus::injection::{DynamicInjector, InjectorContext};
use jiff::Timestamp;

use crate::render::{active_jobs_reminder, attach_handoff_reminder};
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
        let active = self.supervisor.list_active_with_attach(ctx.thread_id(), chain);
        if active.is_empty() {
            return vec![];
        }
        // Section 1: active jobs (existing 2.3a body).
        let job_views: Vec<_> = active.iter().map(|(v, _)| v.clone()).collect();
        let mut body = active_jobs_reminder(&job_views);
        // Section 2: cooperative handoff for attached jobs.
        let attached: Vec<_> = active
            .iter()
            .filter_map(|(v, at)| at.map(|ts| (v.clone(), ts)))
            .collect();
        if !attached.is_empty() {
            body.push_str("\n\n");
            body.push_str(&attach_handoff_reminder(&attached));
        }
        vec![ContextUpdate {
            reason: ContextUpdateReason::CodingJobsChanged,
            content: Some(body),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: Timestamp::now(),
        }]
    }
}
