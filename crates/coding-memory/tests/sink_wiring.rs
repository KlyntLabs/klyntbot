use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use coding_ingest::store::IngestEventLogRepo;
use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
use coding_memory::sink::{InProcessSink, MemorySink};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

#[tokio::test]
async fn in_process_sink_hits_distiller() {
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

    let mut sink = InProcessSink::new();
    sink.set_distiller(distiller.clone());

    let event = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt {
            text: "hi".into(),
            attachments: vec![],
        },
    });
    ingest.insert(&event).await.unwrap();
    sink.accept_event(event).await.unwrap();

    // Fire boundary with assistant msg
    let event2 = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::AssistantMsg {
            text: "done".into(),
            truncated: false,
            token_usage: Some(TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                cached_tokens: None,
            }),
        },
    });
    ingest.insert(&event2).await.unwrap();
    sink.accept_event(event2).await.unwrap();

    // Give the spawned distill_turn a moment to complete
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM episodic_memories WHERE kind = 'turn_trace'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(count.0, 1);
}
