//! GoalHandlerImpl — Implements GoalHandler trait for agent crate (Layer 5).
//!
//! Bridges the GoalHandler trait (defined in tools at Layer 3) with the
//! GoalRepo (defined in storage at Layer 1.5), following the dependency
//! inversion pattern used by CalendarSyncAdapter.

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use goal::GoalError;
use goal::{conversions, Goal, GoalProgress, GoalStatus};
use plan::PlanStatus;
use providers::DynProvider;
use tools::goal_tool::GoalHandler;
use tracing::warn;
use uuid::Uuid;

use crate::plan_step_generator::{drafts_to_plan_steps, generate_plan_steps};

/// Implements GoalHandler by delegating to GoalRepo.
pub struct GoalHandlerImpl {
    repo: storage::GoalRepo,
    plan_repo: Option<storage::PlanRepo>,
    provider: Option<DynProvider>,
    model: Option<String>,
}

impl GoalHandlerImpl {
    pub fn new(repo: storage::GoalRepo) -> Self {
        Self {
            repo,
            plan_repo: None,
            provider: None,
            model: None,
        }
    }

    /// Attach a plan repository and LLM provider so that `decompose_goal` and
    /// `goal_progress` can function.
    pub fn with_plan_repo(mut self, plan_repo: storage::PlanRepo) -> Self {
        self.plan_repo = Some(plan_repo);
        self
    }

    pub fn with_provider(mut self, provider: DynProvider, model: String) -> Self {
        self.provider = Some(provider);
        self.model = Some(model);
        self
    }
}

#[async_trait]
impl GoalHandler for GoalHandlerImpl {
    async fn create_goal(&self, mut goal: Goal) -> Result<Uuid> {
        goal.updated_at = Utc::now();
        let row = conversions::goal_to_row(&goal);
        self.repo.create(&row).await?;
        for pid in &goal.linked_project_ids {
            self.repo.link_project(goal.id, &pid.to_string()).await?;
        }
        Ok(goal.id)
    }

    async fn get_goal(&self, id: &Uuid) -> Result<Option<Goal>> {
        conversions::load_goal(&self.repo, id).await
    }

    async fn list_goals(&self, status: Option<GoalStatus>) -> Result<Vec<Goal>> {
        let status_str = status.as_ref().map(|s| s.to_string());
        let rows = self.repo.list(status_str.as_deref()).await?;
        let mut goals = Vec::with_capacity(rows.len());
        for row in rows {
            let links = self.repo.get_project_links(row.id).await?;
            let project_ids = links
                .into_iter()
                .filter_map(|l| Uuid::parse_str(&l.project_id).ok())
                .collect();
            goals.push(conversions::row_to_goal(row, project_ids));
        }
        Ok(goals)
    }

    async fn update_goal(&self, mut goal: Goal) -> Result<()> {
        // Validate status transition
        match self.repo.get(goal.id).await {
            Ok(existing) => {
                let old_status = std::str::FromStr::from_str(&existing.status).unwrap_or_default();
                GoalStatus::validate_transition(&old_status, &goal.status)?;
            }
            Err(storage::StorageError::NotFound(_)) => {
                return Err(GoalError::NotFound(goal.id.to_string()).into());
            }
            Err(e) => return Err(e.into()),
        }

        goal.updated_at = Utc::now();
        conversions::save_goal(&self.repo, &goal).await?;
        Ok(())
    }

    async fn delete_goal(&self, id: &Uuid) -> Result<()> {
        let deleted = self.repo.delete(*id).await?;
        if !deleted {
            return Err(GoalError::NotFound(id.to_string()).into());
        }
        Ok(())
    }

    async fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress> {
        let goal = conversions::load_goal(&self.repo, id)
            .await?
            .ok_or_else(|| GoalError::NotFound(id.to_string()))?;
        Ok(conversions::compute_progress(&goal))
    }

    async fn decompose_goal(&self, goal_id: &Uuid) -> Result<String> {
        let (provider, plan_repo) = match (&self.provider, &self.plan_repo) {
            (Some(p), Some(r)) => (p, r),
            _ => return Ok("Goal decomposition requires an LLM provider and plan repository.".to_string()),
        };
        let model = self.model.as_deref().unwrap_or("gpt-4o-mini");

        let goal = conversions::load_goal(&self.repo, goal_id)
            .await?
            .ok_or_else(|| GoalError::NotFound(goal_id.to_string()))?;

        let drafts = match generate_plan_steps(provider, model, &goal.description, &[], &[]).await {
            Ok(d) => d,
            Err(e) => {
                warn!("decompose_goal: LLM call failed for goal {}: {}", goal_id, e);
                return Ok(format!("Could not generate steps for goal '{}': {}", goal.title, e));
            }
        };

        if drafts.is_empty() {
            return Ok(format!("Could not decompose goal '{}': no steps generated.", goal.title));
        }

        let now = Utc::now();
        let plan_id = Uuid::new_v4();
        let steps = drafts_to_plan_steps(&drafts, 0);
        let step_count = steps.len();

        let plan = plan::Plan {
            id: plan_id,
            session_key: format!("goal:{}", goal_id),
            goal_id: Some(*goal_id),
            title: goal.title.clone(),
            description: goal.description.clone(),
            status: PlanStatus::Draft,
            steps,
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        if let Err(e) = plan::conversions::save_plan(plan_repo, &plan).await {
            warn!("decompose_goal: failed to save plan: {}", e);
            return Ok(format!("Generated {} steps but could not persist plan: {}", step_count, e));
        }

        Ok(format!(
            "Created plan '{}' with {} steps (id: {}, status: Draft). Approve the plan to begin execution.",
            goal.title, step_count, plan_id
        ))
    }

    async fn goal_progress(&self, goal_id: &Uuid) -> Result<String> {
        let plan_repo = match &self.plan_repo {
            Some(r) => r,
            None => return Ok("Plan tracking requires a plan repository.".to_string()),
        };

        let goal = conversions::load_goal(&self.repo, goal_id)
            .await?
            .ok_or_else(|| GoalError::NotFound(goal_id.to_string()))?;

        let plan_rows = plan_repo.list(None, None, Some(*goal_id)).await?;

        if plan_rows.is_empty() {
            return Ok(format!("Goal '{}': no plans linked.", goal.title));
        }

        let mut lines = vec![format!("Goal '{}' — linked plans:", goal.title)];
        for row in &plan_rows {
            let step_rows = plan_repo.get_steps(row.id).await.unwrap_or_default();
            let total = step_rows.len();
            let done = step_rows
                .iter()
                .filter(|s| s.status == "Completed")
                .count();
            let pct = if total > 0 {
                (done as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            lines.push(format!(
                "  - '{}' [{}]: {}/{} steps complete ({:.0}%) [id: {}]",
                row.title,
                row.status,
                done,
                total,
                pct,
                &row.id.to_string()[..8]
            ));
        }
        Ok(lines.join("\n"))
    }
}
