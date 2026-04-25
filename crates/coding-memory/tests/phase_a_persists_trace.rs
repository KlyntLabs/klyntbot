use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_memory::distiller::phase_a::{compute_turn_trace, persist_turn_trace};
use coding_memory::distiller::DistillerWriter;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use std::path::PathBuf;
use storage::StoragePool;
use uuid::Uuid;

#[tokio::test]
async fn persist_turn_trace_writes_episode_with_provenance_and_kind() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );

    let src_id = Uuid::new_v4();
    let events = vec![AgentEvent::V1(AgentEventV1 {
        id: src_id,
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "hi".into(), attachments: vec![] },
    })];

    let trace = compute_turn_trace("s1", Some("t1"), &events);
    let prov = ProvenanceMetadata {
        source_events: vec![src_id],
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        distilled_at: Timestamp::now(),
        distiller_model: "claude-haiku-4-5".into(),
        source_kind: ProvenanceKind::DistillerExtractive,
    };
    let id = persist_turn_trace(&writer, &trace, None, &prov).await.unwrap();

    let (kind, meta_json): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT kind, metadata FROM episodic_memories WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_one(pool.inner()).await.unwrap();
    assert_eq!(kind.as_deref(), Some("turn_trace"));
    let meta: serde_json::Value = serde_json::from_str(&meta_json.unwrap()).unwrap();
    assert_eq!(meta["provenance"]["sourceKind"], "distiller_extractive");
}
