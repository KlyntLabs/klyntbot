//! Suggest action: generate, list, apply, and dismiss proactive suggestions.

use common::{time::bridge::chrono_to_jiff, Result};
use tools_core::ParamExtractor;
use tracing::info;

use super::super::TaskTool;

impl TaskTool {
    pub(crate) async fn handle_suggest(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.proactive_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Proactive handler not available".into())
        })?;

        let scope = crate::types::SuggestionScope {
            project_id: p.optional_str("project_id")?.map(String::from),
            area_id: p.optional_str("area_id")?.map(String::from),
            tags: vec![],
        };

        info!(?scope, "Generating proactive suggestions");
        let candidates = handler.suggest(&scope).await?;

        if candidates.is_empty() {
            return Ok("No suggestions at this time.".into());
        }

        // Persist suggestions
        for c in &candidates {
            let row = storage::TaskSuggestionRow {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: c.task_id.clone(),
                suggestion_type: format!("{:?}", c.suggestion_type),
                title: c.title.clone(),
                description: c.description.clone(),
                confidence: c.confidence,
                action_payload: c
                    .action
                    .as_ref()
                    .and_then(|a| serde_json::to_string(a).ok()),
                status: "pending".into(),
                trigger: c.trigger.as_ref().map(|t| format!("{t:?}")),
                created_at: chrono_to_jiff(chrono::Utc::now()).into(),
                resolved_at: None,
            };
            self.repo.create_suggestion(&row).await?;
        }

        // Format response
        let mut output = format!("Generated {} suggestions:\n\n", candidates.len());
        for (i, c) in candidates.iter().enumerate() {
            output.push_str(&format!(
                "{}. **{}** (confidence: {:.0}%)\n   {}\n",
                i + 1,
                c.title,
                c.confidence * 100.0,
                c.description.as_deref().unwrap_or(""),
            ));
        }
        Ok(output)
    }

    pub(crate) async fn handle_apply_suggestion(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let applier = self.suggestion_applier.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Suggestion applier not available".into())
        })?;

        let suggestion_id = p.required_str("suggestion_id")?;

        // Load the suggestion by ID
        let suggestion = self
            .repo
            .get_pending_suggestion(suggestion_id)
            .await?
            .ok_or_else(|| {
                common::ToolError::ExecutionFailed(format!(
                    "Suggestion {suggestion_id} not found or not pending"
                ))
            })?;

        let action: crate::types::SuggestionAction = suggestion
            .action_payload
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| common::ToolError::ExecutionFailed(format!("Invalid action: {e}")))?
            .ok_or_else(|| common::ToolError::ExecutionFailed("No action payload".into()))?;

        let summary = applier
            .apply(suggestion_id, suggestion.task_id.as_deref(), &action)
            .await?;

        Ok(format!("Applied suggestion: {summary}"))
    }

    pub(crate) async fn handle_dismiss_suggestion(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let suggestion_id = p.required_str("suggestion_id")?;
        self.repo
            .resolve_suggestion(suggestion_id, "dismissed")
            .await?;
        Ok(format!("Dismissed suggestion {suggestion_id}"))
    }

    pub(crate) async fn handle_list_suggestions(&self, _p: &ParamExtractor<'_>) -> Result<String> {
        let suggestions = self.repo.list_pending_suggestions(None).await?;
        if suggestions.is_empty() {
            return Ok("No pending suggestions.".into());
        }

        let mut output = format!("{} pending suggestions:\n\n", suggestions.len());
        for s in &suggestions {
            output.push_str(&format!(
                "- [{}] **{}** (confidence: {:.0}%)\n  {}\n",
                s.id,
                s.title,
                s.confidence * 100.0,
                s.description.as_deref().unwrap_or(""),
            ));
        }
        Ok(output)
    }
}
