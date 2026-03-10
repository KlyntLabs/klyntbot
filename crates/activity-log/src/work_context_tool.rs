use async_trait::async_trait;
use serde_json::Value;
use storage::StoragePool;
use tools_core::params::ParamExtractor;
use tools_core::{RoutingContext, Tool};

use crate::context_resource_repo::ContextResourceRepo;
use crate::work_context_repo::WorkContextRepo;

pub struct WorkContextTool {
    pool: StoragePool,
}

impl WorkContextTool {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Tool for WorkContextTool {
    fn name(&self) -> &str {
        "work_context"
    }

    fn description(&self) -> &str {
        "Manage work contexts (inferred units of work). Actions: list, show, rename, link_project, search."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "show", "rename", "link_project", "search"],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Context ID (for show/rename/link_project)" },
                "title": { "type": "string", "description": "New title (for rename)" },
                "project_id": { "type": "string", "description": "Project ID (for link_project)" },
                "query": { "type": "string", "description": "Search query (for search)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> common::Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "list" => {
                let contexts = WorkContextRepo::list_active(&self.pool).await?;
                if contexts.is_empty() {
                    return Ok("No active work contexts.".to_string());
                }
                let mut out = format!("Active work contexts ({}):\n\n", contexts.len());
                for ctx in &contexts {
                    out.push_str(&format!(
                        "• {} [{}] ({} events, {:.1}h) ID: {}\n",
                        ctx.title,
                        ctx.context_type.as_str(),
                        ctx.event_count,
                        ctx.total_duration_secs as f64 / 3600.0,
                        ctx.id
                    ));
                }
                Ok(out)
            }
            "show" => {
                let id = p.required_str("id")?;
                let ctx = WorkContextRepo::get(&self.pool, id)
                    .await?
                    .ok_or_else(|| common::ToolError::InvalidParams("Context not found".into()))?;

                let resources = ContextResourceRepo::list_for_context(&self.pool, id).await?;
                let mut out = format!("Work Context: {}\n", ctx.title);
                out.push_str(&format!(
                    "Type: {}\nStatus: {}\n",
                    ctx.context_type.as_str(),
                    ctx.status.as_str()
                ));
                out.push_str(&format!(
                    "Events: {}\nDuration: {:.1}h\n",
                    ctx.event_count,
                    ctx.total_duration_secs as f64 / 3600.0
                ));
                if !resources.is_empty() {
                    out.push_str("\nResources:\n");
                    for (r, score) in &resources {
                        out.push_str(&format!(
                            "  • {} ({:.0}%)\n",
                            r.resource_name,
                            score * 100.0
                        ));
                    }
                }
                Ok(out)
            }
            "rename" => {
                let id = p.required_str("id")?;
                let title = p.required_str("title")?;
                let mut ctx = WorkContextRepo::get(&self.pool, id)
                    .await?
                    .ok_or_else(|| common::ToolError::InvalidParams("Context not found".into()))?;
                ctx.title = title.to_string();
                WorkContextRepo::update(&self.pool, &ctx).await?;
                Ok(format!("Context renamed to: {title}"))
            }
            "link_project" => {
                let id = p.required_str("id")?;
                let project_id = p.required_str("project_id")?;
                let mut ctx = WorkContextRepo::get(&self.pool, id)
                    .await?
                    .ok_or_else(|| common::ToolError::InvalidParams("Context not found".into()))?;
                ctx.linked_project_id = Some(project_id.to_string());
                WorkContextRepo::update(&self.pool, &ctx).await?;
                Ok(format!("Context linked to project {project_id}"))
            }
            "search" => {
                let query = p.required_str("query")?;
                let results = WorkContextRepo::search_by_title(&self.pool, query).await?;
                if results.is_empty() {
                    return Ok("No matching contexts.".to_string());
                }
                let mut out = format!("Search results ({}):\n\n", results.len());
                for ctx in &results {
                    out.push_str(&format!(
                        "• {} [{}] ID: {}\n",
                        ctx.title,
                        ctx.status.as_str(),
                        ctx.id
                    ));
                }
                Ok(out)
            }
            _ => Err(common::ToolError::InvalidParams(format!("Unknown action: {action}")).into()),
        }
    }
}
