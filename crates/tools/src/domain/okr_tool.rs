//! OkrTool — Tool interface for OKR (Objectives & Key Results) management.
//!
//! Provides a dotted-namespace action scheme:
//! - `objective.create`, `objective.list`, `objective.show`, `objective.update`, `objective.delete`
//! - `kr.create`, `kr.list`, `kr.show`, `kr.update`, `kr.update_metric`, `kr.delete`

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;

use crate::{RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};
use storage::{KeyResultRepo, ObjectiveRepo};
use tools_core::ProgressHandler;

/// Tool for managing OKR objectives and key results.
pub struct OkrTool {
    objective_repo: ObjectiveRepo,
    kr_repo: KeyResultRepo,
    progress_handler: Option<Arc<dyn ProgressHandler>>,
}

impl OkrTool {
    pub fn new(objective_repo: ObjectiveRepo, kr_repo: KeyResultRepo) -> Self {
        Self {
            objective_repo,
            kr_repo,
            progress_handler: None,
        }
    }

    pub fn with_progress_handler(mut self, handler: Arc<dyn ProgressHandler>) -> Self {
        self.progress_handler = Some(handler);
        self
    }
}

#[async_trait]
impl Tool for OkrTool {
    fn name(&self) -> &str {
        "okr"
    }

    fn description(&self) -> &str {
        "Manage OKR objectives and key results. Actions: objective.create, objective.list, objective.show, objective.update, objective.delete, kr.create, kr.list, kr.show, kr.update, kr.update_metric, kr.delete"
    }

