//! Annotate tool — CRUD for persistent annotations on any entity.

use serde_json::json;
use tracing::debug;

use cognitive::repos::AnnotationRepo;
use cognitive::types::Annotation;
use common::Result;
use tools_core::{tool_actions, ActionParams};

#[derive(Debug, ActionParams)]
pub struct CreateParams {
    /// Entity type to annotate (e.g. "tool", "fact", "rule", "skill", "project")
    #[param(required)]
    pub target_type: String,
    /// Entity identifier
    #[param(required)]
    pub target_id: String,
    /// Annotation content
    #[param(required)]
    pub content: String,
    /// Comma-separated tags
    pub tags: Option<String>,
    /// Priority level (0=normal, 1=important, 2=critical)
    pub priority: Option<i32>,
    /// ISO8601 expiry timestamp (annotation auto-deletes after this)
    pub expires_at: Option<String>,
}

#[derive(Debug, ActionParams)]
pub struct GetParams {
    /// Entity type
    #[param(required)]
    pub target_type: String,
    /// Entity identifier
    #[param(required)]
    pub target_id: String,
}

#[derive(Debug, ActionParams)]
pub struct ListParams {
    /// Minimum priority filter (omit for all)
    pub min_priority: Option<i32>,
}

#[derive(Debug, ActionParams)]
pub struct DeleteParams {
    /// Annotation ID to delete
    #[param(required)]
    pub id: String,
}

#[derive(Debug, ActionParams)]
pub struct UpdateParams {
    /// Annotation ID to update
    #[param(required)]
    pub id: String,
    /// New content (replaces existing)
    pub content: Option<String>,
    /// Comma-separated tags (replaces existing)
    pub tags: Option<String>,
    /// Priority level (0=normal, 1=important, 2=critical)
    pub priority: Option<i32>,
    /// ISO8601 expiry timestamp. Pass empty string to clear.
    pub expires_at: Option<String>,
}

#[derive(Debug, ActionParams)]
pub struct SearchParams {
    /// Search query (full-text via FTS5)
    #[param(required)]
    pub query: String,
    /// Max results
    pub limit: Option<i32>,
}

pub struct AnnotateTool {
    repo: AnnotationRepo,
}

impl AnnotateTool {
    pub fn new(repo: AnnotationRepo) -> Self {
        Self { repo }
    }
}

#[tool_actions(
    ctx = "()",
    name = "annotate",
    description = "Add metadata annotations to internal system entities (tools, facts, rules, skills). For user-facing notes, use the 'notes' tool instead.",
    category = "Memory",
    tags = "annotation,note,gotcha",
    cost = "Free",
    mcp_exposure = "default"
)]
impl AnnotateTool {
    #[action(name = "create")]
    async fn handle_create(&self, params: CreateParams, _ctx: ()) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = jiff::Timestamp::now().to_string();

        let annotation = Annotation {
            id: id.clone(),
            target_type: params.target_type.clone(),
            target_id: params.target_id.clone(),
            content: params.content.clone(),
            tags: params.tags.unwrap_or_default(),
            author: "agent".into(),
            priority: params.priority.unwrap_or(0),
            created_at: now.clone(),
            updated_at: now,
            expires_at: params.expires_at,
            access_count: 0,
            mark_id: None,
            quoted_text: None,
            range_start: None,
            range_end: None,
            ai_suggestion: None,
        };

