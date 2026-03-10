//! AreaTool — Tool interface for PARA area management.
//!
//! Provides 5 actions: create, list, show, update, reorder.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use super::{RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};
use storage::AreaRepo;

/// Tool for managing top-level PARA areas.
pub struct AreaTool {
    area_repo: AreaRepo,
}

impl AreaTool {
    pub fn new(area_repo: AreaRepo) -> Self {
        Self { area_repo }
    }
}

#[async_trait]
impl Tool for AreaTool {
    fn name(&self) -> &str {
        "area"
    }

    fn description(&self) -> &str {
        "Manage areas (top-level PARA containers). Actions: create, list, show, update, reorder."
    }

    fn metadata(&self) -> tools_core::ToolMetadata {
        tools_core::ToolMetadata {
            category: tools_core::ToolCategory::Productivity,
            tags: vec!["area".into(), "para".into(), "responsibility".into()],
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
                    "enum": ["create", "list", "show", "update", "reorder"],
                    "description": "Action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Area ID (for show/update/reorder)"
                },
                "name": {
                    "type": "string",
                    "description": "Area name (for create/update)"
                },
                "description": {
                    "type": "string",
                    "description": "Area description (for create/update)"
                },
                "color": {
                    "type": "string",
                    "enum": ["blue", "green", "purple", "orange", "red", "yellow", "gray"],
                    "description": "Area color (for create/update)"
                },
                "icon": {
                    "type": "string",
                    "description": "Area icon (for create/update)"
                },
                "status": {
                    "type": "string",
                    "enum": ["active", "archived"],
                    "description": "Status filter (for list) or new status (for update)"
                },
                "position": {
                    "type": "integer",
                    "description": "Display position (for reorder)"
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
                let id = domain::Area::generate_id();
                let now = Utc::now();

                let row = storage::rows::area::AreaRow {
                    id: id.clone(),
                    name: name.to_string(),
                    description: p.optional_str("description")?.map(String::from),
                    color: p.optional_str("color")?.unwrap_or("blue").to_string(),
                    icon: p.optional_str("icon")?.map(String::from),
                    position: 0,
                    status: "active".to_string(),
                    created_at: now,
                    updated_at: now,
                };

                let created = self.area_repo.create(&row).await?;

                if let Some(ref tx) = ctx.entity_tx {
                    let _ = tx
                        .send(common::EntityCard {
                            entity_type: "area".to_string(),
                            entity_id: created.id.clone(),
                            title: created.name.clone(),
                            subtitle: None,
                            route: Some(format!("/areas/{}", created.id)),
                            icon_hint: "area".to_string(),
                            metadata: std::collections::HashMap::new(),
                        })
                        .await;
                }

                Ok(format!(
                    "Area created: {} (ID: {})",
                    created.name, created.id
                ))
            }

            "list" => {
                let status = p.optional_str("status")?.map(String::from);
                let rows = self.area_repo.list(status.as_deref()).await?;

                if rows.is_empty() {
                    return Ok("No areas found.".to_string());
                }

                let mut output = format!("Areas ({}):\n\n", rows.len());
                for area in &rows {
                    let project_count = self.area_repo.count_projects(&area.id).await.unwrap_or(0);
                    output.push_str(&format!(
                        "• {} [{}] ({} projects) (ID: {})\n",
                        area.name, area.status, project_count, area.id
                    ));
                    if let Some(ref desc) = area.description {
                        output.push_str(&format!("  {}\n", desc));
                    }
                }
                Ok(output)
            }

            "show" => {
                let id = p.required_str("id")?;
                let area = self
                    .area_repo
                    .get(id)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Area not found".to_string()))?;

                let (project_count, action_count) = tokio::join!(
                    self.area_repo.count_projects(id),
                    self.area_repo.count_actions(id),
                );
                let project_count = project_count.unwrap_or(0);
                let action_count = action_count.unwrap_or(0);

                let mut output = format!("Area: {}\n", area.name);
                output.push_str(&format!("ID: {}\n", area.id));
                output.push_str(&format!("Status: {}\n", area.status));
                output.push_str(&format!("Color: {}\n", area.color));
                if let Some(ref icon) = area.icon {
                    output.push_str(&format!("Icon: {}\n", icon));
                }
                if let Some(ref desc) = area.description {
                    output.push_str(&format!("Description: {}\n", desc));
                }
                output.push_str(&format!("Projects: {}\n", project_count));
                output.push_str(&format!("Actions: {}\n", action_count));
                output.push_str(&format!("Position: {}\n", area.position));
                output.push_str(&format!("Created: {}\n", area.created_at));

                Ok(output)
            }

            "update" => {
                let id = p.required_str("id")?;

                let updated = self
                    .area_repo
                    .update(
                        id,
                        p.optional_str("name")?,
                        if args.get("description").is_some() {
                            Some(p.optional_str("description")?)
                        } else {
                            None
                        },
                        p.optional_str("color")?,
                        if args.get("icon").is_some() {
                            Some(p.optional_str("icon")?)
                        } else {
                            None
                        },
                        p.optional_str("status")?,
                    )
                    .await?;

                Ok(format!(
                    "Area updated: {} (ID: {})",
                    updated.name, updated.id
                ))
            }

            "reorder" => {
                let id = p.required_str("id")?;
                let position = p
                    .optional_u64("position")?
                    .ok_or_else(|| ToolError::InvalidParams("position is required".to_string()))?
                    as i32;

                self.area_repo.reorder(id, position).await?;
                Ok(format!("Area {} moved to position {}", id, position))
            }

            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_area_tool_name() {
        let pool = sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap();
        let tool = AreaTool::new(AreaRepo::new(pool));
        assert_eq!(tool.name(), "area");
    }
}
