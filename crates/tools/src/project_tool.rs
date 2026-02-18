//! ProjectTool - Tool interface for project system
//!
//! Provides 6 actions for project management through the Tool trait.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{RoutingContext, Tool};
use crate::params::ParamExtractor;
use crate::project_store::ProjectStore;
use crate::project_types::{Project, ProjectColor, ProjectFilter, ProjectPatch, ProjectStatus};
use crate::todo_store::TodoStore;
use crate::todo_types::TodoFilter;
use common::{Result, ToolError};

/// ProjectTool for managing projects and project-task relationships
pub struct ProjectTool {
    project_store: Arc<RwLock<ProjectStore>>,
    todo_store: Arc<RwLock<TodoStore>>,
}

impl ProjectTool {
    /// Create a new ProjectTool
    pub fn new(
        project_store: Arc<RwLock<ProjectStore>>,
        todo_store: Arc<RwLock<TodoStore>>,
    ) -> Self {
        Self {
            project_store,
            todo_store,
        }
    }
}

#[async_trait]
impl Tool for ProjectTool {
    fn name(&self) -> &str {
        "project"
    }

    fn description(&self) -> &str {
        "Manage projects. Actions: create, list, show, update, archive, tasks."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "list", "show", "update", "archive", "tasks"],
                    "description": "Action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Project ID (for show/update/archive/tasks)"
                },
                "name": {
                    "type": "string",
                    "description": "Project name (for create/update)"
                },
                "description": {
                    "type": "string",
                    "description": "Project description (for create/update)"
                },
                "color": {
                    "type": "string",
                    "enum": ["red", "orange", "yellow", "green", "blue", "purple", "gray"],
                    "description": "Project color (for create/update)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags (for create/update)"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "paused", "completed", "archived"],
                    "description": "Status (for update)"
                },
                "tag": {
                    "type": "string",
                    "description": "Tag filter (for list)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results (for list/tasks)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "create" => {
                let name = p.required_str("name")?;

                let project = Project {
                    id: Project::generate_id(),
                    name: name.to_string(),
                    description: p.optional_str("description")?.map(String::from),
                    color: p
                        .optional_str("color")?
                        .and_then(parse_color)
                        .unwrap_or_default(),
                    tags: p.string_array_or_empty("tags")?,
                    status: ProjectStatus::Active,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                let mut store = self.project_store.write().await;
                let created = store.add(project).await?;
                Ok(format!(
                    "Project created: {} (ID: {})",
                    created.name, created.id
                ))
            }

            "list" => {
                let filter = ProjectFilter {
                    status: p.optional_str("status")?.and_then(parse_status),
                    tag: p.optional_str("tag")?.map(String::from),
                    limit: p.optional_u64("limit")?.map(|v| v as usize),
                };

                let mut store = self.project_store.write().await;
                let projects = store.list(&filter).await?;

                if projects.is_empty() {
                    return Ok("No projects found".to_string());
                }

                let mut output = String::new();
                output.push_str(&format!("Projects ({}):\n\n", projects.len()));
                for p in projects {
                    output.push_str(&format!(
                        "• {} [{}] (ID: {})\n",
                        p.name,
                        format_status(&p.status),
                        p.id
                    ));
                    if let Some(desc) = &p.description {
                        output.push_str(&format!("  {}\n", desc));
                    }
                    if !p.tags.is_empty() {
                        output.push_str(&format!("  Tags: {}\n", p.tags.join(", ")));
                    }
                }

                Ok(output)
            }

            "show" => {
                let id = p.required_str("id")?;

                let mut store = self.project_store.write().await;
                let project = store
                    .get(id)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Project not found".to_string()))?;

                let mut output = String::new();
                output.push_str(&format!("Project: {}\n", project.name));
                output.push_str(&format!("ID: {}\n", project.id));
                output.push_str(&format!("Status: {}\n", format_status(&project.status)));
                output.push_str(&format!("Color: {:?}\n", project.color));
                if let Some(desc) = &project.description {
                    output.push_str(&format!("Description: {}\n", desc));
                }
                if !project.tags.is_empty() {
                    output.push_str(&format!("Tags: {}\n", project.tags.join(", ")));
                }
                output.push_str(&format!("Created: {}\n", project.created_at));
                output.push_str(&format!("Updated: {}\n", project.updated_at));

                // Count tasks in this project
                let mut todo_store = self.todo_store.write().await;
                let tasks = todo_store
                    .list(&TodoFilter::default())
                    .await?
                    .into_iter()
                    .filter(|t| t.project_id.as_ref() == Some(&project.id))
                    .collect::<Vec<_>>();
                output.push_str(&format!("\nTasks: {}\n", tasks.len()));

                Ok(output)
            }

            "update" => {
                let id = p.required_str("id")?;

                let patch = ProjectPatch {
                    name: p.optional_str("name")?.map(String::from),
                    description: p.optional_str("description")?.map(|s| Some(s.to_string())),
                    color: p.optional_str("color")?.and_then(parse_color),
                    tags: if args.get("tags").is_some() {
                        Some(p.string_array_or_empty("tags")?)
                    } else {
                        None
                    },
                    status: p.optional_str("status")?.and_then(parse_status),
                };

                let mut store = self.project_store.write().await;
                let updated = store
                    .update(id, patch)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Project not found".to_string()))?;

                Ok(format!(
                    "Project updated: {} (ID: {})",
                    updated.name, updated.id
                ))
            }

            "archive" => {
                let id = p.required_str("id")?;

                let patch = ProjectPatch {
                    status: Some(ProjectStatus::Archived),
                    ..Default::default()
                };

                let mut store = self.project_store.write().await;
                let updated = store
                    .update(id, patch)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Project not found".to_string()))?;

                Ok(format!("Project archived: {}", updated.name))
            }

            "tasks" => {
                let id = p.required_str("id")?;

                // Verify project exists
                let mut project_store = self.project_store.write().await;
                let project = project_store
                    .get(id)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Project not found".to_string()))?;

                // Get tasks for this project
                let mut todo_store = self.todo_store.write().await;
                let mut tasks = todo_store
                    .list(&TodoFilter::default())
                    .await?
                    .into_iter()
                    .filter(|t| t.project_id.as_ref() == Some(&project.id))
                    .collect::<Vec<_>>();

                if let Some(limit) = p.optional_u64("limit")?.map(|v| v as usize) {
                    tasks.truncate(limit);
                }

                if tasks.is_empty() {
                    return Ok(format!("No tasks in project '{}'", project.name));
                }

                let mut output = String::new();
                output.push_str(&format!(
                    "Tasks in '{}' ({}):\n\n",
                    project.name,
                    tasks.len()
                ));
                for task in tasks {
                    output.push_str(&format!(
                        "• {} [{}] (ID: {})\n",
                        task.title,
                        task.status.display_name(),
                        task.id
                    ));
                }

                Ok(output)
            }

            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

/// Parse color string to enum
fn parse_color(s: &str) -> Option<ProjectColor> {
    match s.to_lowercase().as_str() {
        "red" => Some(ProjectColor::Red),
        "orange" => Some(ProjectColor::Orange),
        "yellow" => Some(ProjectColor::Yellow),
        "green" => Some(ProjectColor::Green),
        "blue" => Some(ProjectColor::Blue),
        "purple" => Some(ProjectColor::Purple),
        "gray" => Some(ProjectColor::Gray),
        _ => None,
    }
}

/// Parse status string to enum
fn parse_status(s: &str) -> Option<ProjectStatus> {
    match s.to_lowercase().as_str() {
        "active" => Some(ProjectStatus::Active),
        "paused" => Some(ProjectStatus::Paused),
        "completed" => Some(ProjectStatus::Completed),
        "archived" => Some(ProjectStatus::Archived),
        _ => None,
    }
}

/// Format status for display
fn format_status(status: &ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::Active => "Active",
        ProjectStatus::Paused => "Paused",
        ProjectStatus::Completed => "Completed",
        ProjectStatus::Archived => "Archived",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("blue"), Some(ProjectColor::Blue));
        assert_eq!(parse_color("PURPLE"), Some(ProjectColor::Purple));
        assert_eq!(parse_color("invalid"), None);
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(parse_status("active"), Some(ProjectStatus::Active));
        assert_eq!(parse_status("PAUSED"), Some(ProjectStatus::Paused));
        assert_eq!(parse_status("invalid"), None);
    }

    #[tokio::test]
    async fn test_create_project() {
        let dir = tempdir().unwrap();
        let project_path = dir.path().join("projects.jsonl");
        let todo_path = dir.path().join("todos.jsonl");

        let project_store = Arc::new(RwLock::new(ProjectStore::new(project_path)));
        let todo_store = Arc::new(RwLock::new(TodoStore::new(todo_path)));
        let tool = ProjectTool::new(project_store, todo_store);

        let args = serde_json::json!({
            "action": "create",
            "name": "Test Project",
            "description": "A test project",
            "color": "blue"
        });

        let result = tool
            .execute(args, &RoutingContext::new("cli".into(), "test".into()))
            .await
            .unwrap();

        assert!(result.contains("Project created: Test Project"));
    }

    #[tokio::test]
    async fn test_list_projects() {
        let dir = tempdir().unwrap();
        let project_path = dir.path().join("projects.jsonl");
        let todo_path = dir.path().join("todos.jsonl");

        let project_store = Arc::new(RwLock::new(ProjectStore::new(project_path)));
        let todo_store = Arc::new(RwLock::new(TodoStore::new(todo_path)));
        let tool = ProjectTool::new(Arc::clone(&project_store), Arc::clone(&todo_store));

        // Create a project first
        let p1 = Project {
            id: Project::generate_id(),
            name: "Project 1".to_string(),
            description: None,
            color: ProjectColor::Blue,
            tags: vec![],
            status: ProjectStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        project_store.write().await.add(p1).await.unwrap();

        let args = serde_json::json!({
            "action": "list"
        });

        let result = tool
            .execute(args, &RoutingContext::new("cli".into(), "test".into()))
            .await
            .unwrap();

        assert!(result.contains("Project 1"));
    }
}