        self.repo.upsert(&annotation).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to create annotation: {e}"))
        })?;

        debug!(
            "Created annotation {} on {}:{}",
            id, params.target_type, params.target_id
        );

        Ok(json!({
            "id": id,
            "target_type": params.target_type,
            "target_id": params.target_id,
            "content": params.content,
        })
        .to_string())
    }

    #[action(name = "update")]
    async fn handle_update(&self, params: UpdateParams, _ctx: ()) -> Result<String> {
        let existing = self
            .repo
            .get_by_id(&params.id)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Failed to get annotation: {e}"))
            })?
            .ok_or_else(|| {
                common::ToolError::InvalidParams(format!("Annotation not found: {}", params.id))
            })?;

        let updated = Annotation {
            content: params.content.unwrap_or(existing.content),
            tags: params.tags.unwrap_or(existing.tags),
            priority: params.priority.unwrap_or(existing.priority),
            expires_at: match params.expires_at {
                Some(s) if s.is_empty() => None,
                Some(s) => Some(s),
                None => existing.expires_at,
            },
            updated_at: jiff::Timestamp::now().to_string(),
            ..existing
        };

        self.repo.upsert(&updated).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to update annotation: {e}"))
        })?;

        Ok(format!("Updated annotation {}", params.id))
    }

    #[action(name = "get")]
    async fn handle_get(&self, params: GetParams, _ctx: ()) -> Result<String> {
        let annotations = self
            .repo
            .get_for_target(&params.target_type, &params.target_id)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Failed to get annotations: {e}"))
            })?;

        if annotations.is_empty() {
            return Ok(format!(
                "No annotations found for {}:{}",
                params.target_type, params.target_id
            ));
        }

        Ok(format_annotations(&annotations))
    }

    #[action(name = "list")]
    async fn handle_list(&self, params: ListParams, _ctx: ()) -> Result<String> {
        let annotations = if let Some(min_p) = params.min_priority {
            self.repo.get_by_min_priority(min_p).await
        } else {
            self.repo.list_all().await
        }
        .map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to list annotations: {e}"))
        })?;

        if annotations.is_empty() {
            return Ok("No annotations found.".into());
        }

        Ok(format_annotations(&annotations))
    }

    #[action(name = "delete")]
    async fn handle_delete(&self, params: DeleteParams, _ctx: ()) -> Result<String> {
        let deleted = self.repo.delete(&params.id).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to delete annotation: {e}"))
        })?;

        if deleted {
            Ok(format!("Deleted annotation {}", params.id))
        } else {
            Ok(format!("Annotation {} not found", params.id))
        }
    }

    #[action(name = "search")]
    async fn handle_search(&self, params: SearchParams, _ctx: ()) -> Result<String> {
        let limit = params.limit.unwrap_or(10).max(0) as usize;
        let results = self.repo.search(&params.query, limit).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to search annotations: {e}"))
        })?;

        if results.is_empty() {
            return Ok(format!("No annotations matching '{}'", params.query));
        }

        Ok(format_annotations(&results))
    }
}

fn format_annotations(annotations: &[Annotation]) -> String {
    let mut out = String::new();
    for ann in annotations {
        out.push_str(&format!(
            "- [{}] {}:{} — {} (priority={}, id={})\n",
            ann.author, ann.target_type, ann.target_id, ann.content, ann.priority, ann.id,
        ));
        if !ann.tags.is_empty() {
            out.push_str(&format!("  tags: {}\n", ann.tags));
        }
        if let Some(ref exp) = ann.expires_at {
            out.push_str(&format!("  expires: {}\n", exp));
        }
        if let Some(ref quote) = ann.quoted_text {
            out.push_str(&format!("  quoted: {}\n", quote));
        }
        if let Some(ref suggestion) = ann.ai_suggestion {
            out.push_str(&format!("  ai_suggestion: {}\n", suggestion));
        }
    }
    out
}

#[cfg(test)]
mod exposure_tests {
    use super::*;
    use tools_core::{McpExposure, Tool};

    #[tokio::test]
    async fn historical_mcp_default() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let tool = AnnotateTool::new(AnnotationRepo::new(pool.inner().clone()));
        let policy = tool.exposure_policy();
        assert_eq!(tool.name(), "annotate");
        assert_eq!(policy.mcp, McpExposure::Default);
        assert!(!policy.subagent);
        assert_eq!(tool.allowed_channels(), policy.llm_channels);
        assert!(!tool.subagent_visible());
    }
}
