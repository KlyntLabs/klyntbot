//! Show, list, summary, and tree action handlers.

use common::{Result, ToolError};
use futures_util::future::try_join_all;
use tools_core::ParamExtractor;

use super::super::TaskTool;
use crate::types::Task;
use storage::TaskFilter;

impl TaskTool {
    pub(crate) async fn handle_list(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let filter = TaskFilter {
            status: p
                .optional_str("status")?
                .and_then(|s| match s.to_lowercase().as_str() {
                    "todo" => Some("todo".to_string()),
                    "doing" => Some("doing".to_string()),
                    "done" => Some("done".to_string()),
                    "someday" => Some("someday".to_string()),
                    _ => None,
                }),
            priority_min: p.optional_u64("priority_min")?.map(|v| v as i16),
            tags: p.optional_str("tag")?.map(|t| vec![t.to_string()]),
            limit: p.optional_u64("limit")?.map(|l| l as i64),
            project_id: p.optional_str("project_id")?.map(String::from),
            area_id: p.optional_str("area_id")?.map(String::from),
            key_result_id: p.optional_str("key_result_id")?.map(String::from),
            unassigned: p.optional_bool("unassigned")?.unwrap_or(false),
            task_type: p.optional_str("task_type")?.map(String::from),
            energy_level: p.optional_str("energy_level")?.map(String::from),
            due_after: None,
            due_before: None,
            templates_only: false,
            root_only: false,
            status_group: None,
            group_id: None,
            execution_state: None,
            completed: None,
        };

        let rows = self.repo.list(&filter).await?;
        let tasks: Vec<Task> = rows.into_iter().map(Task::from).collect();

        if tasks.is_empty() {
            return Ok("No tasks found.".to_string());
        }

        let mut output = format!("{} task(s):\n", tasks.len());
        for task in tasks {
            let priority_str = task
                .priority
                .map(|p| format!("P{}", p))
                .unwrap_or_else(|| "P3".to_string());
            let type_str = if task.task_type != crate::types::TaskType::Manual {
                format!(" [{}]", task.task_type)
            } else {
                String::new()
            };
            output.push_str(&format!(
                "\n- [{}] {} ({}, {}{}{})",
                task.id,
                task.title,
                priority_str,
                task.status,
                type_str,
                if task.tags.is_empty() {
                    String::new()
                } else {
                    format!(", {}", task.tags.join(", "))
                }
            ));
        }
        Ok(output)
    }