    fn metadata(&self) -> tools_core::ToolMetadata {
        tools_core::ToolMetadata {
            category: tools_core::ToolCategory::Productivity,
            tags: vec!["okr".into(), "objective".into(), "goal".into()],
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
                    "enum": [
                        "objective.create", "objective.list", "objective.show",
                        "objective.update", "objective.delete",
                        "kr.create", "kr.list", "kr.show",
                        "kr.update", "kr.update_metric", "kr.delete"
                    ],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Objective or KR ID" },
                "project_id": { "type": "string", "description": "Project ID (for objective.create/list)" },
                "objective_id": { "type": "string", "description": "Objective ID (for kr.create/list)" },
                "title": { "type": "string", "description": "Title (for create/update)" },
                "description": { "type": "string", "description": "Description (for create/update)" },
                "status": {
                    "type": "string",
                    "enum": ["active", "paused", "completed", "abandoned"],
                    "description": "Status (for create/update)"
                },
                "priority": { "type": "integer", "description": "Priority (for objectives)" },
                "due_date": { "type": "string", "description": "Due date ISO8601 (for create/update)" },
                "tracking_mode": {
                    "type": "string",
                    "enum": ["metric", "action"],
                    "description": "KR tracking mode (for kr.create)"
                },
                "target_value": { "type": "number", "description": "Target value (for metric KRs)" },
                "current_value": { "type": "number", "description": "Current metric value (for kr.update_metric)" },
                "unit": { "type": "string", "description": "Unit label (for metric KRs)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            // ── Objectives ──────────────────────────────────────────────
            "objective.create" => {
                let project_id = p.required_str("project_id")?;
                let title = p.required_str("title")?;
                let id = domain::Objective::generate_id();
                let now = Utc::now();

                let due_date = p
                    .optional_str("due_date")?
                    .map(|s| s.parse::<chrono::DateTime<Utc>>())
                    .transpose()
                    .map_err(|e| ToolError::InvalidParams(format!("Invalid due_date: {e}")))?;

                let row = storage::rows::objective::ObjectiveRow {
                    id: id.clone(),
                    project_id: project_id.to_string(),
                    title: title.to_string(),
                    description: p.optional_str("description")?.map(String::from),
                    status: "active".to_string(),
                    priority: p.optional_u64("priority")?.map(|v| v as i16),
                    due_date,
                    progress: 0.0,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };

                let created = self.objective_repo.create(&row).await?;

                if let Some(ref tx) = ctx.entity_tx {
                    let _ = tx
                        .send(common::EntityCard {
                            entity_type: "objective".to_string(),
                            entity_id: created.id.clone(),
                            title: created.title.clone(),
                            subtitle: None,
                            route: Some(format!("/objectives/{}", created.id)),
                            icon_hint: "objective".to_string(),
                            metadata: std::collections::HashMap::new(),
                        })
                        .await;
                }

                Ok(format!(
                    "Objective created: {} (ID: {})",
                    created.title, created.id
                ))
            }

            "objective.list" => {
                let project_id = p.optional_str("project_id")?;
                let status = p.optional_str("status")?;
                let rows = self.objective_repo.list(project_id, status).await?;

                if rows.is_empty() {
                    return Ok("No objectives found.".to_string());
                }

                let mut output = format!("Objectives ({}):\n\n", rows.len());
                for obj in &rows {
                    output.push_str(&format!(
                        "• {} [{}] {:.0}% (ID: {})\n",
                        obj.title, obj.status, obj.progress, obj.id
                    ));
                    if let Some(ref desc) = obj.description {
                        output.push_str(&format!("  {}\n", desc));
                    }
                }
                Ok(output)
            }

            "objective.show" => {
                let id = p.required_str("id")?;
                let obj =
                    self.objective_repo.get(id).await?.ok_or_else(|| {
                        ToolError::InvalidParams("Objective not found".to_string())
                    })?;

                let krs = self.kr_repo.list(Some(id)).await?;

                let mut output = format!("Objective: {}\n", obj.title);
                output.push_str(&format!("ID: {}\n", obj.id));
                output.push_str(&format!("Project: {}\n", obj.project_id));
                output.push_str(&format!("Status: {}\n", obj.status));
                output.push_str(&format!("Progress: {:.1}%\n", obj.progress));
                if let Some(ref desc) = obj.description {
                    output.push_str(&format!("Description: {}\n", desc));
                }
                if let Some(p) = obj.priority {
                    output.push_str(&format!("Priority: {}\n", p));
                }
                if let Some(due) = obj.due_date {
                    output.push_str(&format!("Due: {}\n", due));
                }

                if !krs.is_empty() {
                    output.push_str(&format!("\nKey Results ({}):\n", krs.len()));
                    for kr in &krs {
                        output.push_str(&format!(
                            "  • {} [{}] {:.1}% (ID: {})\n",
                            kr.title, kr.tracking_mode, kr.progress, kr.id
                        ));
                    }
                }

                Ok(output)
            }

            "objective.update" => {
                let id = p.required_str("id")?;

                let due_date_opt = if args.get("due_date").is_some() {
                    let s = p.optional_str("due_date")?;
                    match s {
                        Some(s) => {
                            let dt = s.parse::<chrono::DateTime<Utc>>().map_err(|e| {
                                ToolError::InvalidParams(format!("Invalid due_date: {e}"))
                            })?;
                            Some(Some(dt))
                        }
                        None => Some(None), // explicitly clear
                    }
                } else {
                    None
                };

                let updated = self
                    .objective_repo
                    .update(
                        id,
                        p.optional_str("title")?,
                        if args.get("description").is_some() {
                            Some(p.optional_str("description")?)
                        } else {
                            None
                        },
                        p.optional_str("status")?,
                        if args.get("priority").is_some() {
                            Some(p.optional_u64("priority")?.map(|v| v as i16))
                        } else {
                            None
                        },
                        due_date_opt,
                    )
                    .await?;

                Ok(format!(
                    "Objective updated: {} (ID: {})",
                    updated.title, updated.id
                ))
            }

            "objective.delete" => {
                let id = p.required_str("id")?;
                let deleted = self.objective_repo.delete(id).await?;
                if deleted {
                    Ok(format!("Objective {} deleted.", id))
                } else {
                    Err(ToolError::InvalidParams("Objective not found".to_string()).into())
                }
            }

            // ── Key Results ─────────────────────────────────────────────
            "kr.create" => {
                let objective_id = p.required_str("objective_id")?;
                let title = p.required_str("title")?;
                let id = domain::KeyResult::generate_id();
                let now = Utc::now();

                let tracking_mode = p
                    .optional_str("tracking_mode")?
                    .unwrap_or("metric")
                    .to_string();

                let due_date = p
                    .optional_str("due_date")?
                    .map(|s| s.parse::<chrono::DateTime<Utc>>())
                    .transpose()
                    .map_err(|e| ToolError::InvalidParams(format!("Invalid due_date: {e}")))?;

                let row = storage::rows::key_result::KeyResultRow {
                    id: id.clone(),
                    objective_id: objective_id.to_string(),
                    title: title.to_string(),
                    description: p.optional_str("description")?.map(String::from),
                    status: "active".to_string(),
                    tracking_mode,
                    target_value: p.optional_f64("target_value")?,
                    current_value: 0.0,
                    unit: p.optional_str("unit")?.map(String::from),
                    progress: 0.0,
                    due_date,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };

                let created = self.kr_repo.create(&row).await?;

                if let Some(ref tx) = ctx.entity_tx {
                    let _ = tx
                        .send(common::EntityCard {
                            entity_type: "key_result".to_string(),
                            entity_id: created.id.clone(),
                            title: created.title.clone(),
                            subtitle: None,
                            route: Some(format!("/key-results/{}", created.id)),
                            icon_hint: "key_result".to_string(),
                            metadata: std::collections::HashMap::new(),
                        })
                        .await;
                }

                Ok(format!(
                    "Key Result created: {} (ID: {})",
                    created.title, created.id
                ))
            }

            "kr.list" => {
                let objective_id = p.optional_str("objective_id")?;
                let rows = self.kr_repo.list(objective_id).await?;

                if rows.is_empty() {
                    return Ok("No key results found.".to_string());
                }

                let mut output = format!("Key Results ({}):\n\n", rows.len());
                for kr in &rows {
                    let value_str = if kr.tracking_mode == "metric" {
                        if let Some(target) = kr.target_value {
                            let unit = kr.unit.as_deref().unwrap_or("");
                            format!(" ({}{}/{}{})", kr.current_value, unit, target, unit)
                        } else {
                            String::new()
                        }
                    } else {
                        let (total, completed) =
                            self.kr_repo.count_actions(&kr.id).await.unwrap_or((0, 0));
                        format!(" ({}/{} actions)", completed, total)
                    };
                    output.push_str(&format!(
                        "• {} [{}] {:.1}%{} (ID: {})\n",
                        kr.title, kr.status, kr.progress, value_str, kr.id
                    ));
                }
                Ok(output)
            }

            "kr.show" => {
                let id = p.required_str("id")?;
                let kr =
                    self.kr_repo.get(id).await?.ok_or_else(|| {
                        ToolError::InvalidParams("Key result not found".to_string())
                    })?;

                let mut output = format!("Key Result: {}\n", kr.title);
                output.push_str(&format!("ID: {}\n", kr.id));
                output.push_str(&format!("Objective: {}\n", kr.objective_id));
                output.push_str(&format!("Status: {}\n", kr.status));
                output.push_str(&format!("Tracking: {}\n", kr.tracking_mode));
                output.push_str(&format!("Progress: {:.1}%\n", kr.progress));

                if kr.tracking_mode == "metric" {
                    output.push_str(&format!(
                        "Current: {}{}\n",
                        kr.current_value,
                        kr.unit.as_deref().unwrap_or("")
                    ));
                    if let Some(target) = kr.target_value {
                        output.push_str(&format!(
                            "Target: {}{}\n",
                            target,
                            kr.unit.as_deref().unwrap_or("")
                        ));
                    }
                } else {
                    let (total, completed) =
                        self.kr_repo.count_actions(&kr.id).await.unwrap_or((0, 0));
                    output.push_str(&format!("Actions: {}/{} completed\n", completed, total));
                }

                if let Some(ref desc) = kr.description {
                    output.push_str(&format!("Description: {}\n", desc));
                }
                if let Some(due) = kr.due_date {
                    output.push_str(&format!("Due: {}\n", due));
                }

                Ok(output)
            }

            "kr.update" => {
                let id = p.required_str("id")?;

                let due_date_opt = if args.get("due_date").is_some() {
                    let s = p.optional_str("due_date")?;
                    match s {
                        Some(s) => {
                            let dt = s.parse::<chrono::DateTime<Utc>>().map_err(|e| {
                                ToolError::InvalidParams(format!("Invalid due_date: {e}"))
                            })?;
                            Some(Some(dt))
                        }
                        None => Some(None),
                    }
                } else {
                    None
                };

                let updated = self
                    .kr_repo
                    .update(
                        id,
                        p.optional_str("title")?,
                        if args.get("description").is_some() {
                            Some(p.optional_str("description")?)
                        } else {
                            None
                        },
                        p.optional_str("status")?,
                        due_date_opt,
                    )
                    .await?;

                // Recalculate parent objective progress if status changed
                if args.get("status").is_some() {
                    let _ = self
                        .objective_repo
                        .recalculate_progress(&updated.objective_id)
                        .await;
                }

                Ok(format!(
                    "Key Result updated: {} (ID: {})",
                    updated.title, updated.id
                ))
            }

            "kr.update_metric" => {
                let id = p.required_str("id")?;
                let current_value = p.optional_f64("current_value")?.ok_or_else(|| {
                    ToolError::InvalidParams("current_value is required".to_string())
                })?;

                let updated = self.kr_repo.update_metric(id, current_value).await?;

                // Cascade progress to parent objective
                let _ = self
                    .objective_repo
                    .recalculate_progress(&updated.objective_id)
                    .await;

                Ok(format!(
                    "Key Result metric updated: {} = {}{} ({:.1}%)",
                    updated.title,
                    updated.current_value,
                    updated.unit.as_deref().unwrap_or(""),
                    updated.progress
                ))
            }

            "kr.delete" => {
                let id = p.required_str("id")?;

                // Get KR before deletion to recalculate parent objective
                let kr = self.kr_repo.get(id).await?;
                let deleted = self.kr_repo.delete(id).await?;

                if deleted {
                    if let Some(kr) = kr {
                        let _ = self
                            .objective_repo
                            .recalculate_progress(&kr.objective_id)
                            .await;
                    }
                    Ok(format!("Key Result {} deleted.", id))
                } else {
                    Err(ToolError::InvalidParams("Key result not found".to_string()).into())
                }
            }

            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_okr_tool_name() {
        let pool = sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap();
        let tool = OkrTool::new(ObjectiveRepo::new(pool.clone()), KeyResultRepo::new(pool));
        assert_eq!(tool.name(), "okr");
    }
}
