//! List, show, summary, tree, report, plan, and list_recurring action handlers.

use crate::params::ParamExtractor;
use crate::todo_types::{Todo, TodoFilter, TodoStatus};
use chrono::Utc;
use common::{Result, ToolError};
use std::collections::HashMap;
use tracing::{debug, info};

use super::super::TodoTool;

impl TodoTool {
    pub(crate) async fn handle_list(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let filter = TodoFilter {
            status: p
                .optional_str("status")?
                .and_then(TodoStatus::from_str_loose),
            priority_min: p.optional_u64("priority_min")?.map(|v| v as u8),
            tag: p.optional_str("tag")?.map(String::from),
            limit: p.optional_u64("limit")?.map(|l| l as usize),
            project_id: p.optional_str("project_id")?.map(String::from),
            parent_id: p.optional_str("parent_id")?.map(String::from),
            include_templates: false,
        };

        let rows = self.repo.list(&filter.to_storage_filter()).await?;
        let todos: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

        if todos.is_empty() {
            return Ok("No tasks found.".to_string());
        }

        let mut output = format!("{} task(s):\n", todos.len());
        for todo in todos {
            output.push_str(&format!(
                "\n- [{}] {} (P{}, {:?}, {})",
                todo.id,
                todo.title,
                todo.priority.unwrap_or(3),
                todo.status,
                todo.tags.join(", ")
            ));
        }
        Ok(output)
    }

