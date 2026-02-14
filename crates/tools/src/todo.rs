//! TodoTool - Tool interface for todo system
//!
//! Provides 9 actions for complete todo management through the Tool trait.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{RoutingContext, Tool};
use crate::todo_store::TodoStore;
use crate::todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus};
use common::{Result, ToolError};

/// TodoTool with config-driven focus and confidence values (ADR-008)
pub struct TodoTool {
    store: Arc<RwLock<TodoStore>>,
    max_focus_slots: usize,
    focus_deadline_hours: u64,
    confidence_threshold: f32,
}

impl TodoTool {
    /// Create a new TodoTool with config values
    pub fn new(
        store: Arc<RwLock<TodoStore>>,
        max_focus_slots: usize,
        focus_deadline_hours: u64,
        confidence_threshold: f32,
    ) -> Self {
        Self {
            store,
            max_focus_slots,
            focus_deadline_hours,
            confidence_threshold,
        }
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Manage tasks and todos. Actions: add, list, update, complete, delete, show, summary, focus, unfocus."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "update", "complete", "delete", "show", "summary", "focus", "unfocus"],
                    "description": "Action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Task ID (for update/complete/delete/show/focus/unfocus)"
                },
                "title": {
                    "type": "string",
                    "description": "Task title (for add)"
                },
                "description": {
                    "type": "string",
                    "description": "Task description (for add/update)"
                },
                "priority": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 5,
                    "description": "Priority 1-5 (for add/update)"
                },
                "due_date": {
                    "type": "string",
                    "description": "Due date ISO format (for add/update)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags (for add/update)"
                },
                "status": {
                    "type": "string",
                    "enum": ["todo", "doing", "done", "archived"],
                    "description": "Status (for update)"
                },
                "priority_min": {
                    "type": "integer",
                    "description": "Min priority filter (for list)"
                },
                "tag": {
                    "type": "string",
                    "description": "Tag filter (for list)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results (for list)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'action' parameter".to_string()))?;

        let mut store = self.store.write().await;

        match action {
            "add" => {
                let title = args.get("title").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("missing 'title' parameter".to_string())
                })?;

                let todo = Todo {
                    id: Todo::generate_id(),
                    title: title.to_string(),
                    description: args
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    priority: args
                        .get("priority")
                        .and_then(|v| v.as_u64())
                        .map(|p| p as u8),
                    due_date: args
                        .get("due_date")
                        .and_then(|v| v.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    tags: args
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    status: TodoStatus::Todo,
                    confidence: 0.0, // Will be calculated in add()
                    focused_at: None,
                    focus_deadline: None,
                    focus_expired_count: 0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    completed_at: None,
                };

                let created = store.add(todo).await?;
                let mut output = format!(
                    "Task created: {} (ID: {}, confidence: {:.0}%)",
                    created.title,
                    created.id,
                    created.confidence * 100.0
                );

                // Show confidence breakdown for low/medium confidence tasks
                if created.confidence < self.confidence_threshold {
                    output.push_str("\n\nConfidence breakdown:");
                    output.push_str(&format_confidence_breakdown(&created));
                    output.push_str("\n\n[Low confidence task] Use the ask_user tool to gather missing details.");
                    output.push_str(
                        "\nSuggest specific questions about the missing fields before proceeding.",
                    );
                } else if created.confidence < 0.8 {
                    output.push_str("\n\nConfidence breakdown:");
                    output.push_str(&format_confidence_breakdown(&created));
                    output.push_str("\n\n💡 This task could be improved by adding missing fields.");
                }

                Ok(output)
            }

            "list" => {
                let filter = TodoFilter {
                    status: args
                        .get("status")
                        .and_then(|v| v.as_str())
                        .and_then(|s| match s {
                            "todo" => Some(TodoStatus::Todo),
                            "doing" => Some(TodoStatus::Doing),
                            "done" => Some(TodoStatus::Done),
                            "archived" => Some(TodoStatus::Archived),
                            _ => None,
                        }),
                    priority_min: args
                        .get("priority_min")
                        .and_then(|v| v.as_u64())
                        .map(|p| p as u8),
                    tag: args.get("tag").and_then(|v| v.as_str()).map(String::from),
                    limit: args
                        .get("limit")
                        .and_then(|v| v.as_u64())
                        .map(|l| l as usize),
                };

                let todos = store.list(&filter).await?;
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

            "update" => {
                let id = args.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("missing 'id' parameter".to_string())
                })?;

                let patch = TodoPatch {
                    title: args.get("title").and_then(|v| v.as_str()).map(String::from),
                    description: args
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| Some(s.to_string())),
                    priority: args
                        .get("priority")
                        .and_then(|v| v.as_u64())
                        .map(|p| p as u8),
                    due_date: args
                        .get("due_date")
                        .and_then(|v| v.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| Some(dt.with_timezone(&Utc))),
                    tags: args.get("tags").and_then(|v| v.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    }),
                    status: args
                        .get("status")
                        .and_then(|v| v.as_str())
                        .and_then(|s| match s {
                            "todo" => Some(TodoStatus::Todo),
                            "doing" => Some(TodoStatus::Doing),
                            "done" => Some(TodoStatus::Done),
                            "archived" => Some(TodoStatus::Archived),
                            _ => None,
                        }),
                };

                match store.update(id, patch).await? {
                    Some(todo) => {
                        let mut output = format!(
                            "Updated task: {} (confidence: {:.0}%)",
                            todo.title,
                            todo.confidence * 100.0
                        );

                        // During enrichment flows, show confidence progress
                        if todo.confidence < 0.8 {
                            output.push_str("\n\n💡 Still missing:");
                            let mut missing = Vec::new();
                            if todo.title.split_whitespace().count() <= 3 {
                                missing.push("title quality (+25%)");
                            }
                            if todo.description.as_ref().map_or(true, |d| d.len() <= 10) {
                                missing.push("description (+25%)");
                            }
                            if todo.priority.is_none() {
                                missing.push("priority (+15%)");
                            }
                            if todo.due_date.is_none() {
                                missing.push("due date (+20%)");
                            }
                            if todo.tags.is_empty() {
                                missing.push("tags (+15%)");
                            }
                            for field in &missing {
                                output.push_str(&format!("\n  - {}", field));
                            }
                        } else {
                            output.push_str(" ✅ High confidence!");
                        }

                        Ok(output)
                    }
                    None => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                    }
                }
            }

            "complete" => {
                let id = args.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("missing 'id' parameter".to_string())
                })?;

                let patch = TodoPatch {
                    status: Some(TodoStatus::Done),
                    ..Default::default()
                };

                match store.update(id, patch).await? {
                    Some(todo) => Ok(format!("Completed: {}", todo.title)),
                    None => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                    }
                }
            }

            "delete" => {
                let id = args.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("missing 'id' parameter".to_string())
                })?;

                if store.delete(id).await? {
                    Ok(format!("Deleted task: {}", id))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                }
            }

            "show" => {
                let id = args.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("missing 'id' parameter".to_string())
                })?;

                match store.get(id).await? {
                    Some(todo) => {
                        let mut output = format!("Task: {}\n", todo.title);
                        output.push_str(&format!("ID: {}\n", todo.id));
                        output.push_str(&format!("Status: {:?}\n", todo.status));
                        output.push_str(&format!("Priority: {}\n", todo.priority.unwrap_or(3)));
                        output.push_str(&format!("Confidence: {:.0}%\n", todo.confidence * 100.0));
                        if let Some(desc) = &todo.description {
                            output.push_str(&format!("Description: {}\n", desc));
                        }
                        if !todo.tags.is_empty() {
                            output.push_str(&format!("Tags: {}\n", todo.tags.join(", ")));
                        }
                        if let Some(due) = &todo.due_date {
                            output.push_str(&format!("Due: {}\n", due.format("%Y-%m-%d")));
                        }

                        // Always include confidence breakdown for show
                        output.push_str(&format!(
                            "\nConfidence breakdown:{}",
                            format_confidence_breakdown(&todo)
                        ));

                        // Suggest improvements for low-confidence tasks
                        if todo.confidence < 0.8 {
                            output.push_str("\n\n💡 Improvement suggestions:");
                            let mut suggestions = Vec::new();
                            if todo.title.split_whitespace().count() <= 3 {
                                suggestions.push("Expand title to >3 words (+25%)");
                            }
                            if todo.description.as_ref().map_or(true, |d| d.len() <= 10) {
                                suggestions.push("Add detailed description (+25%)");
                            }
                            if todo.priority.is_none() {
                                suggestions.push("Set priority 1-5 (+15%)");
                            }
                            if todo.due_date.is_none() {
                                suggestions.push("Add due date (+20%)");
                            }
                            if todo.tags.is_empty() {
                                suggestions.push("Add relevant tags (+15%)");
                            }
                            for suggestion in &suggestions {
                                output.push_str(&format!("\n  - {}", suggestion));
                            }

                            output.push_str("\n\n🤖 AI INSTRUCTION: Consider asking the user if they want to improve this task using YOLO or Party mode.");
                        }

                        Ok(output)
                    }
                    None => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                    }
                }
            }

            "summary" => {
                let summary = store.summary().await?;
                let mut output = format!("Total tasks: {}\n\n", summary.total);
                output.push_str("By status:\n");
                for (status, count) in summary.by_status {
                    output.push_str(&format!("  {:?}: {}\n", status, count));
                }
                if !summary.overdue.is_empty() {
                    output.push_str(&format!("\nOverdue: {} tasks\n", summary.overdue.len()));
                }
                if !summary.low_confidence.is_empty() {
                    output.push_str(&format!(
                        "Low confidence: {} tasks\n",
                        summary.low_confidence.len()
                    ));
                }
                Ok(output)
            }

            "focus" => {
                let id = args.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("missing 'id' parameter".to_string())
                })?;

                // ADR-008: Use config values, not hardcoded
                if store
                    .focus(id, self.max_focus_slots, self.focus_deadline_hours)
                    .await?
                {
                    Ok(format!("Focused on task: {}", id))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                }
            }

            "unfocus" => {
                let id = args.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("missing 'id' parameter".to_string())
                })?;

                if store.unfocus(id).await? {
                    Ok(format!("Unfocused task: {}", id))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                }
            }

            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

