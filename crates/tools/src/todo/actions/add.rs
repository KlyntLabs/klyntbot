//! Add, add_subtask, and recur action handlers.

use crate::params::ParamExtractor;
use crate::todo_types::{Todo, TodoPatch};
use common::utils::date::parse_datetime;
use common::{Result, ToolError};
use tracing::warn;

use super::super::TodoTool;

impl TodoTool {
    pub(crate) async fn handle_add(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let title = p.required_str("title")?;

        let mut todo = Todo::default_instance();
        todo.title = title.to_string();
        todo.description = p.optional_str("description")?.map(String::from);
        todo.priority = p.optional_u64("priority")?.map(|v| v as u8);
        todo.due_date = p
            .optional_str("due_date")?
            .and_then(|s| parse_datetime(s, &self.timezone));
        todo.tags = p.string_array_or_empty("tags")?;
        todo.project_id = p.optional_str("project_id")?.map(String::from);

        let row = storage::TodoRow::from(&todo);
        let created_row = self.repo.add(&row).await?;
        let created = Todo::from(created_row);

        // Auto-enrich if handler is available
        let mut enriched_info = String::new();
        if let Some(handler) = &self.enrichment_handler {
            match handler.enrich_task(&created).await {
                Ok(Some(suggestions)) => {
                    let mut patch = TodoPatch::default();
                    let mut applied = Vec::new();

                    if let Some(ref priority_sug) = suggestions.priority {
                        if priority_sug.confidence >= 0.7 {
                            patch.priority = Some(priority_sug.value);
                            applied.push(format!("P{}", priority_sug.value));
                        }
                    }

                    if let Some(ref duration_sug) = suggestions.estimated_minutes {
                        if duration_sug.confidence >= 0.7 {
                            patch.estimated_minutes = Some(Some(duration_sug.value));
                            applied.push(format!("~{}min", duration_sug.value));
                        }
                    }

                    if let Some(ref due_sug) = suggestions.due_date {
                        if due_sug.confidence >= 0.7 {
                            patch.due_date = Some(Some(due_sug.value));
                            applied.push("due date".to_string());
                        }
                    }

                    if !applied.is_empty() {
                        let storage_patch = patch.to_storage_patch(&created.id);
                        if self.repo.update(&storage_patch).await.is_ok() {
                            enriched_info = format!(" (enriched: {})", applied.join(", "));
                        }
                    }

                    self.record_enrichment_feedback(&created.id, &suggestions, None)
                        .await;
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("Task enrichment failed: {}", e);
                }
            }
        }

        // Auto-embed (best-effort)
        if let Some(ref emb) = self.embedding_handler {
            if let Err(e) = emb.embed_todo(&created).await {
                warn!(
                    "Failed to generate embedding for todo {}: {}",
                    created.id, e
                );
            }
        }

        self.trigger_sync_async().await;

        Ok(format!(
            "Task created: {} (ID: {}){}",
            created.title, created.id, enriched_info
        ))
    }

    pub(crate) async fn handle_add_subtask(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let title = p.required_str("title")?;
        let parent_id = p.required_str("parent_id")?;

        // Verify parent exists
        if self.repo.get(parent_id).await?.is_none() {
            return Err(
                ToolError::InvalidParams(format!("Parent task not found: {}", parent_id)).into(),
            );
        }

        let mut todo = Todo::default_instance();
        todo.title = title.to_string();
        todo.description = p.optional_str("description")?.map(String::from);
        todo.priority = p.optional_u64("priority")?.map(|v| v as u8);
        todo.due_date = p
            .optional_str("due_date")?
            .and_then(|s| parse_datetime(s, &self.timezone));
        todo.tags = p.string_array_or_empty("tags")?;
        todo.parent_id = Some(parent_id.to_string());
        todo.project_id = p.optional_str("project_id")?.map(String::from);

        let row = storage::TodoRow::from(&todo);
        let created_row = self.repo.add(&row).await?;
        let created = Todo::from(created_row);
        let result = format!(
            "Subtask created: {} (ID: {}, parent: {})",
            created.title, created.id, parent_id
        );

        self.trigger_sync_async().await;

        Ok(result)
    }

    pub(crate) async fn handle_recur(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let title = p.required_str("title")?;
        let rule = p.required_str("rule")?;

        crate::rrule_utils::validate_rrule(rule)?;

        let now = chrono::Utc::now();
        let next_date = crate::rrule_utils::next_occurrence(rule, now)?;

        let mut template = Todo::default_instance();
        template.title = title.to_string();
        template.description = p.optional_str("description")?.map(String::from);
        template.priority = p.optional_u64("priority")?.map(|v| v as u8);
        template.tags = p.string_array_or_empty("tags")?;
        template.project_id = p.optional_str("project_id")?.map(String::from);
        template.recurrence_rule = Some(rule.to_string());
        template.is_template = true;
        template.next_instance_date = next_date;

        let row = storage::TodoRow::from(&template);
        let created_row = self.repo.add(&row).await?;
        let created = Todo::from(created_row);
        let human_rule = crate::rrule_utils::humanize_rrule(rule);
        let next_str = next_date
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(format!(
            "Recurring task created: {} (ID: {}, rule: {}, next: {})",
            created.title, created.id, human_rule, next_str
        ))
    }
}
