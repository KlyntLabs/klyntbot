//! Background consolidation service for accumulated observations.
//!
//! Accumulated events are buffered and promoted to extraction when patterns
//! emerge. The old salience-based event classification has been removed;
//! SignalRouter now handles event-to-signal translation upstream.

use std::sync::Arc;

use jiff::Timestamp;
use serde::Serialize;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::DomainEvent;

use crate::consolidation::ConsolidationHandler;
use crate::embedder::SemanticFactEmbedder;
use crate::extraction::ExtractionHandler;

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
/// KCA Track 1: enriched with 1-hop neighborhood + cross-entity facts.
pub(crate) async fn prefetch_existing(
    repo: &SemanticFactRepo,
    entity_repo: &crate::repos::EntityRepo,
    candidates: Vec<crate::types::SemanticFact>,
) -> Vec<crate::consolidation::ConsolidationCandidate> {
    use futures_util::stream::{FuturesUnordered, StreamExt};

    let mut tasks: FuturesUnordered<_> = candidates
        .into_iter()
        .map(|cand| {
            let r = repo.clone();
            let e = entity_repo.clone();
            async move {
                let existing = r
                    .find_similar(&cand.subject, &cand.predicate)
                    .await
                    .unwrap_or_default();

                // Subject neighborhood
                let subject_neighborhood = match e.find_by_name(&cand.subject).await {
                    Ok(rows) if !rows.is_empty() => {
                        let node = &rows[0];
                        e.get_neighborhood_with_edges(&node.id, 1)
                            .await
                            .unwrap_or_default()
                            .into_iter()
                            .map(|ne| (ne.neighbor.name, ne.relationship_type))
                            .collect()
                    }
                    _ => Vec::new(),
                };

                // Object neighborhood (only if object looks like an entity name)
                let object_neighborhood = if looks_like_entity_name(&cand.object) {
                    match e.find_by_name(&cand.object).await {
                        Ok(rows) if !rows.is_empty() => {
                            let node = &rows[0];
                            e.get_neighborhood_with_edges(&node.id, 1)
                                .await
                                .unwrap_or_default()
                                .into_iter()
                                .map(|ne| (ne.neighbor.name, ne.relationship_type))
                                .collect()
                        }
                        _ => Vec::new(),
                    }
                } else {
                    Vec::new()
                };

                // Cross-entity facts: top-5 facts mentioning the subject entity, dedup against `existing`.
                let cross_entity_facts = match e.find_by_name(&cand.subject).await {
                    Ok(rows) if !rows.is_empty() => {
                        let node = &rows[0];
                        let all = r
                            .find_facts_by_entity_id(&node.id, 10)
                            .await
                            .unwrap_or_default();
                        all.into_iter()
                            .filter(|f| {
                                !(f.subject == cand.subject && f.predicate == cand.predicate)
                            })
                            .take(5)
                            .collect()
                    }
                    _ => Vec::new(),
                };

                crate::consolidation::ConsolidationCandidate {
                    candidate: cand,
                    existing,
                    subject_neighborhood,
                    object_neighborhood,
                    cross_entity_facts,
                }
            }
        })
        .collect();

    let mut out = Vec::new();
    while let Some(c) = tasks.next().await {
        out.push(c);
    }
    out
}

fn looks_like_entity_name(s: &str) -> bool {
    s.len() >= 3
        && s.len() <= 100
        && !s
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
}