/// Format a confidence breakdown showing which fields contribute to the score
fn format_confidence_breakdown(todo: &Todo) -> String {
    let mut parts = Vec::new();

    let title_ok = todo.title.split_whitespace().count() > 3;
    parts.push(format!(
        "\n  {} Title quality (25%) — {}",
        if title_ok { "✅" } else { "⬜" },
        if title_ok { "good" } else { "needs >3 words" }
    ));

    let desc_ok = todo.description.as_ref().is_some_and(|d| d.len() > 10);
    parts.push(format!(
        "\n  {} Description (25%) — {}",
        if desc_ok { "✅" } else { "⬜" },
        if desc_ok { "set" } else { "not set" }
    ));

    let pri_ok = todo.priority.is_some();
    parts.push(format!(
        "\n  {} Priority (15%) — {}",
        if pri_ok { "✅" } else { "⬜" },
        if let Some(p) = todo.priority {
            format!("P{}", p)
        } else {
            "not set".to_string()
        }
    ));

    let due_ok = todo.due_date.is_some();
    parts.push(format!(
        "\n  {} Due date (20%) — {}",
        if due_ok { "✅" } else { "⬜" },
        if let Some(d) = &todo.due_date {
            d.format("%Y-%m-%d").to_string()
        } else {
            "not set".to_string()
        }
    ));

    let tags_ok = !todo.tags.is_empty();
    parts.push(format!(
        "\n  {} Tags (15%) — {}",
        if tags_ok { "✅" } else { "⬜" },
        if tags_ok {
            todo.tags.join(", ")
        } else {
            "none".to_string()
        }
    ));

    parts.join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo_store::TodoStore;
    use tempfile::TempDir;

    async fn create_test_tool() -> (TodoTool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("todos.jsonl");
        let store = Arc::new(RwLock::new(TodoStore::new(file_path)));
        let tool = TodoTool::new(store, 3, 18, 0.5);
        (tool, temp_dir)
    }

    fn ctx() -> RoutingContext {
        RoutingContext::new(
            common::ChannelName::new("telegram"),
            common::ChatId::new("test"),
        )
    }

    #[tokio::test]
    async fn test_tool_name() {
        let (tool, _dir) = create_test_tool().await;
        assert_eq!(tool.name(), "todo");
    }

    #[tokio::test]
    async fn test_add_action() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "add",
            "title": "Test task"
        });

        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(result.contains("Task created"));
        assert!(result.contains("Test task"));
        assert!(result.contains("confidence"));
    }

    #[tokio::test]
    async fn test_add_with_all_fields() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "add",
            "title": "Complete task with all fields",
            "description": "A detailed description",
            "priority": 4,
            "tags": ["backend", "urgent"]
        });

        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(result.contains("Complete task with all fields"));
    }

    #[tokio::test]
    async fn test_list_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task first
        let add_args = serde_json::json!({
            "action": "add",
            "title": "Task to list"
        });
        tool.execute(add_args, &ctx()).await.unwrap();

        // List all tasks
        let list_args = serde_json::json!({
            "action": "list"
        });

        let result = tool.execute(list_args, &ctx()).await.unwrap();
        assert!(result.contains("1 task(s)"));
        assert!(result.contains("Task to list"));
    }

    #[tokio::test]
    async fn test_list_empty() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "list"
        });

        let result = tool.execute(args, &ctx()).await.unwrap();
        assert_eq!(result, "No tasks found.");
    }

    #[tokio::test]
    async fn test_update_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Original title"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        // Extract ID from result
        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(',').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Update the task
        let update_args = serde_json::json!({
            "action": "update",
            "id": id,
            "title": "Updated title"
        });

        let result = tool.execute(update_args, &ctx()).await.unwrap();
        assert!(result.contains("Updated task"));
        assert!(result.contains("Updated title"));
    }

    #[tokio::test]
    async fn test_complete_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to complete"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(',').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Complete it
        let complete_args = serde_json::json!({
            "action": "complete",
            "id": id
        });

        let result = tool.execute(complete_args, &ctx()).await.unwrap();
        assert!(result.contains("Completed"));
        assert!(result.contains("Task to complete"));
    }

    #[tokio::test]
    async fn test_delete_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to delete"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(',').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Delete it
        let delete_args = serde_json::json!({
            "action": "delete",
            "id": id
        });

        let result = tool.execute(delete_args, &ctx()).await.unwrap();
        assert!(result.contains("Deleted task"));
        assert!(result.contains(id));
    }

    #[tokio::test]
    async fn test_show_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to show",
                    "description": "Details here",
                    "priority": 5
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(',').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Show it
        let show_args = serde_json::json!({
            "action": "show",
            "id": id
        });

        let result = tool.execute(show_args, &ctx()).await.unwrap();
        assert!(result.contains("Task: Task to show"));
        assert!(result.contains("Description: Details here"));
        assert!(result.contains("Priority: 5"));
    }

    #[tokio::test]
    async fn test_summary_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a few tasks
        tool.execute(
            serde_json::json!({"action": "add", "title": "Task 1"}),
            &ctx(),
        )
        .await
        .unwrap();
        tool.execute(
            serde_json::json!({"action": "add", "title": "Task 2"}),
            &ctx(),
        )
        .await
        .unwrap();

        let summary_args = serde_json::json!({
            "action": "summary"
        });

        let result = tool.execute(summary_args, &ctx()).await.unwrap();
        assert!(result.contains("Total tasks: 2"));
        assert!(result.contains("By status:"));
    }

    #[tokio::test]
    async fn test_focus_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to focus"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(',').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Focus it
        let focus_args = serde_json::json!({
            "action": "focus",
            "id": id
        });

        let result = tool.execute(focus_args, &ctx()).await.unwrap();
        assert!(result.contains("Focused on task"));
    }

    #[tokio::test]
    async fn test_unfocus_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add and focus a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to unfocus"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(',').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        tool.execute(serde_json::json!({"action": "focus", "id": id}), &ctx())
            .await
            .unwrap();

        // Unfocus it
        let unfocus_args = serde_json::json!({
            "action": "unfocus",
            "id": id
        });

        let result = tool.execute(unfocus_args, &ctx()).await.unwrap();
        assert!(result.contains("Unfocused task"));
    }

    #[tokio::test]
    async fn test_missing_action() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "title": "No action"
        });

        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "invalid"
        });

        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_required_id() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "update",
            "title": "Missing ID"
        });

        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_adr008_config_driven_focus() {
        // ADR-008: Focus and confidence values come from config, not hardcoded
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("todos.jsonl");
        let store = Arc::new(RwLock::new(TodoStore::new(file_path)));

        // Create tool with custom config values
        let tool = TodoTool::new(store, 5, 24, 0.6);

        assert_eq!(tool.max_focus_slots, 5);
        assert_eq!(tool.focus_deadline_hours, 24);
        assert_eq!(tool.confidence_threshold, 0.6);
    }
}
