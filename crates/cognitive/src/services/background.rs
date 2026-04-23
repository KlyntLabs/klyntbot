//! Background consolidation service for accumulated observations.
//!
//! Accumulated events are buffered and promoted to extraction when patterns
//! emerge. The old salience-based event classification has been removed;
//! SignalRouter now handles event-to-signal translation upstream.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use jiff::Timestamp;
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
use crate::types::{EpisodicMemory, Observation};

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

/// Maximum observations kept per accumulator key. Once reached, the oldest
/// observations are dropped (FIFO). This prevents a single high-frequency
/// event type (e.g. `NoteContentChanged`) from consuming unbounded memory
/// between promotion cycles.
const MAX_OBSERVATIONS_PER_KEY: usize = 50;

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
        let day = obs.timestamp.strftime("%Y-%m-%d").to_string();
        self.days_seen.insert(day);
        self.observations.push(obs);
        // Cap per-key observations to prevent unbounded growth
        if self.observations.len() > MAX_OBSERVATIONS_PER_KEY {
            self.observations
                .drain(..self.observations.len() - MAX_OBSERVATIONS_PER_KEY);
        }
    }

    /// Returns the effective promotion threshold for a given `RecallDomain`.
    pub fn effective_threshold(
        domain: &ai_core::RecallDomain,
        overrides: &std::collections::HashMap<ai_core::RecallDomain, usize>,
        global: usize,
    ) -> usize {
        overrides.get(domain).copied().unwrap_or(global)
    }

    pub fn should_promote_for_domain(
        &self,
        domain: &ai_core::RecallDomain,
        overrides: &std::collections::HashMap<ai_core::RecallDomain, usize>,
        global_threshold: usize,
        min_days: usize,
    ) -> bool {
        let t = Self::effective_threshold(domain, overrides, global_threshold);
        self.observations.len() >= t && self.days_seen.len() >= min_days
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
    pub promote_overrides: std::collections::HashMap<ai_core::RecallDomain, usize>,
    pub min_days: usize,
    pub domain_bus: Option<Arc<bus::DomainEventBus>>,
    pub context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
    pub session_repo: Option<storage::SessionRepo>,
    pub rule_repo: Option<crate::repos::ProceduralRuleRepo>,
    /// Unified pipeline signal sender (cloned into collectors).
    pub signal_tx: Option<crate::pipeline::SignalSender>,
    /// Unified pipeline signal receiver (moved into event loop).
    pub signal_rx: Option<crate::pipeline::SignalReceiver>,
    /// Session memory repo for SessionCollector (optional).
    pub session_memory_repo: Option<storage::SessionMemoryRepo>,
    /// Intelligence mode: Standard (heuristic) or Deep (LLM-based) consolidation.
    pub intelligence_mode: config::schema::IntelligenceMode,
    /// Optional LLM handler for Deep Mode consolidation.
    pub deep_handler: Option<Arc<dyn crate::pipeline::DeepConsolidationHandler>>,
    /// Optional density repo for value-density scoring.
    pub density_repo: Option<crate::repos::ConversationDensityRepo>,
    /// Optional pending memory repo — low-confidence facts are routed here
    /// instead of being inserted directly into semantic_facts.
    pub pending_repo: Option<crate::repos::PendingMemoryRepo>,
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
            promote_overrides,
            min_days,
            domain_bus,
            context_update_queue,
            session_repo: _session_repo,
            rule_repo,
            signal_tx: _signal_tx,
            mut signal_rx,
            session_memory_repo: _session_memory_repo,
            intelligence_mode,
            deep_handler,
            density_repo,
            pending_repo,
        } = config;
        // Cognitive collectors are SignalConsumers wired by the app-core init
        // layer; the legacy broadcast-collector startup that lived here is
        // gone. `signal_rx` / `domain_bus` are still consumed by the downstream
        // accumulator and rule-evolution paths below.
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

            let session_start = jiff::Timestamp::now().to_string();
            let mut batch_count: u64 = 0;

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

                // ── Domain entity bridge ──────────────────────────────────
                // Upsert structured domain objects into the entities table
                // immediately.
                {
                    let entity_repo = crate::repos::EntityRepo::new(repo.pool().clone());
                    for event in &batch {
                        match event {
                            DomainEvent::TaskCreated { task_id, .. } => {
                                upsert_domain_entity(&entity_repo, task_id, "task", task_id).await;
                            }
                            DomainEvent::TransactionRecorded { category, .. } => {
                                upsert_domain_entity(
                                    &entity_repo,
                                    category,
                                    "finance_category",
                                    category,
                                )
                                .await;
                            }
                            _ => {}
                        }
                    }
                }

                // SignalRouter now produces AiSignals upstream; this service only
                // handles accumulated-observation promotion and periodic compaction.
                // The old classify_batch + event_to_observation path has been removed.
                let mut to_extract: Vec<Observation> = Vec::new();
                let to_accumulate: Vec<(String, Observation)> = Vec::new();

                // Prepend DLQ retries and promotions (kept for backward compat)
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
                                let ts = Timestamp::now().strftime("%Y-%m-%dT%H:%M:%S").to_string();
                                let mem = EpisodicMemory {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    domain: obs.domain.clone(),
                                    content: obs.content.clone(),
                                    summary: Some(summarize_observation(&obs.content)),
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

                    // Reinforce procedural rules when extracted facts match rule patterns.
                    // Fan out per-observation searches concurrently (mirroring prefetch_existing).
                    if let Some(ref rr) = rule_repo {
                        let match_futs: Vec<_> = to_extract
                            .iter()
                            .map(|obs| async {
                                for domain in crate::repos::RULE_DOMAINS {
                                    if let Ok(Some(matching)) =
                                        rr.find_similar(&obs.content, domain).await
                                    {
                                        return Some(matching.id);
                                    }
                                }
                                None
                            })
                            .collect();
                        let matches = futures_util::future::join_all(match_futs).await;
                        for rule_id in matches.into_iter().flatten() {
                            let _ = rr.increment_signal_count(&rule_id).await;
                        }
                    }

                    // Persist LLM-extracted entities and relationships (weak — low initial strength)
                    if !result.entities.is_empty() || !result.relationships.is_empty() {
                        let entity_repo = crate::repos::EntityRepo::new(repo.pool().clone());
                        crate::pipeline::writer::persist_entities(
                            &entity_repo,
                            &result.entities,
                            &result.relationships,
                        )
                        .await;
                    }

                    // Compute value-density for each observation in the batch
                    if let Some(ref density_repo) = density_repo {
                        for obs in &to_extract {
                            let score = crate::services::value_density::score_turn(
                                &obs.content,
                                None, // Known entities not available here yet
                            );
                            let id = format!("vd-{:x}", {
                                use std::hash::{Hash, Hasher};
                                let mut h = std::collections::hash_map::DefaultHasher::new();
                                obs.content.hash(&mut h);
                                obs.timestamp.hash(&mut h);
                                h.finish()
                            });
                            let preview = common::helpers::truncate_at_boundary(&obs.content, 100);
                            if let Err(e) = density_repo
                                .insert(&id, &session_start, preview, &score)
                                .await
                            {
                                debug!("Failed to store density score: {e}");
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
                            pending_repo.as_ref(),
                        )
                        .await;

                        // ── Live context update: push promoted facts ─────────
                        if let Some(ref queue) = context_update_queue {
                            for (candidate, op) in candidates.iter().zip(ops.iter()) {
                                match op {
                                    crate::types::MemoryOp::Add { .. }
                                    | crate::types::MemoryOp::Update { .. } => {
                                        let fact = &candidate.candidate;
                                        queue.push(bus::ContextUpdate {
                                            reason: bus::ContextUpdateReason::MemoryPromoted,
                                            content: Some(format!(
                                                "{} — {}",
                                                fact.subject, fact.predicate
                                            )),
                                            metadata: None,
                                            priority: bus::UpdatePriority::Normal,
                                            timestamp: jiff::Timestamp::now(),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }

                        // ── Contradiction detection ──────────────────────────
                        if let Some(ref bus) = domain_bus {
                            for (candidate, op) in candidates.iter().zip(ops.iter()) {
                                match op {
                                    crate::types::MemoryOp::Update { id: _, old_id } => {
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
                                    crate::types::MemoryOp::Add { .. } => {
                                        let new = &candidate.candidate;
                                        for existing in &candidate.existing {
                                            if existing.superseded_at.is_some() {
                                                continue;
                                            }
                                            // Same guards as Update path: skip low-confidence
                                            // or non-user-stated facts, and same-session writes
                                            if existing.confidence < 0.7
                                                || existing.source != "user_stated"
                                            {
                                                continue;
                                            }
                                            if existing.object != new.object
                                                && existing.subject == new.subject
                                                && existing.predicate == new.predicate
                                                && !is_same_session(
                                                    &existing.recorded_at,
                                                    &session_start,
                                                )
                                            {
                                                bus.publish(DomainEvent::ContradictionDetected {
                                                    existing_subject: existing.subject.clone(),
                                                    existing_predicate: existing.predicate.clone(),
                                                    existing_object: existing.object.clone(),
                                                    new_object: new.object.clone(),
                                                    confidence: new.confidence,
                                                });
                                            }
                                        }
                                    }
                                    _ => {}
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

                // Handle accumulation (legacy path — now fed only by DLQ/promotions)
                for (key, obs) in to_accumulate {
                    if let Some(ref ar) = accum_repo {
                        ar.insert(&key, &obs).await;
                    }

                    let entry = accumulator
                        .entry(key.clone())
                        .or_insert_with(AccumulatedEntry::new);
                    entry.add(obs);

                    // Use General domain as default since accumulator doesn't track per-domain
                    if entry.should_promote_for_domain(
                        &ai_core::RecallDomain::General,
                        &promote_overrides,
                        promote_threshold,
                        min_days,
                    ) {
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

                // Prevent unbounded accumulator growth — evict oldest entries
                const MAX_ACCUMULATOR_ENTRIES: usize = 500;
                if accumulator.len() > MAX_ACCUMULATOR_ENTRIES {
                    // Keep entries with the most observations (most interesting patterns)
                    let mut entries: Vec<(String, usize)> = accumulator
                        .iter()
                        .map(|(k, v)| (k.clone(), v.observations.len()))
                        .collect();
                    entries.sort_by(|a, b| a.1.cmp(&b.1)); // ascending by count
                    let to_remove = accumulator.len() - MAX_ACCUMULATOR_ENTRIES;
                    for (key, _) in entries.into_iter().take(to_remove) {
                        accumulator.remove(&key);
                        if let Some(ref ar) = accum_repo {
                            ar.delete_by_key(&key).await;
                        }
                    }
                    info!("Accumulator pruned: removed {to_remove} low-count entries");
                }

                // ── Drain unified pipeline signals ─────────────────────
                if let (Some(ref mut rx), Some(ref rr)) = (&mut signal_rx, &rule_repo) {
                    let mut signals = Vec::new();
                    while let Ok(signal) = rx.try_recv() {
                        signals.push(signal);
                    }
                    if !signals.is_empty() {
                        debug!("Unified pipeline: draining {} signal(s)", signals.len());
                        let clusters = crate::pipeline::group_signals(signals);
                        let ops = if intelligence_mode == config::schema::IntelligenceMode::Deep {
                            if let Some(ref handler) = deep_handler {
                                crate::pipeline::deep_promote(&clusters, handler.as_ref()).await
                            } else {
                                crate::pipeline::heuristic_promote(&clusters)
                            }
                        } else {
                            crate::pipeline::heuristic_promote(&clusters)
                        };
                        if !ops.is_empty() {
                            crate::pipeline::execute_promotions(
                                &ops,
                                &repo,
                                rr,
                                &episodic_repo,
                                embedder.as_deref(),
                            )
                            .await;
                        }
                    }
                }

                batch_count += 1;
                if batch_count.is_multiple_of(100) {
                    if let Some(ref dlq) = failed_obs_repo {
                        let removed = dlq.cleanup_permanently_failed().await;
                        if removed > 0 {
                            info!(
                                "DLQ cleanup: removed {} permanently failed observations",
                                removed
                            );
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

/// Create a type key for an event (used as accumulator key). Retained for
/// now as it is still referenced by legacy unit tests; primary pipeline
/// flow is via `IngestionConsumer` + `AiSignal::event_kind`.
#[cfg(test)]
fn event_type_key(event: &DomainEvent) -> &'static str {
    match event {
        DomainEvent::ActivitySessionCompleted { .. } => "ActivitySessionCompleted",
        DomainEvent::FocusSessionStarted { .. } => "FocusSessionStarted",
        DomainEvent::FocusSessionEnded { .. } => "FocusSessionEnded",
        DomainEvent::DistractionDetected { .. } => "DistractionDetected",
        DomainEvent::ProductivityScoreComputed { .. } => "ProductivityScoreComputed",
        DomainEvent::TaskCreated { .. } => "TaskCreated",
        DomainEvent::TaskCompleted { .. } => "TaskCompleted",
        DomainEvent::TaskDeferred { .. } => "TaskDeferred",

        DomainEvent::TransactionRecorded { .. } => "TransactionRecorded",
        DomainEvent::BudgetAlert { .. } => "BudgetAlert",
        DomainEvent::ChatTurnCompleted { .. } => "ChatTurnCompleted",
        DomainEvent::UserStatedFact { .. } => "UserStatedFact",
        DomainEvent::UserCorrectedAI { .. } => "UserCorrectedAI",
        DomainEvent::CoachingFeedback { .. } => "CoachingFeedback",
        DomainEvent::NoteCreated { .. } => "NoteCreated",
        DomainEvent::NoteUpdated { .. } => "NoteUpdated",
        DomainEvent::SessionCreated { .. } => "SessionCreated",
        DomainEvent::SessionEnded { .. } => "SessionEnded",
        DomainEvent::QualityScored { .. } => "QualityScored",
        DomainEvent::ToolCallExecuted { .. } => "ToolCallExecuted",
        DomainEvent::BehavioralPatternDetected { .. } => "BehavioralPatternDetected",
        DomainEvent::EstimationRecorded { .. } => "EstimationRecorded",
        DomainEvent::AutotunerDecision { .. } => "AutotunerDecision",
        DomainEvent::ContradictionDetected { .. } => "ContradictionDetected",
        DomainEvent::NoteContentChanged { .. } => "NoteContentChanged",
        DomainEvent::NoteDeleted { .. } => "NoteDeleted",
        DomainEvent::KnowledgeAtomCreated { .. } => "KnowledgeAtomCreated",
        DomainEvent::KnowledgeAtomAccepted { .. } => "KnowledgeAtomAccepted",
        DomainEvent::KnowledgeAtomArchived { .. } => "KnowledgeAtomArchived",
        DomainEvent::AtomFlashcardReviewed { .. } => "AtomFlashcardReviewed",
        DomainEvent::AtomReinforced { .. } => "AtomReinforced",
        DomainEvent::AtomInteracted { .. } => "AtomInteracted",
        DomainEvent::RetentionMilestoneReached { .. } => "RetentionMilestoneReached",
        DomainEvent::TranslationCompleted { .. } => "TranslationCompleted",
        DomainEvent::NoteStudied { .. } => "NoteStudied",
        DomainEvent::PracticeUnitCompleted { .. } => "PracticeUnitCompleted",
        DomainEvent::PracticeSessionCompleted { .. } => "PracticeSessionCompleted",
        DomainEvent::KnowledgeTransferDetected { .. } => "KnowledgeTransferDetected",
        DomainEvent::CoachingLearningDigest { .. } => "CoachingLearningDigest",
        DomainEvent::FlashcardSessionCompleted { .. } => "FlashcardSessionCompleted",
        DomainEvent::InterventionTriggered { .. } => "InterventionTriggered",
        DomainEvent::MemoryPendingConfirmation { .. } => "MemoryPendingConfirmation",
        DomainEvent::SkillRouted { .. } => "SkillRouted",

        DomainEvent::NoteEditingFinished { .. } => "NoteEditingFinished",
        _ => "Unknown",
    }
}

/// Check if a fact was recorded in the current session.
fn is_same_session(recorded_at: &str, session_start: &str) -> bool {
    let recorded = recorded_at
        .parse::<jiff::Timestamp>()
        .unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    let start = session_start
        .parse::<jiff::Timestamp>()
        .unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    ((recorded.as_millisecond() - start.as_millisecond()) / 1000).abs() < 300
}

/// Upsert a domain object into the entities table.
///
/// Silently swallows errors — this is a best-effort bridge; failures must
/// not disrupt the main cognitive pipeline.
async fn upsert_domain_entity(
    entity_repo: &crate::repos::EntityRepo,
    name: &str,
    entity_type: &str,
    source_id: &str,
) {
    let _ = entity_repo
        .upsert_entity(&crate::repos::NewEntity {
            name: name.to_string(),
            entity_type: entity_type.to_string(),
            description: None,
            source: "domain_event".to_string(),
            source_id: Some(source_id.to_string()),
            metadata: None,
        })
        .await;
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
        timestamp: Timestamp::now(),
    }
}

/// Generate a concise one-line summary from observation content.
/// For enriched multi-turn context, extracts just the last user message.
fn summarize_observation(content: &str) -> String {
    // For enriched ChatTurnCompleted content (multi-line [role]: text format),
    // extract just the last user message
    let last_user_line = content
        .lines()
        .rev()
        .find(|l| l.starts_with("[user]:"))
        .map(|l| {
            l.trim_start_matches("[user]: ")
                .trim_start_matches("[user]:")
                .trim()
        });

    let base = last_user_line.unwrap_or(content);

    if base.len() <= 120 {
        base.to_string()
    } else {
        let truncated = &base[..120];
        match truncated.rfind(' ') {
            Some(pos) if pos > 60 => format!("{}...", &truncated[..pos]),
            _ => format!("{truncated}..."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                timestamp: ts.parse::<jiff::Timestamp>().unwrap(),
            });
        }

        assert!(entry.should_promote_for_domain(
            &ai_core::RecallDomain::General,
            &std::collections::HashMap::new(),
            5,
            3,
        ));
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

        assert!(!entry.should_promote_for_domain(
            &ai_core::RecallDomain::General,
            &std::collections::HashMap::new(),
            5,
            3,
        )); // Not enough days
    }

    #[test]
    fn test_summarize_accumulated() {
        let observations = vec![
            Observation {
                domain: "productivity".into(),
                content: "Score: 72".into(),
                importance: 0.5,
                source_event: "ProductivityScoreComputed".into(),
                timestamp: Timestamp::now(),
            },
            Observation {
                domain: "productivity".into(),
                content: "Score: 78".into(),
                importance: 0.6,
                source_event: "ProductivityScoreComputed".into(),
                timestamp: Timestamp::now(),
            },
        ];

        let summary = summarize_accumulated("ProductivityScoreComputed", &observations);
        assert_eq!(summary.domain, "productivity");
        assert!(summary.content.contains("Score: 72"));
        assert!(summary.content.contains("Score: 78"));
        assert!(summary.content.contains("2 accumulated"));
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

    #[tokio::test]
    async fn domain_event_creates_entity() {
        let pool = crate::repos::cognitive_test_pool().await;
        let entity_repo = crate::repos::EntityRepo::new(pool.clone());

        // Task entity
        upsert_domain_entity(&entity_repo, "task-abc-123", "task", "task-abc-123").await;
        let found = entity_repo.find_by_name("task-abc-123").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, "task");
        assert_eq!(found[0].source, "domain_event");

        // Finance category entity
        upsert_domain_entity(
            &entity_repo,
            "Food & Dining",
            "finance_category",
            "food-dining",
        )
        .await;
        let found = entity_repo.find_by_name("Food & Dining").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, "finance_category");

        // Objective entity
        upsert_domain_entity(
            &entity_repo,
            "obj-revenue-q1",
            "objective",
            "obj-revenue-q1",
        )
        .await;
        let found = entity_repo.find_by_name("obj-revenue-q1").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, "objective");
    }

    #[tokio::test]
    async fn domain_event_entity_increments_mention_count() {
        let pool = crate::repos::cognitive_test_pool().await;
        let entity_repo = crate::repos::EntityRepo::new(pool.clone());

        // Upsert the same entity twice (simulating two TransactionRecorded events
        // for the same category)
        upsert_domain_entity(&entity_repo, "Groceries", "finance_category", "groceries").await;
        upsert_domain_entity(&entity_repo, "Groceries", "finance_category", "groceries").await;

        let found = entity_repo.find_by_name("Groceries").await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].mention_count, 2,
            "mention_count should be 2 after two upserts"
        );
    }

    #[test]
    fn summarize_observation_short_text() {
        let result = summarize_observation("User prefers dark mode");
        assert_eq!(result, "User prefers dark mode");
    }

    #[test]
    fn summarize_observation_long_text() {
        let long = "a ".repeat(100);
        let result = summarize_observation(&long);
        assert!(result.len() <= 125);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn summarize_observation_enriched_context() {
        let enriched =
            "[user]: What language?\n[assistant]: I can help!\n[user]: I prefer Rust for backend";
        let result = summarize_observation(enriched);
        assert_eq!(result, "I prefer Rust for backend");
    }
}
