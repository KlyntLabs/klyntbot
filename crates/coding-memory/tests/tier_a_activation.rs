//! Tier A activation scenario — Phase-A extractive pass fires even when
//! Phase-B LLM is unavailable (noop provider). Verifies the distiller
//! boundary detection → turn trace persistence pipeline.

use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use coding_ingest::store::IngestEventLogRepo;
use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

fn evt(session: &str, turn: Option<&str>, kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: session.into(),
        turn_id: turn.map(str::to_string),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

#[tokio::test]
async fn tier_a_produces_turn_trace_without_llm() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let ingest = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );
    let provider = Arc::new(providers::ProviderManager::new(
        Arc::new(providers::NoopProvider),
        None,
        None,
    ));
    let retriever = Arc::new(cognitive::UnifiedMemoryService::new(SemanticFactRepo::new(
        pool.inner().clone(),
    ))) as Arc<dyn context_engine::MemoryRetriever>;

    let distiller = Arc::new(Distiller::new(
        DistillerConfig::default(),
        ingest.clone(),
        writer,
        provider,
        retriever,
    ));

    // Feed events through accept_event (mimics real-time ingestion).
    let e1 = evt(
        "s1",
        Some("t1"),
        EventKind::UserPrompt {
            text: "fix the auth bug".into(),
            attachments: vec![],
        },
    );
    ingest.insert(&e1).await.unwrap();
    distiller.accept_event(e1).await.unwrap();

    let e2 = evt(
        "s1",
        Some("t1"),
        EventKind::FileEdit {
            path: PathBuf::from("src/auth.rs"),
            op: coding_ingest::event::FileOp::Modify,
            bytes: 42,
            diff_preview: None,
        },
    );
    ingest.insert(&e2).await.unwrap();
    distiller.accept_event(e2).await.unwrap();

    // Boundary fires on AssistantMsg with token_usage.
    let e3 = evt(
        "s1",
        Some("t1"),
        EventKind::AssistantMsg {
            text: "Fixed!".into(),
            truncated: false,
            token_usage: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                cached_tokens: None,
            }),
        },
    );
    ingest.insert(&e3).await.unwrap();
    distiller.accept_event(e3).await.unwrap();

    // Give the spawned distill_turn a moment to complete.
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;

    // Tier A (Phase A) should have produced a turn_trace even with NoopProvider.
    let trace_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM episodic_memories WHERE kind = 'turn_trace'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(trace_count.0, 1, "expected exactly one turn_trace episode");

    // No semantic facts should be written (NoopProvider can't synthesize).
    let fact_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM semantic_facts WHERE metadata IS NOT NULL")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(
        fact_count.0, 0,
        "expected no semantic facts with noop provider"
    );

    // Events should be marked processed.
    assert_eq!(ingest.count_unprocessed().await.unwrap(), 0);
}
