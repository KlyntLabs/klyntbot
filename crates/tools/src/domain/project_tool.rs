//! ProjectTool - Tool interface for project system
//!
//! Provides 6 actions for project management through the Tool trait.
//! Projects belong to areas (area_id is required for create).

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use crate::params::ParamExtractor;
use crate::{RoutingContext, Tool};
use common::{Result, ToolError};

/// ProjectTool for managing projects and project-task relationships
pub struct ProjectTool {
    project_repo: storage::ProjectRepo,
    action_repo: storage::ActionRepo,
}

impl ProjectTool {
    pub fn new(project_repo: storage::ProjectRepo, action_repo: storage::ActionRepo) -> Self {
        Self {
            project_repo,
            action_repo,
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

    fn metadata(&self) -> tools_core::ToolMetadata {
        tools_core::ToolMetadata {
            category: tools_core::ToolCategory::TaskManagement,
            tags: vec!["project".into(), "manage".into(), "plan".into()],
            cost_hint: tools_core::CostHint::Free,
            ..Default::default()
        }
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
                "area_id": {
                    "type": "string",
                    "description": "Area ID (required for create, optional for list/update)"
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
                    "description": "Status filter (for list) or new status (for update)"
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

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "create" => {
                let name = p.required_str("name")?;
                let area_id = p.required_str("area_id")?;
                let id = domain::Project::generate_id();
                let now = Utc::now();

                let row = storage::rows::project::ProjectRow {
                    id: id.clone(),
                    area_id: area_id.to_string(),
                    name: name.to_string(),
                    description: p.optional_str("description")?.map(String::from),
                    color: p.optional_str("color")?.unwrap_or("orange").to_string(),
                    tags: p.string_array_or_empty("tags")?,
                    status: "active".to_string(),
                    created_at: now,
                    updated_at: now,
                    workflow_id: None,
                    instructions: None,
                    ai_personality: None,
                    user_role: None,
                    start_date: None,
                    target_end_date: None,
                    settings: None,
                };

                let created = self.project_repo.create(&row).await?;

                if let Some(ref tx) = ctx.entity_tx {
                    let _ = tx
                        .send(common::EntityCard {
                            entity_type: "project".to_string(),
                            entity_id: created.id.clone(),
                            title: created.name.clone(),
                            subtitle: None,
                            route: Some(format!("/projects/{}", created.id)),
                            icon_hint: "project".to_string(),
                            metadata: std::collections::HashMap::new(),
                        })
                        .await;
                }

                Ok(format!(
                    "Project created: {} (ID: {})",
                    created.name, created.id
                ))
            }

            "list" => {
                let filter = storage::ProjectFilter {
                    area_id: p.optional_str("area_id")?.map(String::from),
                    status: p.optional_str("status")?.map(String::from),
                    tags: p.optional_str("tag")?.map(|t| vec![t.to_string()]),
                    limit: p.optional_u64("limit")?.map(|v| v as i64),
                };

                let rows = self.project_repo.list(&filter).await?;

                if rows.is_empty() {
                    return Ok("No projects found.".to_string());
                }

                let mut output = format!("Projects ({}):\n\n", rows.len());
                for proj in &rows {
                    output.push_str(&format!(
                        "• {} [{}] (ID: {})\n",
                        proj.name, proj.status, proj.id
                    ));
                    if let Some(ref desc) = proj.description {
                        output.push_str(&format!("  {}\n", desc));
                    }
                    if !proj.tags.is_empty() {
                        output.push_str(&format!("  Tags: {}\n", proj.tags.join(", ")));
                    }
                }

                Ok(output)
            }

            "show" => {
                let id = p.required_str("id")?;
                let proj = self
                    .project_repo
                    .get(id)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Project not found".to_string()))?;

                let mut output = format!("Project: {}\n", proj.name);
                output.push_str(&format!("ID: {}\n", proj.id));
                output.push_str(&format!("Area: {}\n", proj.area_id));
                output.push_str(&format!("Status: {}\n", proj.status));
                output.push_str(&format!("Color: {}\n", proj.color));
                if let Some(ref desc) = proj.description {
                    output.push_str(&format!("Description: {}\n", desc));
                }
                if !proj.tags.is_empty() {
                    output.push_str(&format!("Tags: {}\n", proj.tags.join(", ")));
                }
                output.push_str(&format!("Created: {}\n", proj.created_at));
                output.push_str(&format!("Updated: {}\n", proj.updated_at));

                let stats = self.project_repo.get_with_stats(id).await?;
                if let Some(s) = stats {
                    output.push_str(&format!(
                        "\nTasks: {} total ({} todo, {} doing, {} done)\n",
                        s.task_count_total,
                        s.task_count_todo,
                        s.task_count_doing,
                        s.task_count_done
                    ));
                }

                Ok(output)
            }

            "update" => {
                let id = p.required_str("id")?;

                let patch = storage::ProjectPatch {
                    id: id.to_string(),
                    area_id: p.optional_str("area_id")?.map(String::from),
                    name: p.optional_str("name")?.map(String::from),
                    description: p.optional_str("description")?.map(|s| Some(s.to_string())),
                    color: p.optional_str("color")?.map(String::from),
                    tags: if args.get("tags").is_some() {
                        Some(p.string_array_or_empty("tags")?)
                    } else {
                        None
                    },
                    status: p.optional_str("status")?.map(String::from),
                    workflow_id: None,
                    ..Default::default()
                };

                let updated = self.project_repo.update(&patch).await?;
                Ok(format!(
                    "Project updated: {} (ID: {})",
                    updated.name, updated.id
                ))
            }

            "archive" => {
                let id = p.required_str("id")?;
                let updated = self.project_repo.archive(id).await?;
                Ok(format!("Project archived: {}", updated.name))
            }

            "tasks" => {
                let id = p.required_str("id")?;

                let proj = self
                    .project_repo
                    .get(id)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Project not found".to_string()))?;

                let filter = storage::ActionFilter {
                    project_id: Some(proj.id.clone()),
                    limit: p.optional_u64("limit")?.map(|v| v as i64),
                    ..Default::default()
                };
                let rows = self.action_repo.list(&filter).await?;

                if rows.is_empty() {
                    return Ok(format!("No tasks in project '{}'", proj.name));
                }

                let mut output = format!("Tasks in '{}' ({}):\n\n", proj.name, rows.len());
                for task in &rows {
                    let priority_str = task
                        .priority
                        .map(|p| format!(" P{}", p))
                        .unwrap_or_default();
                    output.push_str(&format!(
                        "• {} [{}]{} (ID: {})\n",
                        task.title, task.status, priority_str, task.id
                    ));
                }

                Ok(output)
            }

            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_project_tool_name() {
        let pool = sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap();
        let tool = ProjectTool::new(
            storage::ProjectRepo::new(pool.clone()),
            storage::ActionRepo::new(pool),
        );
        assert_eq!(tool.name(), "project");
    }
}
