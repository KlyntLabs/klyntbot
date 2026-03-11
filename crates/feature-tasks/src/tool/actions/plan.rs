//! Plan day action handler with energy-level matching.

use chrono::Utc;
use common::Result;
use futures_util::future::try_join_all;
use tools_core::ParamExtractor;
use tracing::{debug, info};

use super::super::TaskTool;
use crate::types::{EnergyLevel, Task, TaskStatus};
use storage::TaskFilter;

/// Filter for plannable tasks: todo, not a template, not currently focused.
fn is_plannable(t: &Task) -> bool {
    t.status == TaskStatus::Todo.as_str() && !t.is_template && t.focused_at.is_none()
}

/// Build a TaskFilter that pre-filters at the DB level for plannable candidates.
fn plannable_filter() -> TaskFilter {
    TaskFilter {
        status: Some("todo".into()),
        templates_only: false,
        ..TaskFilter::default()
    }
}

impl TaskTool {
    pub(crate) async fn handle_plan_day(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let count = p.optional_u64("count")?.unwrap_or(3).min(10) as usize;
        let energy_preference = p.optional_str("energy_level")?;

        // If LLM planning handler is available, use it
        if let Some(ref handler) = self.planning_handler {
            return self.handle_plan_day_llm(handler.as_ref(), count).await;
        }

        // Fallback: scoring-only plan (Phase 1 behavior)
        self.handle_plan_day_scoring(count, energy_preference).await
    }

    /// LLM-powered daily planning path.
    async fn handle_plan_day_llm(
        &self,
        handler: &dyn crate::handlers::DayPlanningHandler,
        count: usize,
    ) -> Result<String> {
        use crate::types::PlanningContext;

        let rows = self.repo.list(&plannable_filter()).await?;
        let tasks: Vec<Task> = rows
            .into_iter()
            .map(Task::from)
            .filter(|t| is_plannable(t))
            .collect();
        // Give LLM a broader candidate set (sorted by created_at from DB)
        let tasks: Vec<Task> = tasks.into_iter().take(count * 3).collect();

        let ctx = PlanningContext {
            tasks,
            working_hours: self.config.working_hours.clone(),
            calendar_blocks: vec![],
            energy_profile: None,
            max_tasks: Some(count as u32),
            locked_task_ids: vec![],
            target_date: None,
        };

        let plan = handler.plan_day(&ctx).await?;

        // Format output
        let mut output = format!("Daily plan ({} slots):\n\n", plan.slots.len());
        for (i, slot) in plan.slots.iter().enumerate() {
            let time = slot.start_time.as_deref().unwrap_or("--:--");
            let energy = slot
                .energy_level
                .as_ref()
                .map(|e| format!(" · energy: {e}"))
                .unwrap_or_default();
            output.push_str(&format!(
                "{}. [{}] {} (ID: {}) · {}min{}\n",
                i + 1,
                time,
                slot.title,
                slot.task_id,
                slot.estimated_minutes,
                energy,
            ));
        }

        if !plan.deferred.is_empty() {
            output.push_str(&format!("\nDeferred ({}):\n", plan.deferred.len()));
            for d in &plan.deferred {
                output.push_str(&format!("  - {} ({})\n", d.title, d.reason));
            }
        }

        output.push_str(&format!(
            "\nTotal: {}min ({:.1}h) · Utilization: {:.0}%\n",
            plan.total_work_mins,
            plan.total_work_mins as f64 / 60.0,
            plan.utilization * 100.0,
        ));

        if !plan.reasoning.is_empty() {
            output.push_str(&format!("\n{}\n", plan.reasoning));
        }

        Ok(output)
    }

    /// Scoring-only daily plan (Phase 1 fallback).
    async fn handle_plan_day_scoring(
        &self,
        count: usize,
        energy_preference: Option<&str>,
    ) -> Result<String> {
        info!(
            "Generating daily plan (top {} tasks, scoring fallback)",
            count
        );

        let rows = self.repo.list(&plannable_filter()).await?;
        let all_tasks: Vec<Task> = rows.into_iter().map(Task::from).collect();
        debug!("Total tasks in store: {}", all_tasks.len());

        let now = Utc::now();
        let candidates: Vec<_> = all_tasks.into_iter().filter(|t| is_plannable(t)).collect();

        // Batch-check blockers concurrently instead of N sequential queries
        let blocker_results: Vec<Vec<_>> = try_join_all(
            candidates
                .iter()
                .map(|t| self.repo.incomplete_blockers(&t.id)),
        )
        .await?;

        let eligible: Vec<_> = candidates
            .into_iter()
            .zip(blocker_results)
            .filter(|(_, blockers)| blockers.is_empty())
            .map(|(task, _)| task)
            .collect();

        let mut scored: Vec<(Task, f64)> = eligible
            .into_iter()
            .map(|task| {
                let mut score = Self::calculate_score(&task, now);

                // Energy-level matching bonus
                if let Some(pref) = &energy_preference {
                    if let Ok(pref_level) = pref.parse::<EnergyLevel>() {
                        if task.energy_level.as_ref() == Some(&pref_level) {
                            score += 5.0; // Bonus for matching energy level
                        }
                    }
                }

                (task, score)
            })
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.created_at.cmp(&b.0.created_at))
        });

        let plan: Vec<_> = scored.iter().take(count).collect();

        if plan.is_empty() {
            return Ok("No eligible tasks for planning. All clear!".to_string());
        }

        let total_est_mins: i32 = plan.iter().filter_map(|(t, _)| t.estimated_minutes).sum();

        let mut output = format!("Daily plan ({} tasks):\n\n", plan.len());
        for (i, (task, score)) in plan.iter().enumerate() {
            let priority_label = task
                .priority
                .map(|p| format!("P{}", p))
                .unwrap_or_else(|| "P3".to_string());

            let due_context = if let Some(due) = task.due_date {
                let due_date = due.date_naive();
                let now_date = now.date_naive();
                if due_date < now_date {
                    let days = (now_date - due_date).num_days();
                    format!("Overdue by {} day(s)", days)
                } else if due_date == now_date {
                    "Due today".to_string()
                } else if due_date == now_date + chrono::Duration::days(1) {
                    "Due tomorrow".to_string()
                } else {
                    format!("Due {}", due_date.format("%Y-%m-%d"))
                }
            } else {
                "No deadline".to_string()
            };

            let duration_info = task
                .estimated_minutes
                .map(|m| format!(" · {} min", m))
                .unwrap_or_default();

            let energy_info = task
                .energy_level
                .as_ref()
                .map(|e| format!(" · energy: {}", e))
                .unwrap_or_default();

            output.push_str(&format!(
                "{}. {} (ID: {})\n   {} · {} · Score: {:.1}{}{}\n\n",
                i + 1,
                task.title,
                task.id,
                priority_label,
                due_context,
                score,
                duration_info,
                energy_info,
            ));
        }

        if total_est_mins > 0 {
            output.push_str(&format!(
                "Estimated total: {} min ({:.1}h)\n",
                total_est_mins,
                total_est_mins as f64 / 60.0
            ));
        }

        Ok(output)
    }
}
