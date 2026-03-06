//! Add, add_subtask, and recur action handlers.

use chrono::Utc;
use common::{Result, ToolError};
use tools_core::{ParamExtractor, RoutingContext};
use tracing::warn;

use super::super::TaskTool;
use crate::types::Action;
use storage::ActionPatch;

impl TaskTool {
    pub(crate) async fn handle_add(
        &self,
        p: &ParamExtractor<'_>,
        ctx: &RoutingContext,
    ) -> Result<String> {
        let title = p.required_str("title")?;
        let area_id = p.required_str("area_id")?;

        let mut action = Action::default_instance();
        action.title = title.to_string();
        action.area_id = area_id.to_string();
        action.key_result_id = p.optional_str("key_result_id")?.map(String::from);
        action.description = p.optional_str("description")?.map(String::from);
        action.priority = p.optional_u64("priority")?.map(|v| v as u8);
        action.due_date = p
            .optional_str("due_date")?
            .and_then(|s| common::utils::date::parse_datetime(s, &self.timezone));
        action.tags = p.string_array_or_empty("tags")?;
        action.project_id = p.optional_str("project_id")?.map(String::from);

        let row = storage::ActionRow::from(&action);
        let created_row = self.repo.add(&row).await?;
        let created = Action::from(created_row);

        // Auto-enrich if handler is available
        let mut enriched_info = String::new();
        if let Some(handler) = &self.enrichment_handler {
            match handler.enrich_task(&created).await {
                Ok(Some(suggestions)) => {
                    let threshold = self.enrichment_threshold;
                    let mut update_priority: Option<Option<i16>> = None;
                    let mut update_due: Option<Option<chrono::DateTime<Utc>>> = None;
                    let mut update_est: Option<Option<i32>> = None;
                    let mut applied = Vec::new();

                    if let Some(ref priority_sug) = suggestions.priority {
                        if priority_sug.confidence >= threshold {
                            update_priority = Some(Some(priority_sug.value as i16));
                            applied.push(format!("P{}", priority_sug.value));
                        }
                    }
                    if let Some(ref duration_sug) = suggestions.estimated_minutes {
                        if duration_sug.confidence >= threshold {
                            update_est = Some(Some(duration_sug.value as i32));
                            applied.push(format!("~{}min", duration_sug.value));
                        }
                    }
                    if let Some(ref due_sug) = suggestions.due_date {
                        if due_sug.confidence >= threshold {
                            update_due = Some(Some(due_sug.value));
                            applied.push("due date".to_string());
                        }
                    }

                    if !applied.is_empty() {
                        let patch = ActionPatch {
                            id: created.id.clone(),
                            title: None,
                            description: None,
                            priority: update_priority,
                            due_date: update_due,
                            tags: None,
                            status: None,
                            calendar_event_uid: None,
                            next_instance_date: None,
                            last_reminded_at: None,
                            estimated_minutes: update_est,
                            recurrence_rule: None,
                            area_id: None,
                            project_id: None,
                            key_result_id: None,
                        };
                        if self.repo.update(&patch).await.is_ok() {
                            enriched_info = format!(" (enriched: {})", applied.join(", "));
                        }
                    }
                    self.record_enrichment_feedback(&created.id, &suggestions, None)
                        .await;
                }
                Ok(None) => {}
                Err(e) => warn!("Task enrichment failed: {}", e),
            }
        }

        // Auto-embed (best-effort)
        if let Some(ref emb) = self.embedding_handler {
            if let Err(e) = emb.embed_todo(&created).await {
                warn!(
                    "Failed to generate embedding for action {}: {}",
                    created.id, e
                );
            }
        }

        // Emit entity card for inline display
        if let Some(ref tx) = ctx.entity_tx {
            let subtitle = {
                let mut parts = Vec::new();
                if let Some(p) = created.priority {
                    parts.push(format!("P{}", p));
                }
                if let Some(ref d) = created.due_date {
                    parts.push(format!("Due {}", d.format("%b %d")));
                }
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join(" · "))
                }
            };
            let _ = tx
                .send(common::EntityCard {
                    entity_type: "task".to_string(),
                    entity_id: created.id.clone(),
                    title: created.title.clone(),
                    subtitle,
                    route: Some(format!("/tasks/{}", created.id)),
                    icon_hint: "task".to_string(),
                    metadata: std::collections::HashMap::new(),
                })
                .await;
        }

        Ok(format!(
            "Task created: {} (ID: {}){}",
            created.title, created.id, enriched_info
        ))
    }

    pub(crate) async fn handle_add_subtask(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let title = p.required_str("title")?;
        let parent_id = p.required_str("parent_id")?;

        // Verify parent exists and inherit area_id
        let parent = self.repo.get(parent_id).await?.ok_or_else(|| {
            ToolError::InvalidParams(format!("Parent task not found: {}", parent_id))
        })?;

        let mut action = Action::default_instance();
        action.title = title.to_string();
        action.area_id = parent.area_id.clone();
        action.key_result_id = parent.key_result_id.clone();
        action.description = p.optional_str("description")?.map(String::from);
        action.priority = p.optional_u64("priority")?.map(|v| v as u8);
        action.due_date = p
            .optional_str("due_date")?
            .and_then(|s| common::utils::date::parse_datetime(s, &self.timezone));
        action.tags = p.string_array_or_empty("tags")?;
        action.parent_id = Some(parent_id.to_string());
        action.project_id = p
            .optional_str("project_id")?
            .map(String::from)
            .or_else(|| parent.project_id.clone());

        let row = storage::ActionRow::from(&action);
        let created_row = self.repo.add(&row).await?;
        let created = Action::from(created_row);

        Ok(format!(
            "Subtask created: {} (ID: {}, parent: {})",
            created.title, created.id, parent_id
        ))
    }

    pub(crate) async fn handle_recur(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let title = p.required_str("title")?;
        let rule_str = p.required_str("rule")?;
        let area_id = p.required_str("area_id")?;

        // Validate the RRULE
        crate::rrule_utils::validate_rrule(rule_str)?;

        // Compute next instance date
        let now = Utc::now();
        let next_instance_date = crate::rrule_utils::next_occurrence(rule_str, now)?;

        let mut template = Action::default_instance();
        template.title = title.to_string();
        template.area_id = area_id.to_string();
        template.key_result_id = p.optional_str("key_result_id")?.map(String::from);
        template.description = p.optional_str("description")?.map(String::from);
        template.priority = p.optional_u64("priority")?.map(|v| v as u8);
        template.tags = p.string_array_or_empty("tags")?;
        template.project_id = p.optional_str("project_id")?.map(String::from);
        template.recurrence_rule = Some(rule_str.to_string());
        template.is_template = true;
        template.next_instance_date = next_instance_date;

        let row = storage::ActionRow::from(&template);
        let created_row = self.repo.add_template(&row).await?;
        let created = Action::from(created_row);

        let human = crate::rrule_utils::humanize_rrule(rule_str);
        let next_str = created
            .next_instance_date
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "none".to_string());

        Ok(format!(
            "Recurring template created: {} (ID: {}) | {} | Next: {}",
            created.title, created.id, human, next_str
        ))
    }
}
