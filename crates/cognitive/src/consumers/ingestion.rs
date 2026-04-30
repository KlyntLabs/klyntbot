use crate::{
    repos::{AccumulatedObservationRepo, EntityRepo, EpisodicMemoryRepo, SemanticFactRepo},
    services::extraction::{to_semantic_fact, ExtractionHandler},
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
    fact_repo: Option<SemanticFactRepo>,
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
            fact_repo: None,
            episodic_importance_threshold: 0.7,
        }
    }

    /// Wire the semantic fact repo so extracted facts are persisted (not
    /// just computed). Without this, `extract_facts_batch` runs but its
    /// result is discarded — facts never reach `semantic_facts`.
    pub fn with_fact_repo(mut self, fact_repo: SemanticFactRepo) -> Self {
        self.fact_repo = Some(fact_repo);
        self
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

        // 2. Salience routing. Discard exits before allocating an Observation.
        if matches!(signal.salience, SalienceVerdict::Discard) {
            return Ok(());
        }

        let observation = Self::signal_to_observation(signal);
        self.observation_repo
            .insert(signal.event_kind, &observation)
            .await;
        if matches!(signal.salience, SalienceVerdict::Extract) {
            tracing::info!(
                event_kind = signal.event_kind,
                content_len = signal.content.len(),
                handler_set = self.extraction_handler.is_some(),
                fact_repo_set = self.fact_repo.is_some(),
                "ingestion: Extract salience routing to LLM extractor"
            );
            if let Some(handler) = &self.extraction_handler {
                let handler = handler.clone();
                let obs = observation.clone();
                let fact_repo = self.fact_repo.clone();
                tokio::spawn(async move {
                    match handler.extract_facts_batch(&[obs.clone()]).await {
                        Ok(result) => {
                            let n_facts: usize = result.extractions.iter().map(|e| e.facts.len()).sum();
                            tracing::info!(
                                n_extractions = result.extractions.len(),
                                n_facts,
                                "ingestion: LLM extraction completed"
                            );
                            // Persist extracted facts. Without this, the
                            // extraction work is wasted — facts only
                            // exist in memory and never reach the FTS5
                            // index used by recall.
                            if let Some(repo) = fact_repo {
                                // Cross-turn identity binding. The LLM's
                                // in-batch `bind_user_identity` only mirrors
                                // user-subject facts to a proper noun when
                                // the name is declared in the SAME batch.
                                // After turn 0 stores `user→name=Alice`, any
                                // turn-1 fact like `user→lives_in=SF` would
                                // never get mirrored to `Alice→lives_in=SF`
                                // — and the bench query "Where does Alice
                                // live?" would miss. Look up the persisted
                                // user-name once per spawn and mirror.
                                let user_name = repo
                                    .find_by_subject_predicate("user", "name")
                                    .await
                                    .ok()
                                    .and_then(|rows| {
                                        rows.into_iter()
                                            .find(|f| f.superseded_at.is_none())
                                            .map(|f| f.object)
                                    });
                                let mut written = 0usize;
                                for ext in &result.extractions {
                                    for f in &ext.facts {
                                        let fact = to_semantic_fact(f, &obs);
                                        match repo.upsert(&fact).await {
                                            Ok(()) => written += 1,
                                            Err(e) => tracing::warn!(error = %e, "fact upsert failed"),
                                        }
                                        // Mirror to proper-noun subject when
                                        // the original is a first-person fact
                                        // and a name binding already exists.
                                        // Skip the name predicate itself — it
                                        // would create `Alice→name=Alice`.
                                        if fact.subject == "user"
                                            && fact.predicate != "name"
                                        {
                                            if let Some(name) = &user_name {
                                                let mut mirrored = fact.clone();
                                                mirrored.subject = name.clone();
                                                mirrored.id = uuid::Uuid::new_v4().to_string();
                                                match repo.upsert(&mirrored).await {
                                                    Ok(()) => written += 1,
                                                    Err(e) => tracing::warn!(
                                                        error = %e,
                                                        "mirrored fact upsert failed"
                                                    ),
                                                }
                                            }
                                        }
                                    }
                                }
                                tracing::info!(written, "ingestion: facts persisted");
                            } else {
                                tracing::warn!("ingestion: no fact_repo wired, facts dropped");
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, "extraction failed"),
                    }
                });
            } else {
                tracing::warn!("ingestion: Extract salience but no handler wired");
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
                kind: None,
                scope_repo_id: None,
                metadata: None,
                actor_id: None,
                tier: "raw".to_string(),
                parent_id: None,
                child_count: 0,
                rolled_up_at: None,
            };
            self.episodic_repo.insert(&mem).await.map_err(map_sqlx)?;
        }

        Ok(())
    }
}