/// Run the extraction critic on a batch of facts (KCA Track 5).
pub async fn run_critic_judgment(
    turn_text: &str,
    facts: Vec<crate::types::SemanticFact>,
    handler: std::sync::Arc<dyn crate::services::extraction_critic::ExtractionCriticHandler>,
    fact_repo: &SemanticFactRepo,
    log_repo: &crate::repos::ExtractionCriticLogRepo,
) {
    use crate::services::extraction_critic_types::*;
    if facts.is_empty() {
        return;
    }

    let input = ExtractionCriticInput {
        turn_text: turn_text.to_string(),
        extracted_facts: facts
            .iter()
            .map(|f| FactRef {
                fact_id: f.id.clone(),
                subject: f.subject.clone(),
                predicate: f.predicate.clone(),
                object: f.object.clone(),
            })
            .collect(),
    };
    let out = match handler.judge(input).await {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(error = %e, "critic call failed");
            return;
        }
    };

    for v in &out.verdicts {
        if let Err(e) = log_repo.insert(&v.fact_id, &v.verdict, &v.reason).await {
            tracing::warn!(error = %e, "critic log insert failed");
        }
        if v.verdict == "hallucinated" {
            // Demote stability ×0.5 instead of supersede; nightly Reforge re-evaluates.
            if let Err(e) = fact_repo.scale_stability(&v.fact_id, 0.5).await {
                tracing::warn!(error = %e, "stability scale failed");
            }
        }
    }
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
    pub failed_obs_repo: Option<FailedObservationRepo>,
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
    /// Optional graph link handler — fire-and-forget LLM enrichment for newly written facts.
    pub graph_link_handler: Option<Arc<dyn crate::services::graph_linker::GraphLinkHandler>>,
    /// Optional extraction critic handler — KCA Track 5.
    pub critic_handler:
        Option<Arc<dyn crate::services::extraction_critic::ExtractionCriticHandler>>,
    /// Optional extraction critic log repo — KCA Track 5.
    pub critic_log_repo: Option<crate::repos::ExtractionCriticLogRepo>,
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
            failed_obs_repo,
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
            graph_link_handler,
            critic_handler,
            critic_log_repo,
        } = config;
        // Cognitive collectors are SignalConsumers wired by the app-core init
        // layer; the legacy broadcast-collector startup that lived here is
        // gone. `signal_rx` / `domain_bus` are consumed by the downstream
        // rule-evolution paths below.
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
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
                // handles observation extraction and periodic compaction.
                // The old classify_batch + event_to_observation path has been removed.
                let mut to_extract: Vec<Observation> = Vec::new();

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
                                    kind: None,
                                    scope_repo_id: None,
                                    metadata: None,
                                    actor_id: None,
                                    tier: "raw".to_string(),
                                    parent_id: None,
                                    child_count: 0,
                                    rolled_up_at: None,
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
                    let extracted_facts: Vec<crate::types::SemanticFact> = result
                        .extractions
                        .iter()
                        .flat_map(|ext| {
                            let obs = to_extract.get(ext.observation_index);
                            ext.facts.iter().filter_map(move |f| {
                                obs.map(|o| crate::extraction::to_semantic_fact(f, o))
                            })
                        })
                        .collect();

                    // KCA Track 5: fire-and-forget extraction critic.
                    if let Some(critic) = critic_handler.as_ref() {
                        let critic = critic.clone();
                        let facts_clone = extracted_facts.clone();
                        let turn_text = to_extract
                            .iter()
                            .map(|o| o.content.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let fr = repo.clone();
                        let lg = critic_log_repo.clone();
                        tokio::spawn(async move {
                            if let Some(lg) = lg {
                                run_critic_judgment(&turn_text, facts_clone, critic, &fr, &lg)
                                    .await;
                            }
                        });
                    }

                    let entity_repo = crate::repos::EntityRepo::new(repo.pool().clone());
                    let candidates = prefetch_existing(&repo, &entity_repo, extracted_facts).await;

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

                        if let Some(ref linker) = graph_link_handler {
                            let written: Vec<(crate::types::SemanticFact, crate::types::MemoryOp)> =
                                candidates
                                    .iter()
                                    .zip(ops.iter())
                                    .filter(|(_, op)| {
                                        matches!(
                                            op,
                                            crate::types::MemoryOp::Add { .. }
                                                | crate::types::MemoryOp::Update { .. }
                                        )
                                    })
                                    .map(|(c, op)| (c.candidate.clone(), op.clone()))
                                    .collect();
                            if !written.is_empty() {
                                let linker = linker.clone();
                                let fr = repo.clone();
                                let er = entity_repo.clone();
                                tokio::spawn(async move {
                                    run_post_consolidation_linker(&fr, &er, linker, written, None)
                                        .await;
                                });
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
                                pipeline_tx.as_ref(),
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

/// KCA Track 2 — fire-and-forget graph linker. Called after entity-edge writes for
/// each Add/Update fact. Errors are logged, never propagated.
pub async fn run_post_consolidation_linker(
    fact_repo: &SemanticFactRepo,
    entity_repo: &crate::repos::EntityRepo,
    handler: Arc<dyn crate::services::graph_linker::GraphLinkHandler>,
    written: Vec<(crate::types::SemanticFact, crate::types::MemoryOp)>,
    recent_user_text: Option<String>,
) {
    use crate::services::graph_linker_types::*;

    for (fact, op) in written {
        if !matches!(
            op,
            crate::types::MemoryOp::Add { .. } | crate::types::MemoryOp::Update { .. }
        ) {
            continue;
        }

        let subject_node = entity_repo
            .find_by_name(&fact.subject)
            .await
            .ok()
            .and_then(|r| r.into_iter().next());
        let object_node = if looks_like_entity_name(&fact.object) {
            entity_repo
                .find_by_name(&fact.object)
                .await
                .ok()
                .and_then(|r| r.into_iter().next())
        } else {
            None
        };

        let subject_neighborhood = match &subject_node {
            Some(n) => entity_repo
                .get_neighborhood_with_edges(&n.id, 1)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|ne| NeighborRef {
                    entity_id: ne.neighbor.id,
                    name: ne.neighbor.name,
                    relationship_type: ne.relationship_type,
                    strength: ne.strength,
                })
                .collect(),
            None => Vec::new(),
        };
        let object_neighborhood = match &object_node {
            Some(n) => entity_repo
                .get_neighborhood_with_edges(&n.id, 1)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|ne| NeighborRef {
                    entity_id: ne.neighbor.id,
                    name: ne.neighbor.name,
                    relationship_type: ne.relationship_type,
                    strength: ne.strength,
                })
                .collect(),
            None => Vec::new(),
        };

        let candidate_facts = match &subject_node {
            Some(n) => fact_repo
                .find_facts_by_entity_id(&n.id, 5)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|f| f.id != fact.id)
                .map(|f| ExistingFactRef {
                    fact_id: f.id,
                    subject: f.subject,
                    predicate: f.predicate,
                    object: f.object,
                    valid_at: f.valid_from.clone(),
                    valid_until: f.valid_until.clone(),
                })
                .collect(),
            None => Vec::new(),
        };

        let input = GraphLinkInput {
            new_fact: NewFactRef {
                fact_id: fact.id.clone(),
                subject: fact.subject.clone(),
                subject_entity_id: subject_node.as_ref().map(|n| n.id.clone()),
                predicate: fact.predicate.clone(),
                object: fact.object.clone(),
                object_entity_id: object_node.as_ref().map(|n| n.id.clone()),
                confidence: fact.confidence,
                valid_at: fact.valid_from.clone(),
            },
            subject_neighborhood,
            object_neighborhood,
            candidate_facts,
            recent_user_text: recent_user_text.clone(),
        };

        match handler.link(input).await {
            Ok(out) => {
                if let Err(e) = apply_graph_link_output(fact_repo, entity_repo, &fact, &out).await {
                    tracing::warn!(error = %e, fact_id = %fact.id, "graph_linker: apply failed");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, fact_id = %fact.id, "graph_linker: handler returned error");
            }
        }
    }
}

