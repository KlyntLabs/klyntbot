//! Background consolidation service for accumulated observations.
//!
//! Subscribes to `DomainEventBus`, applies salience filtering, and routes
//! events through extraction → consolidation. Accumulated events are
//! buffered and promoted to extraction when patterns emerge.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::DomainEvent;

use futures_util::future::join_all;

use crate::consolidation::ConsolidationHandler;
use crate::embedder::SemanticFactEmbedder;
use crate::extraction::ExtractionHandler;
use crate::repos::accumulated_observation::AccumulatedObservationRepo;
use crate::repos::failed_observation::FailedObservationRepo;
use crate::repos::{EpisodicMemoryRepo, SemanticFactRepo};
use crate::salience::{evaluate_salience, HIGH_DEVIATION_THRESHOLD};
use crate::types::{EpisodicMemory, Observation, SalienceVerdict};

/// Debug events emitted by the pipeline for the debug dashboard.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PipelineEvent {
    /// A new batch processing cycle started.
    BatchStarted {
        #[serde(rename = "observationCount")]
        observation_count: usize,
    },
    /// An extraction step completed.
    Extraction {
        observation: String,
        #[serde(rename = "factsExtracted")]
        facts_extracted: usize,
        #[serde(rename = "usedFallback")]
        used_fallback: bool,
    },
    /// A consolidation operation was performed.
    Consolidation { operation: String, fact: String },
    /// An observation was queued in the dead-letter table.
    DeadLetterQueued {
        observation: String,
        #[serde(rename = "failureReason")]
        failure_reason: String,
    },
    /// A dead-letter observation was successfully reprocessed.
    DeadLetterReprocessed {
        observation: String,
        #[serde(rename = "factsExtracted")]
        facts_extracted: usize,
    },
}

/// Tracks accumulated events for pattern promotion.
#[derive(Debug, Clone)]
struct AccumulatedEntry {
    observations: Vec<Observation>,
    days_seen: HashSet<String>,
}

impl AccumulatedEntry {
    fn new() -> Self {
        Self {
            observations: Vec::new(),
            days_seen: HashSet::new(),
        }
    }

    fn add(&mut self, obs: Observation) {
        let day = obs.timestamp.format("%Y-%m-%d").to_string();
        self.days_seen.insert(day);
        self.observations.push(obs);
    }

    fn should_promote(&self, promote_threshold: usize, min_days: usize) -> bool {
        self.observations.len() >= promote_threshold && self.days_seen.len() >= min_days
    }
}

