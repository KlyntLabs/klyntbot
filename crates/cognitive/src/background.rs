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

use crate::consolidation::{consolidate_batch, ConsolidationHandler};
use crate::embedder::SemanticFactEmbedder;
use crate::extraction::{extract_from_observation, ExtractionHandler};
use crate::repos::accumulated_observation::AccumulatedObservationRepo;
use crate::repos::{EpisodicMemoryRepo, SemanticFactRepo};
use crate::salience::evaluate_salience;
use crate::types::{EpisodicMemory, Observation, SalienceVerdict};

/// Debug events emitted by the pipeline for the debug dashboard.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum PipelineEvent {
    /// An extraction step completed.
    Extraction {
        observation: String,
        #[serde(rename = "factsExtracted")]
        facts_extracted: usize,
    },
    /// A consolidation operation was performed.
    Consolidation { operation: String, fact: String },
}

/// Minimum occurrences of an accumulated event type before promoting to extraction.
const ACCUMULATE_PROMOTE_THRESHOLD: usize = 5;
/// Minimum distinct days for accumulated events before promoting.
const ACCUMULATE_MIN_DAYS: usize = 3;

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

    fn should_promote(&self) -> bool {
        self.observations.len() >= ACCUMULATE_PROMOTE_THRESHOLD
            && self.days_seen.len() >= ACCUMULATE_MIN_DAYS
    }
}

/// Background service that processes domain events into cognitive memory.
pub struct BackgroundConsolidationService {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl BackgroundConsolidationService {
    /// Start the background service.
    ///
    /// - `event_rx`: Receiver from `DomainEventBus`
    /// - `extraction`: Handler for extracting facts from observations
    /// - `consolidation`: Handler for consolidation decisions
    /// - `repo`: Semantic fact repository
    /// - `cancel`: Cancellation token for graceful shutdown
    pub fn start(
        mut event_rx: broadcast::Receiver<DomainEvent>,
        extraction: Arc<dyn ExtractionHandler>,
        consolidation: Arc<dyn ConsolidationHandler>,
        repo: SemanticFactRepo,
        episodic_repo: Option<EpisodicMemoryRepo>,
        embedder: Option<Arc<dyn SemanticFactEmbedder>>,
        cancel: CancellationToken,
        pipeline_tx: Option<tokio::sync::broadcast::Sender<PipelineEvent>>,
        accum_repo: Option<AccumulatedObservationRepo>,
    ) -> Self {
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

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    result = event_rx.recv() => {
                        let event = match result {
                            Ok(e) => e,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("BackgroundConsolidation lagged, skipped {n} events");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        let verdict = evaluate_salience(&event);
                        let obs = event_to_observation(&event);

                        match verdict {
                            SalienceVerdict::Extract => {
                                // Hot path: immediate extraction + consolidation
                                if let Some(obs) = obs {
                                    let facts = extract_from_observation(
                                        extraction.as_ref(),
                                        &obs,
                                    ).await;

                                    if let Some(tx) = &pipeline_tx {
                                        let _ = tx.send(PipelineEvent::Extraction {
                                            observation: obs.content.clone(),
                                            facts_extracted: facts.len(),
                                        });
                                    }

                                    if !facts.is_empty() {
                                        let ops = consolidate_batch(
                                            &facts,
                                            &repo,
                                            consolidation.as_ref(),
                                            embedder.as_deref(),
                                        ).await;

                                        if let Some(tx) = &pipeline_tx {
                                            for (fact, op) in facts.iter().zip(ops.iter()) {
                                                let _ = tx.send(PipelineEvent::Consolidation {
                                                    operation: op_to_string(op),
                                                    fact: format!("{}.{} = {}", fact.subject, fact.predicate, fact.object),
                                                });
                                            }
                                        }
                                    }

                                    // Store high-importance observations as episodic memories
                                    if obs.importance >= 0.7 {
                                        if let Some(ep_repo) = &episodic_repo {
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
                                            };
                                            if let Err(e) = ep_repo.insert(&mem).await {
                                                warn!("BackgroundConsolidation: failed to store episodic memory: {e}");
                                            }
                                        }
                                    }
                                }
                            }
                            SalienceVerdict::Accumulate => {
                                if let Some(obs) = obs {
                                    let key = event_type_key(&event);

                                    // Persist observation before adding to in-memory buffer
                                    if let Some(ref ar) = accum_repo {
                                        ar.insert(&key, &obs).await;
                                    }

                                    let entry = accumulator
                                        .entry(key.clone())
                                        .or_insert_with(AccumulatedEntry::new);
                                    entry.add(obs);

                                    // Check for promotion
                                    if entry.should_promote() {
                                        debug!(
                                            "Promoting accumulated events for '{key}' ({} events, {} days)",
                                            entry.observations.len(),
                                            entry.days_seen.len()
                                        );
                                        // Create a summary observation for extraction
                                        let summary = summarize_accumulated(
                                            &key,
                                            &entry.observations,
                                        );
                                        let facts = extract_from_observation(
                                            extraction.as_ref(),
                                            &summary,
                                        ).await;

                                        if let Some(tx) = &pipeline_tx {
                                            let _ = tx.send(PipelineEvent::Extraction {
                                                observation: summary.content.clone(),
                                                facts_extracted: facts.len(),
                                            });
                                        }

                                        if !facts.is_empty() {
                                            let ops = consolidate_batch(
                                                &facts,
                                                &repo,
                                                consolidation.as_ref(),
                                                embedder.as_deref(),
                                            ).await;

                                            if let Some(tx) = &pipeline_tx {
                                                for (fact, op) in facts.iter().zip(ops.iter()) {
                                                    let _ = tx.send(PipelineEvent::Consolidation {
                                                        operation: op_to_string(op),
                                                        fact: format!("{}.{} = {}", fact.subject, fact.predicate, fact.object),
                                                    });
                                                }
                                            }
                                        }
                                        accumulator.remove(&key);
                                        // Clear persisted rows after promotion
                                        if let Some(ref ar) = accum_repo {
                                            ar.delete_by_key(&key).await;
                                        }
                                    }
                                }
                            }
                            SalienceVerdict::Discard => {
                                // Intentionally dropped
                            }
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
        } => Some(Observation {
            domain: "meta".into(),
            content: format!("User corrected: '{original}' → '{correction}'"),
            importance: 1.0,
            source_event: "UserCorrectedAI".into(),
            timestamp: now,
        }),
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
        } => {
            let detail = match (actual_duration_mins, estimated_duration_mins) {
                (Some(actual), Some(est)) => {
                    format!(" (actual: {actual}min, estimated: {est}min)")
                }
                _ => String::new(),
            };
            Some(Observation {
                domain: "tasks".into(),
                content: format!("Task {task_id} completed{detail}"),
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
        _ => {
            // TaskCreated, TaskDeferred, GoalProgress, ActivitySessionCompleted
            // Lower priority — convert generically
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
    }
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

        assert!(entry.should_promote());
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

        assert!(!entry.should_promote()); // Not enough days
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
            }),
            "TaskCompleted"
        );
    }
}