async fn apply_graph_link_output(
    fact_repo: &SemanticFactRepo,
    entity_repo: &crate::repos::EntityRepo,
    fact: &crate::types::SemanticFact,
    out: &crate::services::graph_linker_types::GraphLinkOutput,
) -> common::Result<()> {
    // 1. Apply discovered relationships (typed edges).
    for rel in &out.discovered_relationships {
        let src = match entity_repo.find_by_name(&rel.source_entity_name).await {
            Ok(rows) => rows.into_iter().next(),
            Err(_) => None,
        };
        let tgt = match entity_repo.find_by_name(&rel.target_entity_name).await {
            Ok(rows) => rows.into_iter().next(),
            Err(_) => None,
        };
        if let (Some(s), Some(t)) = (src, tgt) {
            if let Err(e) = entity_repo
                .upsert_relationship_typed(
                    &s.id,
                    &t.id,
                    &rel.relationship_type,
                    &rel.edge_type,
                    rel.strength,
                    Some(&rel.evidence),
                    "graph_linker",
                )
                .await
            {
                tracing::debug!(error = %e, "linker: upsert_relationship_typed failed");
            }
        }
    }

    // 2. Apply supersedes.
    for sup in &out.superseded {
        if let Err(e) = fact_repo.supersede(&sup.old_fact_id, &fact.id).await {
            tracing::debug!(error = %e, old = %sup.old_fact_id, "linker: supersede failed");
        }
    }

    // 3. Apply merges (entity merges). Conservative: only emit a merge_proposal row.
    // Actual merging happens in nightly Reforge Phase 6.5 to avoid corrupting in-flight state.
    for merge in &out.merges {
        if let Err(e) = entity_repo
            .record_merge_proposal(
                &merge.entity_a_id,
                &merge.entity_b_id,
                &merge.canonical_name,
                &merge.reason,
                "graph_linker",
            )
            .await
        {
            tracing::debug!(error = %e, "linker: record_merge_proposal failed");
        }
    }

    Ok(())
}