/// Collect domain events into a batch.
///
/// Waits up to `timeout` for events, returns once either `max_events` are
/// collected or the timer fires. Handles broadcast lag and channel close gracefully.
async fn collect_batch(
    event_rx: &mut broadcast::Receiver<DomainEvent>,
    cancel: &CancellationToken,
    timeout: std::time::Duration,
    max_events: usize,
) -> Vec<DomainEvent> {
    let mut batch = Vec::new();
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        if batch.len() >= max_events {
            break;
        }
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = &mut deadline => break,
            result = event_rx.recv() => {
                match result {
                    Ok(event) => batch.push(event),
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("BackgroundConsolidation lagged, skipped {n} events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    batch
}

/// Split a batch of domain events into extraction vs accumulation buckets.
fn classify_batch(events: Vec<DomainEvent>) -> (Vec<Observation>, Vec<(String, Observation)>) {
    let mut to_extract = Vec::new();
    let mut to_accumulate = Vec::new();

    for event in events {
        let verdict = evaluate_salience(&event);
        let key = event_type_key(&event);
        if let Some(obs) = event_to_observation(&event) {
            match verdict {
                SalienceVerdict::Extract => to_extract.push(obs),
                SalienceVerdict::Accumulate => to_accumulate.push((key, obs)),
                SalienceVerdict::Discard => {}
            }
        }
    }
    (to_extract, to_accumulate)
}

/// For each extracted fact, concurrently look up existing similar facts from the repo.
async fn prefetch_existing(
    extractions: &[crate::extraction::BatchExtraction],
    observations: &[Observation],
    repo: &SemanticFactRepo,
) -> Vec<crate::consolidation::ConsolidationCandidate> {
    let mut all_facts: Vec<(crate::types::SemanticFact, _)> = Vec::new();

    for batch_ext in extractions {
        let Some(obs) = observations.get(batch_ext.observation_index) else {
            warn!(
                "Skipping extraction with out-of-bounds observation_index {}",
                batch_ext.observation_index
            );
            continue;
        };
        for extracted in &batch_ext.facts {
            let fact = crate::extraction::to_semantic_fact(extracted, obs);
            let subject = fact.subject.clone();
            let predicate = fact.predicate.clone();
            let repo = repo.clone();
            let fut = Box::pin(async move {
                repo.find_similar(&subject, &predicate)
                    .await
                    .unwrap_or_default()
            });
            all_facts.push((fact, fut));
        }
    }

    let (facts, futs): (Vec<_>, Vec<_>) = all_facts.into_iter().unzip();
    let existing_results = join_all(futs).await;

    facts
        .into_iter()
        .zip(existing_results)
        .map(
            |(candidate, existing)| crate::consolidation::ConsolidationCandidate {
                candidate,
                existing,
            },
        )
        .collect()
}

/// Configuration for starting the background consolidation service (avoids too-many-arguments).
pub struct BackgroundServiceConfig {
    pub event_rx: broadcast::Receiver<DomainEvent>,
    pub extraction: Arc<dyn ExtractionHandler>,
    pub consolidation: Arc<dyn ConsolidationHandler>,
    pub repo: SemanticFactRepo,
    pub episodic_repo: Option<EpisodicMemoryRepo>,
    pub embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    pub cancel: CancellationToken,
    pub pipeline_tx: Option<tokio::sync::broadcast::Sender<PipelineEvent>>,
    pub accum_repo: Option<AccumulatedObservationRepo>,
    pub failed_obs_repo: Option<FailedObservationRepo>,
    pub promote_threshold: usize,
    pub min_days: usize,
    pub domain_bus: Option<Arc<bus::DomainEventBus>>,
}

/// Background service that processes domain events into cognitive memory.
pub struct BackgroundConsolidationService {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl BackgroundConsolidationService {
    /// Start the background service.
    pub fn start(config: BackgroundServiceConfig) -> Self {
        let BackgroundServiceConfig {
            mut event_rx,
            extraction,
            consolidation,
            repo,
            episodic_repo,
            embedder,
            cancel,
            pipeline_tx,
            accum_repo,
            failed_obs_repo,
            promote_threshold,
            min_days,
            domain_bus,
        } = config;
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            // Restore accumulated entries from previous session
            let mut accumulator: HashMap<String, AccumulatedEntry> =
                if let Some(ref ar) = accum_repo {
                    let persisted = ar.load_all().await;
                    let mut map = HashMap::new();
                    for (key, entry) in persisted {
                        map.insert(
                            key,
                            AccumulatedEntry {
                                observations: entry.observations,
                                days_seen: entry.days_seen,
                            },
                        );
                    }
                    if !map.is_empty() {
                        info!(
                            "Restored {} accumulated event type(s) from previous session",
                            map.len()
                        );
                    }
                    map
                } else {
                    HashMap::new()
                };

            // DLQ retries — paired vecs (observation + row ID at same index)
            let mut dlq_reprocess_queue: Vec<Observation> = Vec::new();
            let mut dlq_reprocess_ids: Vec<String> = Vec::new();
            // Accumulator promotions — separate from DLQ to avoid index corruption
            let mut promotion_queue: Vec<Observation> = Vec::new();

            let session_start = chrono::Utc::now().to_rfc3339();

            loop {
                // Collect batch (3s window, max 10 events)
                let batch = collect_batch(
                    &mut event_rx,
                    &cancel_clone,
                    std::time::Duration::from_secs(3),
                    10,
                )
                .await;

                if cancel_clone.is_cancelled() && batch.is_empty() {
                    break;
                }
                if batch.is_empty() {
                    continue;
                }

                let (mut to_extract, to_accumulate) = classify_batch(batch);

                // Prepend DLQ retries at indices 0..dlq_count, then promotions, then new events
                let dlq_ids_this_batch = std::mem::take(&mut dlq_reprocess_ids);
                let dlq_items = std::mem::take(&mut dlq_reprocess_queue);
                let promotion_items = std::mem::take(&mut promotion_queue);
                let dlq_count = dlq_ids_this_batch.len();
                if !dlq_items.is_empty() || !promotion_items.is_empty() {
                    let mut combined = dlq_items;
                    combined.extend(promotion_items);
                    combined.append(&mut to_extract);
                    to_extract = combined;
                }

                if !to_extract.is_empty() {
                    if let Some(tx) = &pipeline_tx {
                        let _ = tx.send(PipelineEvent::BatchStarted {
                            observation_count: to_extract.len(),
                        });
                    }

                    // Batch extraction
                    let result = match extraction.extract_facts_batch(&to_extract).await {
                        Ok(r) => r,
                        Err(e) => {
                            warn!("Batch extraction failed: {e}");
                            continue;
                        }
                    };

                    // Build fallback set once for O(1) lookups
                    let fallback_set: std::collections::HashSet<usize> =
                        result.fallback_indices.iter().copied().collect();

                    // Emit extraction pipeline events
                    if let Some(tx) = &pipeline_tx {
                        for ext in &result.extractions {
                            if let Some(obs) = to_extract.get(ext.observation_index) {
                                let _ = tx.send(PipelineEvent::Extraction {
                                    observation: obs.content.clone(),
                                    facts_extracted: ext.facts.len(),
                                    used_fallback: fallback_set.contains(&ext.observation_index),
                                });
                            }
                        }
                    }

                    // Resolve DLQ items from previous cycle that were in this batch
                    // DLQ items were prepended at indices 0..dlq_count
                    if let Some(ref dlq) = failed_obs_repo {
                        for (i, dlq_id) in dlq_ids_this_batch.iter().enumerate() {
                            if fallback_set.contains(&i) {
                                // Still failing — increment retry count
                                dlq.mark_failed(dlq_id).await;
                            } else {
                                // Successfully reprocessed by LLM
                                dlq.mark_succeeded(dlq_id).await;
                                if let Some(tx) = &pipeline_tx {
                                    if let Some(obs) = to_extract.get(i) {
                                        let facts_count = result
                                            .extractions
                                            .iter()
                                            .find(|e| e.observation_index == i)
                                            .map(|e| e.facts.len())
                                            .unwrap_or(0);
                                        let _ = tx.send(PipelineEvent::DeadLetterReprocessed {
                                            observation: obs.content.clone(),
                                            facts_extracted: facts_count,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Queue NEW fallback observations to dead-letter (skip DLQ items — already tracked above)
                    if let Some(ref dlq) = failed_obs_repo {
                        for &idx in &result.fallback_indices {
                            if idx >= dlq_count {
                                // This is a new observation (not a DLQ reprocess)
                                if let Some(obs) = to_extract.get(idx) {
                                    dlq.insert(obs, "extraction", "llm_fallback").await;
                                    if let Some(tx) = &pipeline_tx {
                                        let _ = tx.send(PipelineEvent::DeadLetterQueued {
                                            observation: obs.content.clone(),
                                            failure_reason: "llm_fallback".into(),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Episodic memory for high-importance observations
                    for obs in &to_extract {
                        if obs.importance >= 0.7 {
                            if let Some(ref ep_repo) = episodic_repo {
                                let ts = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
                                let mem = EpisodicMemory {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    domain: obs.domain.clone(),
                                    content: obs.content.clone(),
                                    summary: None,
                                    importance: obs.importance,
                                    occurred_at: ts.clone(),
                                    recorded_at: ts,
                                    stability: 1.0,
                                    last_accessed: None,
                                    access_count: 0,
                                    project_id: None,
                                    scope_type: "system".to_string(),
                                    scope_id: None,
                                };
                                if let Err(e) = ep_repo.insert(&mem).await {
                                    warn!("Failed to store episodic memory: {e}");
                                }
                            }
                        }
                    }

                    // Prefetch existing facts + batch consolidation
                    let candidates =
                        prefetch_existing(&result.extractions, &to_extract, &repo).await;

                    if !candidates.is_empty() {
                        let ops = match consolidation.decide_batch(&candidates).await {
                            Ok(o) => o,
                            Err(e) => {
                                warn!("Batch consolidation failed: {e}");
                                vec![crate::types::MemoryOp::Noop; candidates.len()]
                            }
                        };

                        // Emit consolidation pipeline events
                        if let Some(tx) = &pipeline_tx {
                            for (c, op) in candidates.iter().zip(ops.iter()) {
                                let _ = tx.send(PipelineEvent::Consolidation {
                                    operation: op_to_string(op),
                                    fact: format!(
                                        "{}.{} = {}",
                                        c.candidate.subject,
                                        c.candidate.predicate,
                                        c.candidate.object
                                    ),
                                });
                            }
                        }

                        crate::consolidation::execute_memory_ops(
                            &ops,
                            &candidates,
                            &repo,
                            embedder.as_deref(),
                        )
                        .await;

                        // ── Contradiction detection ──────────────────────────
                        if let Some(ref bus) = domain_bus {
                            for (candidate, op) in candidates.iter().zip(ops.iter()) {
                                if let crate::types::MemoryOp::Update { id: _, old_id } = op {
                                    let new = &candidate.candidate;
                                    if let Ok(Some(old_fact)) = repo.get(old_id).await {
                                        if old_fact.confidence < 0.7
                                            || old_fact.source != "user_stated"
                                        {
                                            continue;
                                        }
                                        if old_fact.object != new.object
                                            && !is_same_session(
                                                &old_fact.recorded_at,
                                                &session_start,
                                            )
                                        {
                                            bus.publish(DomainEvent::ContradictionDetected {
                                                existing_subject: old_fact.subject.clone(),
                                                existing_predicate: old_fact.predicate.clone(),
                                                existing_object: old_fact.object.clone(),
                                                new_object: new.object.clone(),
                                                confidence: new.confidence,
                                            });
                                        }
                                    }
                                }
                            }
                        }

                        // ── Entity extraction from new facts ──────────────────
                        let entity_repo = crate::repos::EntityRepo::new(repo.pool().clone());
                        for (candidate, op) in candidates.iter().zip(ops.iter()) {
                            match op {
                                crate::types::MemoryOp::Add { .. }
                                | crate::types::MemoryOp::Update { .. } => {
                                    let fact = &candidate.candidate;

                                    // Infer entity type from predicate (once, reused for both subject and object)
                                    let entity_type = infer_entity_type(&fact.predicate);

                                    // Upsert subject as entity
                                    let subj = entity_repo
                                        .upsert_entity(&crate::repos::NewEntity {
                                            name: fact.subject.clone(),
                                            entity_type: entity_type.clone(),
                                            description: None,
                                            source: "extracted".to_string(),
                                            source_id: Some(fact.id.clone()),
                                            metadata: None,
                                        })
                                        .await;

                                    // Upsert object as entity (skip numeric/short values)
                                    let obj = if fact.object.len() > 2
                                        && fact.object.len() < 100
                                        && !fact
                                            .object
                                            .chars()
                                            .all(|c| c.is_ascii_digit() || c == '.')
                                    {
                                        entity_repo
                                            .upsert_entity(&crate::repos::NewEntity {
                                                name: fact.object.clone(),
                                                entity_type,
                                                description: None,
                                                source: "extracted".to_string(),
                                                source_id: Some(fact.id.clone()),
                                                metadata: None,
                                            })
                                            .await
                                            .ok()
                                    } else {
                                        None
                                    };

                                    // Create relationship between subject and object entities
                                    if let (Ok(s), Some(o)) = (subj, obj) {
                                        let _ = entity_repo
                                            .upsert_relationship(&crate::repos::NewRelationship {
                                                source_entity_id: s.id,
                                                target_entity_id: o.id,
                                                relationship_type: fact.predicate.clone(),
                                                evidence: Some(format!(
                                                    "{} {} {}",
                                                    fact.subject, fact.predicate, fact.object
                                                )),
                                                source: "extracted".to_string(),
                                            })
                                            .await;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    // Self-healing: drain dead-letter if LLM is healthy (no fallbacks this batch)
                    if fallback_set.is_empty() {
                        if let Some(ref dlq) = failed_obs_repo {
                            let eligible = dlq.list_eligible(5).await;
                            for row in eligible {
                                match serde_json::from_str::<Observation>(&row.observation_json) {
                                    Ok(obs) => {
                                        dlq_reprocess_queue.push(obs);
                                        dlq_reprocess_ids.push(row.id.clone());
                                    }
                                    Err(e) => {
                                        warn!("Failed to deserialize dead-letter observation: {e}");
                                        dlq.mark_failed(&row.id).await;
                                    }
                                }
                            }
                        }
                    }
                }

                // Handle accumulation
                for (key, obs) in to_accumulate {
                    if let Some(ref ar) = accum_repo {
                        ar.insert(&key, &obs).await;
                    }

                    let entry = accumulator
                        .entry(key.clone())
                        .or_insert_with(AccumulatedEntry::new);
                    entry.add(obs);

                    if entry.should_promote(promote_threshold, min_days) {
                        debug!(
                            "Promoting accumulated events for '{key}' ({} events, {} days)",
                            entry.observations.len(),
                            entry.days_seen.len()
                        );
                        let summary = summarize_accumulated(&key, &entry.observations);
                        promotion_queue.push(summary);
                        accumulator.remove(&key);
                        if let Some(ref ar) = accum_repo {
                            ar.delete_by_key(&key).await;
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel,
            task_handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                warn!("BackgroundConsolidation task panicked: {e}");
            }
        }
    }
}

/// Convert a `DomainEvent` to an `Observation` for processing.
fn event_to_observation(event: &DomainEvent) -> Option<Observation> {
    let now = Utc::now();
    match event {
        DomainEvent::ChatTurnCompleted { user_message, .. } => Some(Observation {
            domain: "general".into(),
            content: user_message.clone(),
            importance: 0.8,
            source_event: "ChatTurnCompleted".into(),
            timestamp: now,
        }),
        DomainEvent::UserStatedFact { fact, domain } => Some(Observation {
            domain: domain.clone(),
            content: fact.clone(),
            importance: 1.0,
            source_event: "UserStatedFact".into(),
            timestamp: now,
        }),
        DomainEvent::UserCorrectedAI {
            original,
            correction,
            ..
        } => Some(Observation {
            domain: "meta".into(),
            content: format!("User corrected: '{original}' → '{correction}'"),
            importance: 1.0,
            source_event: "UserCorrectedAI".into(),
            timestamp: now,
        }),
        DomainEvent::AutotunerDecision {
            verdict,
            improvement_pct,
            affected_params,
            ..
        } => {
            let params_text = if affected_params.is_empty() {
                "general parameters".into()
            } else {
                affected_params.join(", ")
            };
            let content = if verdict == "reverted" {
                format!(
                    "I noticed a recent change to {params_text} wasn't working well and reverted to my previous approach."
                )
            } else {
                format!(
                    "I refined how I handle your requests — adjusted {params_text}, \
                     improving response alignment by {improvement_pct:.1}%."
                )
            };
            Some(Observation {
                domain: "meta".into(),
                content,
                importance: if verdict == "reverted" { 0.9 } else { 0.8 },
                source_event: "AutotunerDecision".into(),
                timestamp: now,
            })
        }
        DomainEvent::BudgetAlert {
            category,
            spent,
            limit,
        } => Some(Observation {
            domain: "finance".into(),
            content: format!("Budget alert: {category} at ${spent:.2} of ${limit:.2} limit"),
            importance: 0.9,
            source_event: "BudgetAlert".into(),
            timestamp: now,
        }),
        DomainEvent::ProductivityScoreComputed { date, score } => Some(Observation {
            domain: "productivity".into(),
            content: format!("Productivity score for {date}: {score:.1}"),
            importance: 0.5,
            source_event: "ProductivityScoreComputed".into(),
            timestamp: now,
        }),
        DomainEvent::TaskCompleted {
            task_id,
            actual_duration_mins,
            estimated_duration_mins,
            deviation_pct,
        } => {
            let detail = match (actual_duration_mins, estimated_duration_mins) {
                (Some(actual), Some(est)) => {
                    format!(" (actual: {actual}min, estimated: {est}min)")
                }
                _ => String::new(),
            };
            let dev_detail = deviation_pct
                .map(|d| format!(", deviation: {d:.1}%"))
                .unwrap_or_default();
            Some(Observation {
                domain: "tasks".into(),
                content: format!("Task {task_id} completed{detail}{dev_detail}"),
                importance: 0.5,
                source_event: "TaskCompleted".into(),
                timestamp: now,
            })
        }
        DomainEvent::DistractionDetected { app, context, .. } => Some(Observation {
            domain: "productivity".into(),
            content: format!("Distraction: {app} ({context})"),
            importance: 0.6,
            source_event: "DistractionDetected".into(),
            timestamp: now,
        }),
        DomainEvent::FocusSessionEnded {
            duration_secs,
            quality,
            interruptions,
        } => Some(Observation {
            domain: "productivity".into(),
            content: format!(
                "Focus session: {}min, quality {quality:.0}%, {interruptions} interruptions",
                duration_secs / 60
            ),
            importance: 0.5,
            source_event: "FocusSessionEnded".into(),
            timestamp: now,
        }),
        DomainEvent::TransactionRecorded {
            category,
            amount,
            is_over_budget,
        } => Some(Observation {
            domain: "finance".into(),
            content: format!(
                "Transaction: ${amount:.2} in {category}{}",
                if *is_over_budget {
                    " (OVER BUDGET)"
                } else {
                    ""
                }
            ),
            importance: if *is_over_budget { 0.8 } else { 0.4 },
            source_event: "TransactionRecorded".into(),
            timestamp: now,
        }),
        DomainEvent::CoachingFeedback {
            intervention_id,
            response,
        } => Some(Observation {
            domain: "coaching".into(),
            content: format!("Coaching feedback for {intervention_id}: {response:?}"),
            importance: 0.9,
            source_event: "CoachingFeedback".into(),
            timestamp: now,
        }),
        DomainEvent::SessionEnded {
            session_type,
            duration_secs,
            quality_score,
            ..
        } => {
            let quality = quality_score.map_or("N/A".to_string(), |q| format!("{q:.0}"));
            Some(Observation {
                domain: "productivity".into(),
                content: format!(
                    "{session_type} session ended: {}min, quality {quality}",
                    duration_secs / 60
                ),
                importance: if quality_score.is_some_and(|q| q >= 80.0) {
                    0.7
                } else {
                    0.5
                },
                source_event: "SessionEnded".into(),
                timestamp: now,
            })
        }
        DomainEvent::QualityScored {
            score_date,
            overall_score,
            ..
        } => Some(Observation {
            domain: "productivity".into(),
            content: format!("Quality score for {score_date}: {overall_score:.1}"),
            importance: if *overall_score >= 85.0 || *overall_score <= 30.0 {
                0.8
            } else {
                0.4
            },
            source_event: "QualityScored".into(),
            timestamp: now,
        }),
        DomainEvent::NarrativeGenerated {
            date,
            sentiment,
            excerpt,
        } => Some(Observation {
            domain: "productivity".into(),
            content: format!("Daily narrative ({date}, {sentiment}): {excerpt}"),
            importance: 0.7,
            source_event: "NarrativeGenerated".into(),
            timestamp: now,
        }),
        DomainEvent::VoiceJournalProcessed {
            extracted_fact_count,
            sentiment,
            ..
        } => {
            let sent = sentiment.as_deref().unwrap_or("unknown");
            Some(Observation {
                domain: "productivity".into(),
                content: format!(
                    "Voice journal processed: {extracted_fact_count} facts extracted, sentiment {sent}"
                ),
                importance: 0.6,
                source_event: "VoiceJournalProcessed".into(),
                timestamp: now,
            })
        }
        // Intelligence layer events that are discarded by salience — avoid Debug formatting
        DomainEvent::SessionCreated { .. } | DomainEvent::RuleEvolved { .. } => None,
        DomainEvent::PredictiveAlert {
            forecast_type,
            predicted_value,
            suggested_action,
            ..
        } => Some(Observation {
            domain: "productivity".into(),
            content: format!(
                "Predictive alert ({forecast_type}): value={predicted_value:.2}{}",
                suggested_action
                    .as_deref()
                    .map(|a| format!(", action: {a}"))
                    .unwrap_or_default()
            ),
            importance: 0.5,
            source_event: "PredictiveAlert".into(),
            timestamp: now,
        }),
        // Agentic task events — Extract-priority need good observations
        DomainEvent::TaskExecutionCompleted {
            task_id,
            execution_id,
            tokens_used,
            cost_usd,
            artifacts_count,
        } => {
            let cost_str = cost_usd
                .map(|c| format!(", cost ${c:.4}"))
                .unwrap_or_default();
            Some(Observation {
                domain: "tasks".into(),
                content: format!(
                    "Task {task_id} execution {execution_id} completed: {tokens_used} tokens{cost_str}, {artifacts_count} artifacts"
                ),
                importance: 0.8,
                source_event: "TaskExecutionCompleted".into(),
                timestamp: now,
            })
        }
        DomainEvent::TaskExecutionFailed {
            task_id,
            execution_id,
            error,
            retry_count,
        } => Some(Observation {
            domain: "tasks".into(),
            content: format!(
                "Task {task_id} execution {execution_id} failed (retry {retry_count}): {error}"
            ),
            importance: 0.9,
            source_event: "TaskExecutionFailed".into(),
            timestamp: now,
        }),
        DomainEvent::TaskFocusStarted {
            task_id,
            energy_level,
        } => Some(Observation {
            domain: "tasks".into(),
            content: format!("Focus started on task {task_id} at energy level {energy_level}"),
            importance: 0.3,
            source_event: "TaskFocusStarted".into(),
            timestamp: now,
        }),
        DomainEvent::TaskFocusEnded {
            task_id,
            duration_secs,
        } => Some(Observation {
            domain: "tasks".into(),
            content: format!(
                "Focus ended on task {task_id} after {}min",
                duration_secs / 60
            ),
            importance: 0.3,
            source_event: "TaskFocusEnded".into(),
            timestamp: now,
        }),
        DomainEvent::EstimationRecorded {
            task_id,
            estimated_mins,
            actual_mins,
            deviation_pct,
        } => {
            let importance = if deviation_pct.abs() > HIGH_DEVIATION_THRESHOLD {
                0.6
            } else {
                0.3
            };
            Some(Observation {
                domain: "tasks".into(),
                content: format!(
                    "Estimation recorded for task {task_id}: estimated {estimated_mins}min, actual {actual_mins}min, deviation {deviation_pct:.1}%"
                ),
                importance,
                source_event: "EstimationRecorded".into(),
                timestamp: now,
            })
        }
        DomainEvent::TaskDecomposed {
            source_task_id,
            subtask_ids,
            total_estimated_mins,
        } => {
            let est = total_estimated_mins
                .map(|m| format!(", total {m}min"))
                .unwrap_or_default();
            Some(Observation {
                domain: "tasks".into(),
                content: format!(
                    "Task {source_task_id} decomposed into {} subtasks{est}",
                    subtask_ids.len()
                ),
                importance: 0.4,
                source_event: "TaskDecomposed".into(),
                timestamp: now,
            })
        }
        DomainEvent::DayPlanGenerated {
            task_count,
            total_estimated_mins,
        } => Some(Observation {
            domain: "tasks".into(),
            content: format!(
                "Day plan generated: {task_count} tasks, {total_estimated_mins}min scheduled"
            ),
            importance: 0.4,
            source_event: "DayPlanGenerated".into(),
            timestamp: now,
        }),
        DomainEvent::ProactiveSuggestionCreated {
            suggestion_id: _,
            suggestion_type,
            task_id,
            confidence,
        } => {
            let target = task_id
                .as_deref()
                .map(|id| format!(" for task {id}"))
                .unwrap_or_default();
            Some(Observation {
                domain: "tasks".into(),
                content: format!(
                    "Proactive suggestion: {suggestion_type}{target} (confidence {:.0}%)",
                    confidence * 100.0
                ),
                importance: 0.3,
                source_event: "ProactiveSuggestionCreated".into(),
                timestamp: now,
            })
        }
        DomainEvent::TaskExecutionStarted {
            task_id,
            execution_id,
            agent_profile: _,
        } => Some(Observation {
            domain: "tasks".into(),
            content: format!("Agentic execution {execution_id} started for task {task_id}"),
            importance: 0.4,
            source_event: "TaskExecutionStarted".into(),
            timestamp: now,
        }),
        _ => {
            // TaskCreated, TaskDeferred, GoalProgress, ActivitySessionCompleted,
            // and lower-priority agentic events (Accumulate-level)
            let content = format!("{event:?}");
            Some(Observation {
                domain: "general".into(),
                content,
                importance: 0.3,
                source_event: "Other".into(),
                timestamp: now,
            })
        }
    }
}

/// Create a type key for an event (used as accumulator key).
fn event_type_key(event: &DomainEvent) -> String {
    match event {
        DomainEvent::ActivitySessionCompleted { .. } => "ActivitySessionCompleted".into(),
        DomainEvent::FocusSessionStarted { .. } => "FocusSessionStarted".into(),
        DomainEvent::FocusSessionEnded { .. } => "FocusSessionEnded".into(),
        DomainEvent::DistractionDetected { .. } => "DistractionDetected".into(),
        DomainEvent::ProductivityScoreComputed { .. } => "ProductivityScoreComputed".into(),
        DomainEvent::TaskCreated { .. } => "TaskCreated".into(),
        DomainEvent::TaskCompleted { .. } => "TaskCompleted".into(),
        DomainEvent::TaskDeferred { .. } => "TaskDeferred".into(),
        DomainEvent::GoalProgress { .. } => "GoalProgress".into(),
        DomainEvent::TransactionRecorded { .. } => "TransactionRecorded".into(),
        DomainEvent::BudgetAlert { .. } => "BudgetAlert".into(),
        DomainEvent::ChatTurnCompleted { .. } => "ChatTurnCompleted".into(),
        DomainEvent::UserStatedFact { .. } => "UserStatedFact".into(),
        DomainEvent::UserCorrectedAI { .. } => "UserCorrectedAI".into(),
        DomainEvent::CoachingFeedback { .. } => "CoachingFeedback".into(),
        DomainEvent::NoteCreated { .. } => "NoteCreated".into(),
        DomainEvent::NoteUpdated { .. } => "NoteUpdated".into(),
        DomainEvent::SessionCreated { .. } => "SessionCreated".into(),
        DomainEvent::SessionEnded { .. } => "SessionEnded".into(),
        DomainEvent::QualityScored { .. } => "QualityScored".into(),
        DomainEvent::PredictiveAlert { .. } => "PredictiveAlert".into(),
        DomainEvent::NarrativeGenerated { .. } => "NarrativeGenerated".into(),
        DomainEvent::RuleEvolved { .. } => "RuleEvolved".into(),
        DomainEvent::VoiceJournalProcessed { .. } => "VoiceJournalProcessed".into(),
        DomainEvent::ToolCallExecuted { .. } => "ToolCallExecuted".into(),
        DomainEvent::BehavioralPatternDetected { .. } => "BehavioralPatternDetected".into(),
        DomainEvent::TaskDecomposed { .. } => "TaskDecomposed".into(),
        DomainEvent::TaskExecutionStarted { .. } => "TaskExecutionStarted".into(),
        DomainEvent::TaskExecutionCompleted { .. } => "TaskExecutionCompleted".into(),
        DomainEvent::TaskExecutionFailed { .. } => "TaskExecutionFailed".into(),
        DomainEvent::TaskBlocked { .. } => "TaskBlocked".into(),
        DomainEvent::TaskUnblocked { .. } => "TaskUnblocked".into(),
        DomainEvent::DayPlanGenerated { .. } => "DayPlanGenerated".into(),
        DomainEvent::ProactiveSuggestionCreated { .. } => "ProactiveSuggestionCreated".into(),
        DomainEvent::TaskFocusStarted { .. } => "TaskFocusStarted".into(),
        DomainEvent::TaskFocusEnded { .. } => "TaskFocusEnded".into(),
        DomainEvent::EstimationRecorded { .. } => "EstimationRecorded".into(),
        DomainEvent::TaskExecutionProgress { .. } => "TaskExecutionProgress".into(),
        DomainEvent::TaskStatusChanged { .. } => "TaskStatusChanged".into(),
        DomainEvent::TaskPriorityChanged { .. } => "TaskPriorityChanged".into(),
        DomainEvent::TaskFieldUpdated { .. } => "TaskFieldUpdated".into(),
        DomainEvent::AutotunerDecision { .. } => "AutotunerDecision".into(),
        DomainEvent::ContradictionDetected { .. } => "ContradictionDetected".into(),
        DomainEvent::NoteContentChanged { .. } => "NoteContentChanged".into(),
        DomainEvent::NoteDeleted { .. } => "NoteDeleted".into(),
        DomainEvent::TaskHierarchyChanged { .. } => "TaskHierarchyChanged".into(),
    }
}

/// Check if a fact was recorded in the current session.
fn is_same_session(recorded_at: &str, session_start: &str) -> bool {
    let recorded = chrono::DateTime::parse_from_rfc3339(recorded_at)
        .or_else(|_| {
            recorded_at
                .parse::<chrono::NaiveDateTime>()
                .map(|ndt| ndt.and_utc().fixed_offset())
        })
        .unwrap_or_default();
    let start = chrono::DateTime::parse_from_rfc3339(session_start).unwrap_or_default();
    (recorded - start).num_seconds().abs() < 300
}

/// Infer entity type from predicate keywords.
/// Applied to both subject and object sides — the predicate determines type regardless of position.
fn infer_entity_type(predicate: &str) -> String {
    let p = predicate.to_lowercase();
    if p.contains("person")
        || p.contains("manages")
        || p.contains("hired")
        || p.contains("reports_to")
    {
        return "person".to_string();
    }
    if p.contains("project") || p.contains("works_on") || p.contains("contributes_to") {
        return "project".to_string();
    }
    if p.contains("uses")
        || p.contains("tool")
        || p.contains("technology")
        || p.contains("framework")
        || p.contains("is_a_technology")
    {
        return "technology".to_string();
    }
    if p.contains("organization") || p.contains("company") || p.contains("employer") {
        return "organization".to_string();
    }
    "concept".to_string()
}

/// Convert a MemoryOp to a string label for the debug dashboard.
fn op_to_string(op: &crate::types::MemoryOp) -> String {
    match op {
        crate::types::MemoryOp::Add { .. } => "ADD".into(),
        crate::types::MemoryOp::Update { .. } => "UPDATE".into(),
        crate::types::MemoryOp::Delete { .. } => "DELETE".into(),
        crate::types::MemoryOp::Noop => "NOOP".into(),
    }
}

/// Summarize accumulated observations into a single observation for extraction.
fn summarize_accumulated(event_type: &str, observations: &[Observation]) -> Observation {
    let contents: Vec<&str> = observations.iter().map(|o| o.content.as_str()).collect();
    let domain = observations
        .first()
        .map(|o| o.domain.clone())
        .unwrap_or_else(|| "general".into());
    let avg_importance =
        observations.iter().map(|o| o.importance).sum::<f64>() / observations.len().max(1) as f64;

    Observation {
        domain,
        content: format!(
            "Pattern from {} accumulated {} events:\n{}",
            observations.len(),
            event_type,
            contents.join("\n")
        ),
        importance: avg_importance,
        source_event: format!("accumulated:{event_type}"),
        timestamp: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    #[test]
    fn test_event_to_observation_user_stated_fact() {
        let event = DomainEvent::UserStatedFact {
            fact: "I work best in mornings".into(),
            domain: "productivity".into(),
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "productivity");
        assert_eq!(obs.importance, 1.0);
        assert_eq!(obs.content, "I work best in mornings");
    }

    #[test]
    fn test_event_to_observation_chat_turn_completed() {
        let event = DomainEvent::ChatTurnCompleted {
            user_message: "I prefer dark mode in all editors".into(),
            session_key: "session-1".into(),
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "general");
        assert_eq!(obs.importance, 0.8);
        assert_eq!(obs.content, "I prefer dark mode in all editors");
        assert_eq!(obs.source_event, "ChatTurnCompleted");
    }

    #[test]
    fn test_event_to_observation_budget_alert() {
        let event = DomainEvent::BudgetAlert {
            category: "food".into(),
            spent: 450.0,
            limit: 500.0,
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "finance");
        assert!(obs.content.contains("450.00"));
    }

    #[test]
    fn test_accumulated_entry_promotion() {
        let mut entry = AccumulatedEntry::new();

        // Add 5 observations across 3 days
        for (i, day) in [
            "2026-03-01",
            "2026-03-02",
            "2026-03-03",
            "2026-03-03",
            "2026-03-01",
        ]
        .iter()
        .enumerate()
        {
            let ts = format!("{day}T10:00:00Z");
            entry.add(Observation {
                domain: "productivity".into(),
                content: format!("event {i}"),
                importance: 0.5,
                source_event: "test".into(),
                timestamp: ts.parse::<DateTime<Utc>>().unwrap(),
            });
        }

        assert!(entry.should_promote(5, 3));
        assert_eq!(entry.observations.len(), 5);
        assert_eq!(entry.days_seen.len(), 3);
    }

    #[test]
    fn test_accumulated_entry_not_enough_days() {
        let mut entry = AccumulatedEntry::new();

        // 5 events but only 1 day
        for i in 0..5 {
            entry.add(Observation {
                domain: "productivity".into(),
                content: format!("event {i}"),
                importance: 0.5,
                source_event: "test".into(),
                timestamp: "2026-03-01T10:00:00Z".parse().unwrap(),
            });
        }

        assert!(!entry.should_promote(5, 3)); // Not enough days
    }

    #[test]
    fn test_summarize_accumulated() {
        let observations = vec![
            Observation {
                domain: "productivity".into(),
                content: "Score: 72".into(),
                importance: 0.5,
                source_event: "ProductivityScoreComputed".into(),
                timestamp: Utc::now(),
            },
            Observation {
                domain: "productivity".into(),
                content: "Score: 78".into(),
                importance: 0.6,
                source_event: "ProductivityScoreComputed".into(),
                timestamp: Utc::now(),
            },
        ];

        let summary = summarize_accumulated("ProductivityScoreComputed", &observations);
        assert_eq!(summary.domain, "productivity");
        assert!(summary.content.contains("Score: 72"));
        assert!(summary.content.contains("Score: 78"));
        assert!(summary.content.contains("2 accumulated"));
    }

    #[test]
    fn test_event_to_observation_task_focus_started() {
        let event = DomainEvent::TaskFocusStarted {
            task_id: "t1".into(),
            energy_level: "high".into(),
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "tasks");
        assert!(obs.content.contains("t1"));
        assert!(obs.content.contains("high"));
        assert_eq!(obs.source_event, "TaskFocusStarted");
    }

    #[test]
    fn test_event_to_observation_task_focus_ended() {
        let event = DomainEvent::TaskFocusEnded {
            task_id: "t1".into(),
            duration_secs: 2700,
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "tasks");
        assert!(obs.content.contains("45min")); // 2700 / 60
        assert_eq!(obs.source_event, "TaskFocusEnded");
    }

    #[test]
    fn test_event_to_observation_estimation_recorded() {
        let event = DomainEvent::EstimationRecorded {
            task_id: "t1".into(),
            estimated_mins: 30,
            actual_mins: 75,
            deviation_pct: 150.0,
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "tasks");
        assert!(obs.content.contains("estimated 30min"));
        assert!(obs.content.contains("actual 75min"));
        assert!(obs.content.contains("150.0%"));
        assert_eq!(obs.source_event, "EstimationRecorded");
    }

    #[test]
    fn test_event_to_observation_estimation_recorded_importance() {
        let large_dev = DomainEvent::EstimationRecorded {
            task_id: "t1".into(),
            estimated_mins: 30,
            actual_mins: 75,
            deviation_pct: 150.0,
        };
        let obs = event_to_observation(&large_dev).unwrap();
        assert!(
            obs.importance >= 0.6,
            "Large deviation should have high importance, got {}",
            obs.importance
        );

        let small_dev = DomainEvent::EstimationRecorded {
            task_id: "t2".into(),
            estimated_mins: 30,
            actual_mins: 35,
            deviation_pct: 16.7,
        };
        let obs2 = event_to_observation(&small_dev).unwrap();
        assert!(
            obs2.importance <= 0.4,
            "Small deviation should have low importance, got {}",
            obs2.importance
        );
    }

    #[test]
    fn test_event_to_observation_task_decomposed() {
        let event = DomainEvent::TaskDecomposed {
            source_task_id: "t1".into(),
            subtask_ids: vec!["s1".into(), "s2".into(), "s3".into()],
            total_estimated_mins: Some(120),
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "tasks");
        assert!(obs.content.contains("3 subtasks"));
        assert!(obs.content.contains("120min"));
        assert_eq!(obs.source_event, "TaskDecomposed");
    }

    #[test]
    fn test_event_to_observation_day_plan_generated() {
        let event = DomainEvent::DayPlanGenerated {
            task_count: 5,
            total_estimated_mins: 360,
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "tasks");
        assert!(obs.content.contains("5 tasks"));
        assert!(obs.content.contains("360min"));
        assert_eq!(obs.source_event, "DayPlanGenerated");
    }

    #[test]
    fn test_event_to_observation_proactive_suggestion() {
        let event = DomainEvent::ProactiveSuggestionCreated {
            suggestion_id: "sug-1".into(),
            suggestion_type: "Decompose".into(),
            task_id: Some("t1".into()),
            confidence: 0.85,
        };
        let obs = event_to_observation(&event).unwrap();
        assert_eq!(obs.domain, "tasks");
        assert!(obs.content.contains("Decompose"));
        assert!(obs.content.contains("85%"));
        assert_eq!(obs.source_event, "ProactiveSuggestionCreated");
    }

    #[test]
    fn test_event_type_key() {
        assert_eq!(
            event_type_key(&DomainEvent::ProductivityScoreComputed {
                date: "2026-03-06".into(),
                score: 72.0,
            }),
            "ProductivityScoreComputed"
        );
        assert_eq!(
            event_type_key(&DomainEvent::TaskCompleted {
                task_id: "t1".into(),
                actual_duration_mins: None,
                estimated_duration_mins: None,
                deviation_pct: None,
            }),
            "TaskCompleted"
        );
    }
}
