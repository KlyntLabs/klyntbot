use crate::{
    repos::{AccumulatedObservationRepo, EntityRepo, EpisodicMemoryRepo},
    services::extraction::ExtractionHandler,
    types::Observation,
};
use ai_core::{AiSignal, SalienceVerdict, SignalConsumer};
use async_trait::async_trait;
use std::sync::Arc;

fn map_sqlx(e: sqlx::Error) -> common::KlyntbotError {
    common::KlyntbotError::Storage(e.to_string())
}

pub struct IngestionConsumer {
    observation_repo: AccumulatedObservationRepo,
    entity_repo: EntityRepo,
    episodic_repo: EpisodicMemoryRepo,
    extraction_handler: Option<Arc<dyn ExtractionHandler>>,
    episodic_importance_threshold: f64,
}

impl IngestionConsumer {
    pub fn new(
        observation_repo: AccumulatedObservationRepo,
        entity_repo: EntityRepo,
        episodic_repo: EpisodicMemoryRepo,
        extraction_handler: Option<Arc<dyn ExtractionHandler>>,
    ) -> Self {
        Self {
            observation_repo,
            entity_repo,
            episodic_repo,
            extraction_handler,
            episodic_importance_threshold: 0.7,
        }
    }

    fn signal_to_observation(signal: &AiSignal) -> Observation {
        Observation {
            domain: signal.domain.as_str().to_string(),
            content: signal.content.clone(),
            importance: signal.importance,
            source_event: signal.event_kind.to_string(),
            timestamp: signal.timestamp,
        }
    }
}

#[async_trait]
impl SignalConsumer for IngestionConsumer {
    fn name(&self) -> &'static str {
        "cognitive_ingestion"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        // 1. Entity bridge (always, regardless of salience).
        if let Some(entity) = &signal.entity {
            let _ = self
                .entity_repo
                .upsert_entity(&crate::repos::NewEntity {
                    name: entity.name.clone(),
                    entity_type: entity.entity_type.to_string(),
                    description: None,
                    source: "signal".to_string(),
                    source_id: Some(entity.id.clone()),
                    metadata: None,
                })
                .await;
        }

        // 2. Salience routing.
        let observation = Self::signal_to_observation(signal);
        match signal.salience {
            SalienceVerdict::Discard => return Ok(()),
            SalienceVerdict::Accumulate => {
                self.observation_repo
                    .insert(signal.event_kind, &observation)
                    .await;
            }
            SalienceVerdict::Extract => {
                self.observation_repo
                    .insert(signal.event_kind, &observation)
                    .await;
                if let Some(handler) = &self.extraction_handler {
                    let handler = handler.clone();
                    let obs = observation.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handler.extract_facts_batch(&[obs]).await {
                            tracing::warn!(error = %e, "extraction failed");
                        }
                    });
                }
            }
        }

        // 3. Episodic branch for high-importance signals.
        if observation.importance >= self.episodic_importance_threshold {
            let mem = crate::types::EpisodicMemory {
                id: uuid::Uuid::new_v4().to_string(),
                domain: observation.domain.clone(),
                content: observation.content.clone(),
                summary: None,
                importance: observation.importance,
                occurred_at: observation.timestamp.to_string(),
                recorded_at: jiff::Timestamp::now().to_string(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "system".to_string(),
                scope_id: None,
            };
            self.episodic_repo.insert(&mem).await.map_err(map_sqlx)?;
        }

        Ok(())
    }
}