    pub(crate) async fn handle_show(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        match self.get_full_task(id).await? {
            Some(task) => {
                let mut output = format!("Task: {}\n", task.title);
                output.push_str(&format!("ID: {}\n", task.id));
                output.push_str(&format!("Status: {}\n", task.status));
                let priority_str = task
                    .priority
                    .map(|p| format!("P{}", p))
                    .unwrap_or_else(|| "P3".to_string());
                output.push_str(&format!("Priority: {}\n", priority_str));
                output.push_str(&format!("Area: {}\n", task.area_id));
                output.push_str(&format!("Type: {}\n", task.task_type));

                if let Some(ref kr_id) = task.key_result_id {
                    output.push_str(&format!("Key Result: {}\n", kr_id));
                }
                if let Some(ref obj_id) = task.objective_id {
                    output.push_str(&format!("Objective: {}\n", obj_id));
                }
                if let Some(desc) = &task.description {
                    output.push_str(&format!("Description: {}\n", desc));
                }
                if let Some(ref criteria) = task.acceptance_criteria {
                    output.push_str(&format!("Acceptance Criteria: {}\n", criteria));
                }
                if !task.tags.is_empty() {
                    output.push_str(&format!("Tags: {}\n", task.tags.join(", ")));
                }
                if let Some(due) = &task.due_date {
                    output.push_str(&format!("Due: {}\n", due.format("%Y-%m-%d")));
                }
                if let Some(ref energy) = task.energy_level {
                    output.push_str(&format!("Energy: {}\n", energy));
                }
                if let Some(ref parent_id) = task.parent_id {
                    output.push_str(&format!("Parent: {}\n", parent_id));
                }
                if let Some(ref project_id) = task.project_id {
                    output.push_str(&format!("Project: {}\n", project_id));
                }
                if task.execution_state != crate::types::ExecutionState::Idle {
                    output.push_str(&format!("Execution: {}\n", task.execution_state));
                }
                if !task.blocked_by.is_empty() {
                    output.push_str(&format!("Blocked by: {}\n", task.blocked_by.join(", ")));
                }
                if !task.blocks.is_empty() {
                    output.push_str(&format!("Blocks: {}\n", task.blocks.join(", ")));
                }
                if !task.attachments.is_empty() {
                    output.push_str(&format!("Attachments: {}\n", task.attachments.len()));
                }
                if task.total_tracked_secs > 0 {
                    output.push_str(&format!(
                        "Time tracked: {:.1}h\n",
                        task.total_tracked_secs as f64 / 3600.0
                    ));
                }
                if let Some(est) = task.estimated_minutes {
                    output.push_str(&format!("Estimated: {} min\n", est));
                }

                // Recent activity
                if let Ok(activities) = self.repo.list_activity(id, 5).await {
                    if !activities.is_empty() {
                        output.push_str("\nRecent Activity:\n");
                        for act in &activities {
                            let summary = act
                                .summary
                                .as_deref()
                                .or(act.field_changed.as_deref())
                                .unwrap_or(&act.activity_type);
                            output.push_str(&format!(
                                "  - {} ({})\n",
                                summary,
                                act.created_at.format("%Y-%m-%d %H:%M")
                            ));
                        }
                    }
                }

                Ok(output)
            }
            None => Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into()),
        }
    }

    pub(crate) async fn handle_summary(&self) -> Result<String> {
        // Try group-based summary first (uses status_label workflow)
        match self.repo.summary_by_group().await {
            Ok(group_counts) if !group_counts.is_empty() => {
                let total: i64 = group_counts.values().sum();
                let mut output = format!("Total tasks: {}\n\n", total);
                output.push_str("By status group:\n");
                for group in &["not_started", "active", "done", "stuck"] {
                    let count = group_counts.get(*group).unwrap_or(&0);
                    output.push_str(&format!("  {}: {}\n", group, count));
                }
                let overdue = self.repo.overdue().await?;
                if !overdue.is_empty() {
                    output.push_str(&format!("\nOverdue: {} tasks\n", overdue.len()));
                }

                // Agentic stats
                let agentic_filter = TaskFilter {
                    task_type: Some("agentic".to_string()),
                    ..Default::default()
                };
                let agentic_count = self.repo.list(&agentic_filter).await?.len();
                if agentic_count > 0 {
                    output.push_str(&format!("\nAgentic tasks: {}\n", agentic_count));
                }

                return Ok(output);
            }
            Ok(_) => {} // no tasks with labels yet, fall through to legacy
            Err(e) => {
                tracing::warn!(error = %e, "summary_by_group failed, falling back to legacy summary");
            }
        }

        // Fallback: legacy status-based summary
        let summary = self.repo.summary().await?;
        let mut output = format!("Total tasks: {}\n\n", summary.total);
        output.push_str("By status:\n");
        output.push_str(&format!("  Todo: {}\n", summary.todo));
        output.push_str(&format!("  Doing: {}\n", summary.doing));
        output.push_str(&format!("  Done: {}\n", summary.done));
        let overdue = self.repo.overdue().await?;
        if !overdue.is_empty() {
            output.push_str(&format!("\nOverdue: {} tasks\n", overdue.len()));
        }
        Ok(output)
    }

    pub(crate) async fn handle_tree(&self) -> Result<String> {
        let rows = self.repo.list(&TaskFilter::default()).await?;
        let tasks: Vec<Task> =
            try_join_all(rows.into_iter().map(|row| self.load_full_task(row))).await?;

        let roots: Vec<_> = tasks.iter().filter(|t| t.parent_id.is_none()).collect();

        if roots.is_empty() {
            return Ok("No tasks found.".to_string());
        }

        fn render_tree(task: &Task, all_tasks: &[Task], prefix: &str, is_last: bool) -> String {
            let mut output = String::new();
            let connector = if is_last { "└─ " } else { "├─ " };
            let priority_str = task
                .priority
                .map(|p| format!("P{}", p))
                .unwrap_or_else(|| "P3".to_string());
            output.push_str(&format!(
                "{}{}{} [{}] ({}, {})\n",
                prefix, connector, task.title, task.id, priority_str, task.status
            ));

            let detail_prefix = format!("{}{}  ", prefix, if is_last { " " } else { "│" });
            if !task.blocked_by.is_empty() {
                for dep_id in &task.blocked_by {
                    let dep_title = all_tasks
                        .iter()
                        .find(|t| t.id == *dep_id)
                        .map(|t| t.title.as_str())
                        .unwrap_or("(unknown)");
                    output.push_str(&format!(
                        "{}Blocked by: [{}] {}\n",
                        detail_prefix, dep_id, dep_title
                    ));
                }
            }

            let children: Vec<_> = all_tasks
                .iter()
                .filter(|t| t.parent_id.as_ref() == Some(&task.id))
                .collect();
            for (i, child) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;
                output.push_str(&render_tree(
                    child,
                    all_tasks,
                    &detail_prefix,
                    is_last_child,
                ));
            }
            output
        }

        let mut output = String::from("Task Tree:\n\n");
        for (i, root) in roots.iter().enumerate() {
            let is_last = i == roots.len() - 1;
            output.push_str(&render_tree(root, &tasks, "", is_last));
        }

        Ok(output)
    }
}
