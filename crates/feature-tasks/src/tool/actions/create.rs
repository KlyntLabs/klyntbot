//! Create action handler.

use common::{Result, ToolError};
use tools_core::{ParamExtractor, RoutingContext};
use tracing::warn;

use bus::DomainEvent;

use super::super::TaskTool;
use crate::types::Task;
use storage::TaskPatch;

impl TaskTool {
    pub(crate) async fn handle_create(
        &self,
        p: &ParamExtractor<'_>,
        ctx: &RoutingContext,
    ) -> Result<String> {
        let title = p.required_str("title")?;
        let area_id = p.required_str("area_id")?;

        let mut task = Task::default_instance();
        task.title = title.to_string();
        task.area_id = area_id.to_string();
        task.key_result_id = p.optional_str("key_result_id")?.map(String::from);
        task.objective_id = p.optional_str("objective_id")?.map(String::from);
        task.description = p.optional_str("description")?.map(String::from);
        task.priority = p.optional_u64("priority")?.map(|v| v as i16);
        task.due_date = p
            .optional_str("due_date")?
            .and_then(|s| common::utils::date::parse_datetime(s, &self.timezone));
        task.tags = p.string_array_or_empty("tags")?;
        task.project_id = p.optional_str("project_id")?.map(String::from);
        task.parent_id = p.optional_str("parent_id")?.map(String::from);
        task.estimated_minutes = p.optional_u64("estimated_minutes")?.map(|v| v as i32);

        // New agentic fields
        if let Some(tt) = p.optional_str("task_type")? {
            task.task_type = tt
                .parse()
                .map_err(|e: String| ToolError::InvalidParams(e))?;
        }
        task.acceptance_criteria = p.optional_str("acceptance_criteria")?.map(String::from);
        if let Some(el) = p.optional_str("energy_level")? {
            task.energy_level = Some(
                el.parse()
                    .map_err(|e: String| ToolError::InvalidParams(e))?,
            );
        }
        if let Some(ac) = p.optional_str("agent_config")? {
            task.agent_config = serde_json::from_str(ac).ok();
        }

        let row = storage::TaskRow::from(&task);
        let created_row = self.repo.add(&row).await?;
        let created = Task::from(created_row);

        // Auto-enrich if handler is available
        let mut enriched_info = String::new();
        if let Some(handler) = &self.enrichment_handler {
            match handler.enrich(&created).await {
                Ok(Some(suggestions)) => {
                    let threshold = self.enrichment_threshold;
                    let mut update_priority: Option<Option<i16>> = None;
                    let update_est: Option<Option<i32>> = None;
                    let mut update_energy: Option<Option<String>> = None;
                    let mut applied = Vec::new();

                    if let Some(ref priority_sug) = suggestions.priority {
                        if priority_sug.confidence >= threshold {
                            update_priority = Some(Some(priority_sug.value));
                            applied.push(format!("P{}", priority_sug.value));
                        }
                    }

                    if let Some(ref energy_sug) = suggestions.energy_level {
                        if energy_sug.confidence >= threshold {
                            update_energy = Some(Some(energy_sug.value.to_string()));
                            applied.push(format!("energy:{}", energy_sug.value));
                        }
                    }

                    if !applied.is_empty() {
                        let patch = TaskPatch {
                            id: created.id.clone(),
                            priority: update_priority,
                            estimated_minutes: update_est,
                            energy_level: update_energy,
                            ..Default::default()
                        };
                        if self.repo.update(&patch).await.is_ok() {
                            enriched_info = format!(" (enriched: {})", applied.join(", "));
                        }
                    }

                    // Log activity if configured
                    if self.config.auto_log_activity && !applied.is_empty() {
                        let _ = self
                            .repo
                            .log_activity(
                                &created.id,
                                "enrichment",
                                None,
                                None,
                                Some(&applied.join(", ")),
                                "system",
                                Some("Auto-enrichment applied on creation"),
                            )
                            .await;
                    }
                }
                Ok(None) => {}
                Err(e) => warn!("Task enrichment failed: {}", e),
            }
        }

        // Auto-embed (best-effort)
        if let Some(ref emb) = self.embedding_handler {
            if let Err(e) = emb.embed_task(&created).await {
                warn!(
                    "Failed to generate embedding for task {}: {}",
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
                if created.task_type != crate::types::TaskType::Manual {
                    parts.push(format!("{}", created.task_type));
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

        if let Some(ref bus) = self.domain_bus {
            bus.publish(DomainEvent::TaskCreated {
                task_id: created.id.clone(),
                project: created.project_id.clone(),
                estimate_mins: created.estimated_minutes.map(|m| m as i64),
                task_type: created.task_type.to_string(),
            });
        }

        Ok(format!(
            "Task created: {} (ID: {}, type: {}){}",
            created.title, created.id, created.task_type, enriched_info
        ))
    }
}