    pub(crate) async fn handle_show(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        match self.get_full_todo(id).await? {
            Some(todo) => {
                let mut output = format!("Task: {}\n", todo.title);
                output.push_str(&format!("ID: {}\n", todo.id));
                output.push_str(&format!("Status: {:?}\n", todo.status));
                output.push_str(&format!("Priority: {}\n", todo.priority.unwrap_or(3)));
                if let Some(desc) = &todo.description {
                    output.push_str(&format!("Description: {}\n", desc));
                }
                if !todo.tags.is_empty() {
                    output.push_str(&format!("Tags: {}\n", todo.tags.join(", ")));
                }
                if let Some(due) = &todo.due_date {
                    output.push_str(&format!("Due: {}\n", due.format("%Y-%m-%d")));
                }

                Ok(output)
            }
            None => Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into()),
        }
    }

    pub(crate) async fn handle_summary(&self) -> Result<String> {
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
        let rows = self.repo.list(&storage::TodoFilter::default()).await?;

        let mut todos: Vec<Todo> = Vec::with_capacity(rows.len());
        for row in rows {
            todos.push(self.load_full_todo(row).await?);
        }

        let roots: Vec<_> = todos.iter().filter(|t| t.parent_id.is_none()).collect();

        if roots.is_empty() {
            return Ok("No tasks found.".to_string());
        }

        fn render_tree(todo: &Todo, all_todos: &[Todo], prefix: &str, is_last: bool) -> String {
            let mut output = String::new();

            let connector = if is_last { "└─ " } else { "├─ " };
            output.push_str(&format!(
                "{}{}{} [{}] (P{}, {:?})\n",
                prefix,
                connector,
                todo.title,
                todo.id,
                todo.priority.unwrap_or(3),
                todo.status
            ));

            let detail_prefix = format!("{}{}  ", prefix, if is_last { " " } else { "│" });
            if !todo.blocked_by.is_empty() {
                for dep_id in &todo.blocked_by {
                    let dep_title = all_todos
                        .iter()
                        .find(|t| t.id == *dep_id)
                        .map(|t| t.title.as_str())
                        .unwrap_or("(unknown)");
                    output.push_str(&format!(
                        "{}⛔ Blocked by: [{}] {}\n",
                        detail_prefix, dep_id, dep_title
                    ));
                }
            }
            if !todo.blocks.is_empty() {
                for dep_id in &todo.blocks {
                    let dep_title = all_todos
                        .iter()
                        .find(|t| t.id == *dep_id)
                        .map(|t| t.title.as_str())
                        .unwrap_or("(unknown)");
                    output.push_str(&format!(
                        "{}→ Blocks: [{}] {}\n",
                        detail_prefix, dep_id, dep_title
                    ));
                }
            }

            let children: Vec<_> = all_todos
                .iter()
                .filter(|t| t.parent_id.as_ref() == Some(&todo.id))
                .collect();

            for (i, child) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;
                output.push_str(&render_tree(
                    child,
                    all_todos,
                    &detail_prefix,
                    is_last_child,
                ));
            }

            output
        }

        let mut output = String::from("Task Tree:\n\n");
        for (i, root) in roots.iter().enumerate() {
            let is_last = i == roots.len() - 1;
            output.push_str(&render_tree(root, &todos, "", is_last));
        }

        Ok(output)
    }

    pub(crate) async fn handle_report(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let period = p.required_str("period")?;
        let project_id_filter = p.optional_str("project_id")?.map(String::from);

        let now = Utc::now();
        let period_start = match period {
            "week" => now - chrono::Duration::days(7),
            "month" => now - chrono::Duration::days(30),
            _ => {
                return Err(ToolError::InvalidParams(
                    "period must be 'week' or 'month'".to_string(),
                )
                .into())
            }
        };

        let rows = self.repo.list(&storage::TodoFilter::default()).await?;
        let mut todos: Vec<Todo> = Vec::with_capacity(rows.len());
        for row in rows {
            todos.push(self.load_full_todo(row).await?);
        }

        let todos: Vec<_> = if let Some(ref proj_id) = project_id_filter {
            todos
                .into_iter()
                .filter(|t| t.project_id.as_ref() == Some(proj_id))
                .collect()
        } else {
            todos
        };

        let mut projects: HashMap<String, Vec<&Todo>> = HashMap::new();
        for todo in &todos {
            let project_key = todo
                .project_id
                .clone()
                .unwrap_or_else(|| "(no project)".to_string());
            projects.entry(project_key).or_default().push(todo);
        }

        let mut completed_in_period_total = 0;
        let mut created_in_period_total = 0;
        let mut total_time_secs = 0u64;
        let mut focus_session_count = 0;
        let mut focus_time_secs = 0u64;
        let mut overdue_count = 0;

        let mut output = String::from("=== Todo Report ===\n\n");
        output.push_str(&format!(
            "Period: Last {} days\n\n",
            match period {
                "week" => 7,
                "month" => 30,
                _ => 0,
            }
        ));

        output.push_str("## By Project:\n\n");
        for (project_key, project_todos) in &projects {
            let completed_in_period = project_todos
                .iter()
                .filter(|t| {
                    t.status == TodoStatus::Done
                        && t.completed_at.map(|dt| dt >= period_start).unwrap_or(false)
                })
                .count();

            let created_in_period = project_todos
                .iter()
                .filter(|t| t.created_at >= period_start)
                .count();

            let time_tracked: u64 = project_todos
                .iter()
                .flat_map(|t| &t.time_entries)
                .filter_map(|e| e.duration_secs)
                .sum();

            completed_in_period_total += completed_in_period;
            created_in_period_total += created_in_period;
            total_time_secs += time_tracked;

            output.push_str(&format!("### {}\n", project_key));
            output.push_str(&format!("  - Completed: {}\n", completed_in_period));
            output.push_str(&format!("  - Created: {}\n", created_in_period));
            output.push_str(&format!(
                "  - Time tracked: {:.1}h\n",
                time_tracked as f64 / 3600.0
            ));
            output.push('\n');
        }

        for todo in &todos {
            for entry in &todo.time_entries {
                if entry.source == crate::todo_types::TimeEntrySource::Focus
                    && entry.started_at >= period_start
                {
                    focus_session_count += 1;
                    if let Some(duration) = entry.duration_secs {
                        focus_time_secs += duration;
                    }
                }
            }

            if let Some(due) = todo.due_date {
                if due < now && todo.status != TodoStatus::Done {
                    overdue_count += 1;
                }
            }
        }

        output.push_str("## Summary:\n\n");
        output.push_str(&format!("  - Tasks created: {}\n", created_in_period_total));
        output.push_str(&format!(
            "  - Tasks completed: {}\n",
            completed_in_period_total
        ));
        output.push_str(&format!(
            "  - Total time tracked: {:.1}h\n",
            total_time_secs as f64 / 3600.0
        ));
        output.push_str(&format!("  - Focus sessions: {}\n", focus_session_count));
        output.push_str(&format!(
            "  - Focus time: {:.1}h\n",
            focus_time_secs as f64 / 3600.0
        ));
        output.push_str(&format!("  - Overdue tasks: {}\n", overdue_count));

        Ok(output)
    }

    pub(crate) async fn handle_plan(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let count = p.optional_u64("count")?.unwrap_or(3).min(10) as usize;
        info!("Generating daily plan (top {} tasks)", count);

        let rows = self.repo.list(&storage::TodoFilter::default()).await?;
        let all_tasks: Vec<Todo> = rows.into_iter().map(Todo::from).collect();
        debug!("Total tasks in store: {}", all_tasks.len());

        let now = Utc::now();
        let mut eligible = Vec::new();
        let mut filtered_counts = std::collections::HashMap::new();

        for task in all_tasks {
            if task.status != TodoStatus::Todo {
                *filtered_counts.entry("non_todo").or_insert(0) += 1;
                continue;
            }
            if task.is_template {
                *filtered_counts.entry("template").or_insert(0) += 1;
                continue;
            }
            if task.focused_at.is_some() {
                *filtered_counts.entry("focused").or_insert(0) += 1;
                continue;
            }

            let blockers = self.repo.incomplete_blockers(&task.id).await?;
            if !blockers.is_empty() {
                *filtered_counts.entry("blocked").or_insert(0) += 1;
                continue;
            }

            eligible.push(task);
        }

        debug!(
            "Eligible tasks: {} (filtered: {} non-todo, {} templates, {} focused, {} blocked)",
            eligible.len(),
            filtered_counts.get("non_todo").unwrap_or(&0),
            filtered_counts.get("template").unwrap_or(&0),
            filtered_counts.get("focused").unwrap_or(&0),
            filtered_counts.get("blocked").unwrap_or(&0)
        );

        let mut scored: Vec<(Todo, f64)> = eligible
            .into_iter()
            .map(|task| {
                let score = Self::calculate_score(&task, now);
                debug!("Scored task [{}] '{}': {:.2}", task.id, task.title, score);
                (task, score)
            })
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.created_at.cmp(&b.0.created_at))
        });

        let plan: Vec<&(Todo, f64)> = scored.iter().take(count).collect();

        if plan.is_empty() {
            info!("Daily plan generated: No eligible tasks (all clear)");
            return Ok("No eligible tasks for planning. All clear!".to_string());
        }

        info!(
            "Daily plan generated: {} tasks (top scores: {:.1}, {:.1}, {:.1})",
            plan.len(),
            plan.first().map(|(_, s)| s).unwrap_or(&0.0),
            plan.get(1).map(|(_, s)| s).unwrap_or(&0.0),
            plan.get(2).map(|(_, s)| s).unwrap_or(&0.0)
        );

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
                    let days_overdue = (now_date - due_date).num_days();
                    format!("Overdue by {} day(s)", days_overdue)
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

            output.push_str(&format!(
                "{}. {} (ID: {})\n   {} · {} · Score: {:.1}{}\n\n",
                i + 1,
                task.title,
                task.id,
                priority_label,
                due_context,
                score,
                duration_info
            ));
        }

        Ok(output)
    }

    pub(crate) async fn handle_list_recurring(&self) -> Result<String> {
        let rows = self.repo.list_templates().await?;
        let templates: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

        if templates.is_empty() {
            return Ok("No recurring task templates found.".to_string());
        }

        let mut output = format!("{} recurring template(s):\n\n", templates.len());
        for t in &templates {
            let rule_str = t.recurrence_rule.as_deref().unwrap_or("(no rule)");
            let human = crate::rrule_utils::humanize_rrule(rule_str);
            let next_str = t
                .next_instance_date
                .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| "none".to_string());

            output.push_str(&format!(
                "- [{}] {} | {} | Next: {}\n",
                t.id, t.title, human, next_str
            ));

            if let Some(ref desc) = t.description {
                output.push_str(&format!("  Description: {}\n", desc));
            }
            if let Some(pri) = t.priority {
                output.push_str(&format!("  Priority: {}\n", pri));
            }
            if !t.tags.is_empty() {
                output.push_str(&format!("  Tags: {}\n", t.tags.join(", ")));
            }
        }

        Ok(output)
    }
}