/// Phase 3: Run graph consolidation over the current active fact set.
///
/// Reuses `run_post_consolidation_linker` over a snapshot of active
/// facts, marking each as a synthetic Add op. Call between batch
/// ingest and QA in the bench path; in production the same function
/// fires periodically when the per-conversation fact counter crosses
/// `cognitive.consolidate_threshold`.
///
/// Returns the number of facts processed.
pub async fn run_graph_consolidation(
    fact_repo: &SemanticFactRepo,
    entity_repo: &crate::repos::EntityRepo,
    handler: Arc<dyn crate::services::graph_linker::GraphLinkHandler>,
    max_facts: usize,
) -> u32 {
    // Load a bounded snapshot of recent active facts. We don't process
    // the full table to keep latency predictable for large datasets;
    // the most recent N is where the new entity edges will land
    // anyway.
    let facts = match fact_repo.list_all_active().await {
        Ok(rows) => rows.into_iter().take(max_facts).collect::<Vec<_>>(),
        Err(e) => {
            tracing::warn!(error = %e, "graph_consolidation: failed to list active facts");
            return 0;
        }
    };
    let count = facts.len() as u32;
    if count == 0 {
        return 0;
    }
    let written: Vec<(crate::types::SemanticFact, crate::types::MemoryOp)> = facts
        .into_iter()
        .map(|f| {
            let id = f.id.clone();
            (f, crate::types::MemoryOp::Add { id })
        })
        .collect();
    run_post_consolidation_linker(fact_repo, entity_repo, handler, written, None).await;
    tracing::info!(facts_processed = count, "graph consolidation complete");
    count
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

    #[tokio::test]
    async fn prefetch_existing_includes_neighborhood_when_entities_exist() {
        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = crate::repos::SemanticFactRepo::new(pool.clone());
        let entity_repo = crate::repos::EntityRepo::new(pool.clone());

        // Seed: Alice—knows—Bob (already in graph)
        let alice = entity_repo
            .upsert_entity(&crate::repos::NewEntity {
                name: "Alice".into(),
                entity_type: "person".into(),
                description: None,
                source: "t".into(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();
        let bob = entity_repo
            .upsert_entity(&crate::repos::NewEntity {
                name: "Bob".into(),
                entity_type: "person".into(),
                description: None,
                source: "t".into(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();
        entity_repo
            .upsert_relationship(&crate::repos::NewRelationship {
                source_entity_id: alice.id.clone(),
                target_entity_id: bob.id.clone(),
                relationship_type: "knows".into(),
                evidence: None,
                source: "t".into(),
            })
            .await
            .unwrap();
        let fact1 = crate::types::SemanticFact {
            id: "f1".into(),
            domain: "test".into(),
            subject: "Alice".into(),
            predicate: "knows".into(),
            object: "Bob".into(),
            confidence: 0.8,
            source: "t".into(),
            valid_from: "2026-04-29".into(),
            valid_until: None,
            recorded_at: "2026-04-29".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".into(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            speaker: None,
        };
        let fact2 = crate::types::SemanticFact {
            id: "f2".into(),
            domain: "test".into(),
            subject: "Bob".into(),
            predicate: "manages".into(),
            object: "Alice".into(),
            confidence: 0.9,
            source: "t".into(),
            valid_from: "2026-04-29".into(),
            valid_until: None,
            recorded_at: "2026-04-29".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".into(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            speaker: None,
        };
        fact_repo.upsert(&fact1).await.unwrap();
        fact_repo.upsert(&fact2).await.unwrap();

        // New extracted fact: Alice prefers Rust
        let new_fact = crate::types::SemanticFact {
            id: "f3".into(),
            domain: "test".into(),
            subject: "Alice".into(),
            predicate: "prefers".into(),
            object: "Rust".into(),
            confidence: 0.7,
            source: "t".into(),
            valid_from: "2026-04-29".into(),
            valid_until: None,
            recorded_at: "2026-04-29".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".into(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            speaker: None,
        };

        let candidates = prefetch_existing(&fact_repo, &entity_repo, vec![new_fact.clone()]).await;

        assert_eq!(candidates.len(), 1);
        let cand = &candidates[0];
        assert_eq!(cand.candidate.subject, "Alice");

        // Subject neighborhood should include Bob via "knows"
        let nbrs: Vec<&str> = cand
            .subject_neighborhood
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        assert!(
            nbrs.contains(&"Bob"),
            "subject neighborhood must include Bob, got {:?}",
            cand.subject_neighborhood
        );

        // Cross-entity facts should include Bob's works_at fact
        let cef_subjects: Vec<&str> = cand
            .cross_entity_facts
            .iter()
            .map(|f| f.subject.as_str())
            .collect();
        assert!(
            cef_subjects.contains(&"Bob"),
            "cross-entity facts should include Bob's facts, got {:?}",
            cef_subjects
        );
    }

    use crate::services::graph_linker::GraphLinkHandler;
    use crate::services::graph_linker_types::{GraphLinkInput, GraphLinkOutput};
    use std::sync::Mutex;

    struct CapturingLinker(Mutex<Vec<GraphLinkInput>>);

    #[async_trait::async_trait]
    impl GraphLinkHandler for CapturingLinker {
        async fn link(&self, input: GraphLinkInput) -> common::Result<GraphLinkOutput> {
            self.0.lock().unwrap().push(input);
            Ok(GraphLinkOutput::default())
        }
    }

    #[tokio::test]
    async fn linker_invoked_after_add_op_with_neighborhood() {
        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let entity_repo = crate::repos::EntityRepo::new(pool.clone());

        let alice = entity_repo
            .upsert_entity(&crate::repos::NewEntity {
                name: "Alice".into(),
                entity_type: "person".into(),
                description: None,
                source: "t".into(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();
        let bob = entity_repo
            .upsert_entity(&crate::repos::NewEntity {
                name: "Bob".into(),
                entity_type: "person".into(),
                description: None,
                source: "t".into(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();
        entity_repo
            .upsert_relationship(&crate::repos::NewRelationship {
                source_entity_id: alice.id.clone(),
                target_entity_id: bob.id.clone(),
                relationship_type: "knows".into(),
                evidence: None,
                source: "t".into(),
            })
            .await
            .unwrap();

        let linker = Arc::new(CapturingLinker(Mutex::new(Vec::new())));

        let new_fact = crate::types::SemanticFact {
            id: "f3".into(),
            domain: "test".into(),
            subject: "Alice".into(),
            predicate: "prefers".into(),
            object: "Rust".into(),
            confidence: 0.7,
            source: "t".into(),
            valid_from: "2026-04-29".into(),
            valid_until: None,
            recorded_at: "2026-04-29".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".into(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            speaker: None,
        };
        fact_repo.upsert(&new_fact).await.unwrap();

        run_post_consolidation_linker(
            &fact_repo,
            &entity_repo,
            linker.clone() as Arc<dyn GraphLinkHandler>,
            vec![(
                new_fact.clone(),
                crate::types::MemoryOp::Add {
                    id: new_fact.id.clone(),
                },
            )],
            None,
        )
        .await;

        let captured = linker.0.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].new_fact.subject, "Alice");
        assert!(
            !captured[0].subject_neighborhood.is_empty(),
            "neighborhood must be populated"
        );
    }

    #[tokio::test]
    async fn linker_skipped_for_cold_start_facts() {
        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let entity_repo = crate::repos::EntityRepo::new(pool.clone());

        let linker = Arc::new(CapturingLinker(Mutex::new(Vec::new())));

        let new_fact = crate::types::SemanticFact {
            id: "f3".into(),
            domain: "test".into(),
            subject: "Zoe".into(),
            predicate: "loves".into(),
            object: "skydiving".into(),
            confidence: 0.7,
            source: "t".into(),
            valid_from: "2026-04-29".into(),
            valid_until: None,
            recorded_at: "2026-04-29".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: crate::types::DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".into(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
            speaker: None,
        };
        fact_repo.upsert(&new_fact).await.unwrap();

        run_post_consolidation_linker(
            &fact_repo,
            &entity_repo,
            linker.clone() as Arc<dyn GraphLinkHandler>,
            vec![(
                new_fact.clone(),
                crate::types::MemoryOp::Add {
                    id: new_fact.id.clone(),
                },
            )],
            None,
        )
        .await;

        let cap = linker.0.lock().unwrap();
        assert_eq!(cap.len(), 1);
        assert!(cap[0].subject_neighborhood.is_empty());
        assert!(cap[0].object_neighborhood.is_empty());
        assert!(cap[0].candidate_facts.is_empty());
    }

    #[tokio::test]
    async fn critic_demotes_stability_for_hallucinated_facts() {
        use crate::services::extraction_critic::ExtractionCriticHandler;
        use crate::services::extraction_critic_types::*;

        struct FakeCritic;
        #[async_trait::async_trait]
        impl ExtractionCriticHandler for FakeCritic {
            async fn judge(
                &self,
                input: ExtractionCriticInput,
            ) -> common::Result<ExtractionCriticOutput> {
                Ok(ExtractionCriticOutput {
                    verdicts: input
                        .extracted_facts
                        .iter()
                        .enumerate()
                        .map(|(i, f)| Verdict {
                            fact_id: f.fact_id.clone(),
                            verdict: if i % 2 == 0 {
                                "grounded".into()
                            } else {
                                "hallucinated".into()
                            },
                            reason: "test".into(),
                        })
                        .collect(),
                    missed_facts: vec![],
                })
            }
        }

        let pool = crate::repos::cognitive_test_pool().await;
        let fact_repo = crate::repos::SemanticFactRepo::new(pool.clone());
        let crit_log = crate::repos::ExtractionCriticLogRepo::new(
            storage::StoragePool::from_existing(pool.clone()),
        );

        let f1 = crate::types::SemanticFact {
            id: "f1".into(),
            domain: "t".into(),
            subject: "A".into(),
            predicate: "p".into(),
            object: "B".into(),
            confidence: 0.8,
            source: "t".into(),
            valid_from: "2026-04-29".into(),
            recorded_at: "2026-04-29".into(),
            stability: 1.0,
            ..Default::default()
        };
        let f2 = crate::types::SemanticFact {
            id: "f2".into(),
            domain: "t".into(),
            subject: "X".into(),
            predicate: "p".into(),
            object: "Y".into(),
            confidence: 0.8,
            source: "t".into(),
            valid_from: "2026-04-29".into(),
            recorded_at: "2026-04-29".into(),
            stability: 1.0,
            ..Default::default()
        };
        fact_repo.upsert(&f1).await.unwrap();
        fact_repo.upsert(&f2).await.unwrap();

        run_critic_judgment(
            "User said A is B.",
            vec![f1.clone(), f2.clone()],
            std::sync::Arc::new(FakeCritic),
            &fact_repo,
            &crit_log,
        )
        .await;

        // f2 should have stability halved.
        let f2_after = fact_repo.get(&f2.id).await.unwrap().unwrap();
        let f1_after = fact_repo.get(&f1.id).await.unwrap().unwrap();
        assert!(
            f2_after.stability < f1_after.stability,
            "hallucinated fact stability {} should be < grounded {}",
            f2_after.stability,
            f1_after.stability
        );
        let log = crit_log.list_unreviewed(10).await.unwrap();
        assert_eq!(log.len(), 2);
    }
}
