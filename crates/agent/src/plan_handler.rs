//! PlanHandlerImpl — Implements PlanHandler trait for agent crate (Layer 5).
//!
//! Bridges the PlanHandler trait (defined in tools at Layer 3) with the
//! PlanRepo (defined in storage at Layer 1.5), following the dependency
//! inversion pattern used by GoalHandlerImpl.

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use plan::PlanError;
use plan::{conversions, Plan, PlanStatus};
use providers::DynProvider;
use tools::plan_tool::PlanHandler;
use tracing::warn;
use uuid::Uuid;

use crate::plan_step_generator::{drafts_to_plan_steps, generate_plan_steps};

/// Implements PlanHandler by delegating to PlanRepo.
pub struct PlanHandlerImpl {
    repo: storage::PlanRepo,
    provider: Option<DynProvider>,
    model: Option<String>,
}

impl PlanHandlerImpl {
    pub fn new(repo: storage::PlanRepo) -> Self {
        Self {
            repo,
            provider: None,
            model: None,
        }
    }

    /// Attach an LLM provider so that `generate_steps` can call the LLM.
    pub fn with_provider(mut self, provider: DynProvider, model: String) -> Self {
        self.provider = Some(provider);
        self.model = Some(model);
        self
    }
}

#[async_trait]
impl PlanHandler for PlanHandlerImpl {
    async fn create_plan(
        &self,
        title: &str,
        description: &str,
        session_key: &str,
        goal_id: Option<Uuid>,
    ) -> Result<Plan> {
        let now = Utc::now();
        let plan = Plan {
            id: Uuid::new_v4(),
            session_key: session_key.to_string(),
            goal_id,
            title: title.to_string(),
            description: description.to_string(),
            status: PlanStatus::Draft,
            steps: vec![],
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        conversions::save_plan(&self.repo, &plan).await?;
        Ok(plan)
    }

    async fn get_plan(&self, id: &Uuid) -> Result<Option<Plan>> {
        conversions::load_plan(&self.repo, id).await
    }

    async fn get_active_plan(&self, session_key: &str) -> Result<Option<Plan>> {
        conversions::get_active_plan(&self.repo, session_key).await
    }

    async fn approve_plan(&self, id: &Uuid) -> Result<Plan> {
        let mut plan = conversions::load_plan(&self.repo, id)
            .await?
            .ok_or_else(|| PlanError::NotFound(id.to_string()))?;

        // Strict validation: only Draft → Approved is allowed
        if plan.status != PlanStatus::Draft {
            return Err(PlanError::InvalidState(format!(
                "Cannot approve plan in {:?} state, must be Draft",
                plan.status
            ))
            .into());
        }

        plan.status = PlanStatus::Approved;
        plan.updated_at = Utc::now();

        conversions::save_plan(&self.repo, &plan).await?;
        Ok(plan)
    }

    async fn abandon_plan(&self, id: &Uuid) -> Result<()> {
        let mut plan = conversions::load_plan(&self.repo, id)
            .await?
            .ok_or_else(|| PlanError::NotFound(id.to_string()))?;

        // Validate state transition before changing status
        PlanStatus::validate_transition(&plan.status, &PlanStatus::Abandoned)?;

        plan.status = PlanStatus::Abandoned;
        plan.updated_at = Utc::now();

        conversions::save_plan(&self.repo, &plan).await?;
        Ok(())
    }

    async fn get_step_context(&self, id: &Uuid) -> Result<String> {
        let plan = conversions::load_plan(&self.repo, id)
            .await?
            .ok_or_else(|| PlanError::NotFound(id.to_string()))?;

        // Build context window: current step + next 3
        let idx = plan.current_step_index;
        let window_end = (idx + 4).min(plan.steps.len());

        if idx >= plan.steps.len() {
            return Ok("Plan completed - no active step".to_string());
        }

        let steps_window = &plan.steps[idx..window_end];

        let mut ctx = format!("## Active Plan: {}\n", plan.title);
        ctx.push_str(&format!(
            "Progress: step {}/{}\n\n",
            idx + 1,
            plan.steps.len()
        ));

        for (i, step) in steps_window.iter().enumerate() {
            let marker = if i == 0 {
                ">>> CURRENT"
            } else {
                match i {
                    1 => "    NEXT 1",
                    2 => "    NEXT 2",
                    3 => "    NEXT 3",
                    _ => "    NEXT",
                }
            };
            ctx.push_str(&format!(
                "{}: {}\n  Reasoning: {}\n",
                marker, step.description, step.reasoning
            ));
        }
        Ok(ctx)
    }

    async fn execute_plan(&self, id: &Uuid) -> Result<Plan> {
        let mut plan = conversions::load_plan(&self.repo, id)
            .await?
            .ok_or_else(|| PlanError::NotFound(id.to_string()))?;

        // Validate transition: only Approved → Executing is allowed here
        PlanStatus::validate_transition(&plan.status, &PlanStatus::Executing)?;

        plan.status = PlanStatus::Executing;
        plan.updated_at = Utc::now();

        conversions::save_plan(&self.repo, &plan).await?;
        Ok(plan)
    }

    async fn generate_steps(&self, plan_id: &Uuid) -> Result<()> {
        let provider = match &self.provider {
            Some(p) => p,
            None => {
                // No provider configured — skip silently
                return Ok(());
            }
        };
        let model = self.model.as_deref().unwrap_or("gpt-4o-mini");

        let plan = match conversions::load_plan(&self.repo, plan_id).await? {
            Some(p) => p,
            None => {
                warn!("generate_steps: plan {} not found", plan_id);
                return Ok(());
            }
        };

        // Only generate steps if the plan has none yet
        if !plan.steps.is_empty() {
            return Ok(());
        }

        let drafts = match generate_plan_steps(provider, model, &plan.description, &[], &[]).await {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    "generate_steps: LLM call failed for plan {}: {}",
                    plan_id, e
                );
                return Ok(());
            }
        };

        if drafts.is_empty() {
            return Ok(());
        }

        let steps = drafts_to_plan_steps(&drafts, 0);
        let mut plan = plan;
        plan.steps = steps;
        plan.updated_at = Utc::now();

        conversions::save_plan(&self.repo, &plan).await?;
        Ok(())
    }
}
