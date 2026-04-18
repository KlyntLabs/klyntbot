use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use bus::{DomainEvent, DomainEventBus};
use feature_tasks::types::SuggestionScope;
use feature_tasks::{
    handlers::suggestion_applier::SuggestionApplier, ProactiveHandler, TasksConfig,
};
use storage::{Repos, TaskSuggestionRow};

/// Run a full proactive suggestion scan: call `suggest()`, persist each candidate,
/// emit `ProactiveSuggestionCreated` with the DB row ID, and auto-apply high-confidence
/// suggestions.
pub async fn run_proactive_scan(
    handler: &Arc<dyn ProactiveHandler>,
    repos: &Repos,
    domain_bus: &Option<Arc<DomainEventBus>>,
    tasks_config: &TasksConfig,
    suggestion_applier: &Option<Arc<dyn SuggestionApplier>>,
) -> common::Result<usize> {
    let candidates = handler.suggest(&SuggestionScope::default()).await?;
    let mut persisted = 0;

    for candidate in &candidates {
        let row = TaskSuggestionRow {
            id: Uuid::new_v4().to_string(),
            task_id: candidate.task_id.clone(),
            suggestion_type: format!("{:?}", candidate.suggestion_type),
            title: candidate.title.clone(),
            description: candidate.description.clone(),
            confidence: candidate.confidence,
            action_payload: candidate
                .action
                .as_ref()
                .and_then(|a| serde_json::to_string(a).ok()),
            status: "pending".to_string(),
            trigger: candidate.trigger.as_ref().map(|t| format!("{t:?}")),
            created_at: jiff::Timestamp::now().into(),
            resolved_at: None,
        };

        match repos.tasks.create_suggestion(&row).await {
            Ok(created) => {
                if let Some(bus) = domain_bus {
                    bus.publish(DomainEvent::ProactiveSuggestionCreated {
                        suggestion_id: created.id.clone(),
                        suggestion_type: created.suggestion_type.clone(),
                        task_id: created.task_id.clone(),
                        confidence: created.confidence,
                    });
                }

                // Auto-apply if confidence meets threshold
                if candidate.confidence >= tasks_config.suggestion_auto_apply_threshold {
                    if let (Some(applier), Some(action)) = (suggestion_applier, &candidate.action) {
                        match applier
                            .apply(&created.id, created.task_id.as_deref(), action)
                            .await
                        {
                            Ok(_) => {
                                let _ = repos
                                    .tasks
                                    .resolve_suggestion(&created.id, "autoapplied")
                                    .await;
                                info!(
                                    "Auto-applied suggestion {} ({}) for task {:?} (confidence {:.2})",
                                    created.id,
                                    created.suggestion_type,
                                    created.task_id,
                                    candidate.confidence
                                );
                            }
                            Err(e) => {
                                warn!("Auto-apply failed for suggestion {}: {e}", created.id)
                            }
                        }
                    }
                }

                persisted += 1;
            }
            Err(e) => warn!("Failed to persist proactive suggestion: {e}"),
        }
    }

    Ok(persisted)
}
