//! Decompose action: AI-powered subtask generation.

use common::Result;
use tools_core::ParamExtractor;
use tracing::info;

use super::super::TaskTool;
use crate::types::{DecompositionContext, DecompositionTree};

fn format_subtask_list(tree: &DecompositionTree) -> String {
    let mut out = String::new();
    for (i, subtask) in tree.subtasks.iter().enumerate() {
        out.push_str(&format!("  {}. {}", i + 1, subtask.title));
        if let Some(est) = subtask.estimated_minutes {
            out.push_str(&format!(" ({}min)", est));
        }
        out.push('\n');
    }
    out
}

impl TaskTool {
    pub(crate) async fn handle_decompose(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.decomposition_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Decomposition handler not available".into())
        })?;

        let id = p.required_str("id")?;

        info!(task_id = %id, "Decomposing task");

        let children_fut = async { self.repo.get_children(id).await.map_err(Into::into) };
        let (task, existing): (crate::types::Task, Vec<storage::TaskRow>) =
            tokio::try_join!(self.require_task(id), children_fut)?;
        let context = DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 10,
            existing_subtasks: existing.iter().map(|r| r.title.clone()).collect(),
            project_context: task.project_id.clone(),
            cognitive_facts: vec![],
            user_energy_profile: None,
            calendar_context: vec![],
        };

        let result = handler.decompose(&task, &context).await?;

        // Confidence gate
        let threshold = self.config.decomposition_auto_apply_threshold;
        if result.confidence >= threshold {
            // Auto-create subtasks
            let mut created_ids = Vec::new();
            for subtask in &result.tree.subtasks {
                let mut sub = crate::types::Task::default_instance();
                sub.title = subtask.title.clone();
                sub.description = subtask.description.clone();
                sub.parent_id = Some(task.id.clone());
                sub.area_id = task.area_id.clone();
                sub.project_id = task.project_id.clone();
                sub.acceptance_criteria = subtask.acceptance_criteria.clone();
                sub.estimated_minutes = subtask.estimated_minutes;
                sub.energy_level = subtask.energy_level.clone();
                sub.priority = subtask.priority;
                if let Some(ref tt) = subtask.task_type {
                    sub.task_type = tt.clone();
                }

                let row = storage::TaskRow::from(&sub);
                let created = self.repo.add(&row).await?;
                created_ids.push(created.id);
            }

            // Log activity
            let _ = self
                .repo
                .log_activity(
                    &task.id,
                    "decomposed",
                    None,
                    None,
                    None,
                    "agent",
                    Some(&format!(
                        "Auto-decomposed into {} subtasks (confidence: {:.0}%)",
                        created_ids.len(),
                        result.confidence * 100.0
                    )),
                )
                .await;

            // Emit domain event
            if let Some(ref bus) = self.domain_bus {
                bus.publish(bus::DomainEvent::TaskDecomposed {
                    source_task_id: task.id.clone(),
                    subtask_ids: created_ids.clone(),
                    total_estimated_mins: result.tree.total_estimated_mins.map(|m| m as i64),
                });
            }

            let mut output = format!(
                "Decomposed '{}' into {} subtasks (confidence: {:.0}%, auto-applied):\n\n",
                task.title,
                created_ids.len(),
                result.confidence * 100.0,
            );
            output.push_str(&format_subtask_list(&result.tree));
            Ok(output)
        } else {
            // Store as pending decomposition for review
            let decomp_row = storage::TaskDecompositionRow {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: task.id.clone(),
                plan: serde_json::to_string(&result.tree)?,
                confidence: result.confidence,
                status: "pending".to_string(),
                reasoning: Some(result.reasoning.clone()),
                created_at: chrono::Utc::now(),
                applied_at: None,
            };
            self.repo.create_decomposition(&decomp_row).await?;

            let mut output = format!(
                "Decomposition plan created for '{}' (confidence: {:.0}%, below {:.0}% threshold — needs review):\n\n",
                task.title,
                result.confidence * 100.0,
                threshold * 100.0,
            );
            output.push_str(&format_subtask_list(&result.tree));
            if !result.validation_warnings.is_empty() {
                output.push_str(&format!(
                    "\nWarnings ({}):\n",
                    result.validation_warnings.len()
                ));
                for w in &result.validation_warnings {
                    output.push_str(&format!("  - {}\n", w.message));
                }
            }
            Ok(output)
        }
    }
}
