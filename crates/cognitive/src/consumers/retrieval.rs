//! `RetrievalIndexer` — `SignalConsumer` that projects `AiSignal`s into the
//! `ai_signal_index` table for fast domain-scoped recall.
//!
//! Filter gates:
//! 1. `SalienceVerdict::Discard` → skip. The pipeline already decided this
//!    signal shouldn't extract or accumulate; retrieval gains nothing.
//! 2. `importance < min_importance` → skip. Keeps the index dense with
//!    useful content rather than bloated with low-signal events.
//!
//! The default min-importance floor is 0.2; callers can override via
//! `with_min_importance` if a subsystem wants a tighter or looser gate.

use ai_core::{AiSignal, SalienceVerdict, SignalConsumer};
use async_trait::async_trait;
use uuid::Uuid;

use crate::repos::ai_signal_index::AiSignalIndexRepo;

pub struct RetrievalIndexer {
    repo: AiSignalIndexRepo,
    min_importance: f64,
}

impl RetrievalIndexer {
    pub fn new(repo: AiSignalIndexRepo) -> Self {
        Self {
            repo,
            min_importance: 0.2,
        }
    }

    pub fn with_min_importance(mut self, threshold: f64) -> Self {
        self.min_importance = threshold.clamp(0.0, 1.0);
        self
    }

}

#[async_trait]
impl SignalConsumer for RetrievalIndexer {
    fn name(&self) -> &'static str {
        "retrieval_indexer"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if matches!(signal.salience, SalienceVerdict::Discard) {
            return Ok(());
        }
        if signal.importance < self.min_importance {
            return Ok(());
        }

        let id = Uuid::new_v4().to_string();
        let (entity_type, entity_id) = match &signal.entity {
            Some(e) => (Some(e.entity_type), Some(e.id.as_str())),
            None => (None, None),
        };

        self.repo
            .insert(
                &id,
                signal.domain,
                signal.event_kind,
                &signal.content,
                signal.importance,
                signal.salience.as_str(),
                entity_type,
                entity_id,
                &signal.timestamp.to_string(),
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiSignal, EntityRef, RecallDomain};
    use bus::DomainEvent;

    fn sample_signal(imp: f64, salience: SalienceVerdict) -> AiSignal {
        AiSignal {
            domain: RecallDomain::Tasks,
            event_kind: bus::DomainEvent::KIND_TASK_CREATED,
            importance: imp,
            salience,
            content: "user created a task".into(),
            entity: Some(EntityRef {
                entity_type: "task",
                id: "t1".into(),
                name: "do thing".into(),
            }),
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(DomainEvent::TaskCreated {
                task_id: "t1".into(),
                project: None,
                estimate_mins: None,
                task_type: "basic".into(),
            }),
            metrics: ai_core::AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
            metric_samples: Vec::new(),
        }
    }

    #[tokio::test]
    async fn consumes_high_importance_signal() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = AiSignalIndexRepo::new(pool);
        let indexer = RetrievalIndexer::new(repo.clone());
        let signal = sample_signal(0.7, SalienceVerdict::Extract);
        indexer.consume(&signal).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn discard_is_skipped() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = AiSignalIndexRepo::new(pool);
        let indexer = RetrievalIndexer::new(repo.clone());
        let signal = sample_signal(0.9, SalienceVerdict::Discard);
        indexer.consume(&signal).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn low_importance_is_skipped() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = AiSignalIndexRepo::new(pool);
        let indexer = RetrievalIndexer::new(repo.clone());
        let signal = sample_signal(0.1, SalienceVerdict::Extract);
        indexer.consume(&signal).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn threshold_override_lowers_gate() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = AiSignalIndexRepo::new(pool);
        let indexer = RetrievalIndexer::new(repo.clone()).with_min_importance(0.05);
        let signal = sample_signal(0.1, SalienceVerdict::Extract);
        indexer.consume(&signal).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn recent_query_returns_indexed_rows() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = AiSignalIndexRepo::new(pool);
        let indexer = RetrievalIndexer::new(repo.clone());
        indexer
            .consume(&sample_signal(0.5, SalienceVerdict::Extract))
            .await
            .unwrap();
        let recent = repo
            .recent_by_domain(RecallDomain::Tasks, 10)
            .await
            .unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].event_kind, bus::DomainEvent::KIND_TASK_CREATED);
        assert_eq!(recent[0].entity_type.as_deref(), Some("task"));
        assert_eq!(recent[0].entity_id.as_deref(), Some("t1"));
    }
}
