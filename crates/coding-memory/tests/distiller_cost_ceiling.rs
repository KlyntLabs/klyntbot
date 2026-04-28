//! Distiller halts Phase B when daily cost ceiling is breached.

use coding_memory::distiller::{Distiller, DistillerConfig, DistillerError};
use std::sync::Arc;

mod common;

#[tokio::test]
async fn phase_b_halts_at_ceiling() {
    let pool = common::pool_with_migrations().await;
    let writer = common::writer_for_pool(&pool);
    let ingest = Arc::new(coding_ingest::store::IngestEventLogRepo::new(
        pool.inner().clone(),
    ));
    let retriever = Arc::new(cognitive::UnifiedMemoryService::new(
        cognitive::SemanticFactRepo::new(pool.inner().clone()),
    )) as Arc<dyn context_engine::MemoryRetriever>;

    let distiller = Distiller::new(
        DistillerConfig {
            cost_ceiling_usd: Some(0.0),
            ..DistillerConfig::default()
        },
        ingest,
        writer,
        Arc::new(providers::ProviderManager::new(
            Arc::new(providers::NoopProvider),
            None,
            None,
        )),
        retriever,
    );

    // Seed a minimal event so distill_turn has work.
    let event = coding_ingest::event::AgentEvent::V1(coding_ingest::event::AgentEventV1 {
        id: uuid::Uuid::new_v4(),
        source: coding_ingest::event::AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: std::path::PathBuf::from("/tmp"),
        repo: None,
        occurred_at: jiff::Timestamp::now(),
        kind: coding_ingest::event::EventKind::UserPrompt {
            text: "hello".into(),
            attachments: vec![],
        },
    });
    distiller.accept_event(event).await.unwrap();

    let result = distiller.distill_turn("s1", Some("t1")).await;
    assert!(result.is_err(), "expected error, got {result:?}");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cost ceiling"), "expected 'cost ceiling' in error: {err}");
}
