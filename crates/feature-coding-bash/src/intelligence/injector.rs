//! Phase 2.3b — ExecutionIntelligenceInjector
//!
//! Surfaces, during plan mode, those pending TodoItems whose titles look like
//! verification steps (Run/Test/Check/Verify/Build), as background-bash
//! candidates the LLM should consider after `/plan-exit`.

use std::sync::Arc;

use bus::context_updates::{ContextUpdate, ContextUpdateReason, UpdatePriority};
use bus::injection::{DynamicInjector, InjectorContext};
use jiff::Timestamp;
use storage::repos::TodoRepo;
use tools_core::JobSupervisorHandle;

use crate::intelligence::{classify_verification, VerificationVerb};
use crate::render::{verification_affordance_reminder, VerificationAffordance};

pub struct ExecutionIntelligenceInjector {
    todo_repo:  TodoRepo,
    supervisor: Arc<dyn JobSupervisorHandle>,
}

impl ExecutionIntelligenceInjector {
    pub fn new(todo_repo: TodoRepo, supervisor: Arc<dyn JobSupervisorHandle>) -> Self {
        Self { todo_repo, supervisor }
    }
}

impl DynamicInjector for ExecutionIntelligenceInjector {
    fn name(&self) -> &str {
        "execution-intelligence"
    }

    fn collect(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate> {
        if !ctx.plan_mode_active() {
            return vec![];
        }
        let chain = ctx.agent_chain();
        if chain.is_empty() {
            return vec![];
        }

        // Block on async TodoRepo + JobSupervisor reads. The injector trait
        // is synchronous; we mirror PlanModeInjector's pattern.
        let todos = match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                self.todo_repo.list_for_thread(ctx.thread_id())
            )
        }) {
            Ok(items) => items,
            Err(e) => {
                tracing::debug!(error = ?e, "todo lookup failed in injector; suppressing affordance");
                return vec![];
            }
        };

        let active_jobs = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                self.supervisor.list(ctx.thread_id(), chain, true)
            )
        });

        // Filter: pending or in-progress todos whose title classifies, and which
        // are NOT already covered by an active job (substring match on description).
        let mut affordances: Vec<(String, String, VerificationVerb)> = Vec::new();
        for row in &todos {
            // Parse items_json into individual todo items
            let items: Vec<serde_json::Value> = match serde_json::from_str(&row.items_json) {
                Ok(v) => v,
                Err(e) => {
                    tracing::debug!(error = ?e, "failed to parse items_json; skipping");
                    continue;
                }
            };

            for item in items {
                let Some(title) = item.get("title").and_then(|v| v.as_str()) else { continue };
                let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
                if !is_pending_or_in_progress(status) {
                    continue;
                }
                let Some(verb) = classify_verification(title) else { continue };
                if active_jobs.iter().any(|j| j.description.contains(title)) {
                    continue;
                }
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                affordances.push((id, title.to_string(), verb));
            }
        }

        if affordances.is_empty() {
            return vec![];
        }

        let view: Vec<VerificationAffordance<'_>> = affordances
            .iter()
            .map(|(id, title, verb)| VerificationAffordance {
                todo_id: id.as_str(),
                title:   title.as_str(),
                verb:    *verb,
            })
            .collect();

        let body = verification_affordance_reminder(&view);
        vec![ContextUpdate {
            reason:   ContextUpdateReason::CodingJobsChanged,
            content:  Some(body),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: Timestamp::now(),
        }]
    }
}

fn is_pending_or_in_progress(status: &str) -> bool {
    matches!(status, "pending" | "in_progress")
}
