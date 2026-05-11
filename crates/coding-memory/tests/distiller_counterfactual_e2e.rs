//! End-to-end test: a `FixAttempt` observation with `outcome: failure` flowing
//! through `Distiller::distill_turn` derives a counterfactual `SemanticFact`
//! via `derive_dead_end`, written through `DistillerWriter` with full
//! provenance.

use async_trait::async_trait;
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use coding_ingest::store::IngestEventLogRepo;
use coding_memory::distiller::{Distiller, DistillerConfig, DistillerWriter};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use providers::types::{ChatParams, LlmResponse, Message, ToolCall, Usage};
use providers::{LlmProvider, ProviderManager};
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

struct FixedProvider(Vec<ToolCall>);

#[async_trait]
impl LlmProvider for FixedProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[serde_json::Value]>,
        _params: &ChatParams,
        _breakpoints: &[providers::types::CacheBreakpoint],
    ) -> common::Result<LlmResponse> {
        Ok(LlmResponse {
            content: Some(String::new()),
            tool_calls: self.0.clone(),
            finish_reason: "stop".into(),
            usage: Usage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
            reasoning_content: None,
        })
    }
    fn default_model(&self) -> &str {
        "fixed"
    }
    fn name(&self) -> &str {
        "fixed-provider"
    }
}

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

async fn build_distiller(pool: &StoragePool, observation: serde_json::Value) -> Distiller {
    let ingest = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    ingest
        .insert(&evt(
            "s1",
            Some("t1"),
            EventKind::UserPrompt {
                text: "fix the auth bug".into(),
                attachments: vec![],
            },
        ))
        .await
        .unwrap();
    ingest
        .insert(&evt(
            "s1",
            Some("t1"),
            EventKind::AssistantMsg {
                text: "tried adding a retry; reverted — root cause was elsewhere".into(),
                truncated: false,
                token_usage: Some(TokenUsage {
                    prompt_tokens: 50,
                    completion_tokens: 20,
                    cached_tokens: None,
                }),
            },
        ))
        .await
        .unwrap();

    let writer = DistillerWriter::new(
        SemanticFactRepo::new(pool.inner().clone()),
        EpisodicMemoryRepo::new(pool.inner().clone()),
    );
    let provider = Arc::new(ProviderManager::new(
        Arc::new(FixedProvider(vec![ToolCall {
            id: "c1".into(),
            name: "record_observation".into(),
            arguments: observation,
        }])),
        None,
        None,
    ));
    let retriever = Arc::new(cognitive::UnifiedMemoryService::new(SemanticFactRepo::new(
        pool.inner().clone(),
    ))) as Arc<dyn context_engine::MemoryRetriever>;

    Distiller::new(
        DistillerConfig::default(),
        ingest,
        writer,
        provider,
        retriever,
    )
}

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn fix_attempt_failure_emits_counterfactual_dead_end_fact() {
    let pool = fresh_pool().await;
    let observation = serde_json::json!({
        "kind": "fix_attempt",
        "subject": "auth bug — token expires mid-request",
        "predicate": "tried",
        "object": "added retry-with-backoff in middleware",
        "confidence": 0.8,
        "scope": "repo",
        "reasoning": "tests still failed; reverted",
        "outcome": "failure",
    });
    let distiller = build_distiller(&pool, observation).await;

    let report = distiller.distill_turn("s1", Some("t1")).await.unwrap();
    assert!(report.episodic_writes >= 1, "fix_attempt episode must land");
    assert!(
        report.semantic_writes >= 1,
        "counterfactual semantic fact must be written"
    );

    let dead_end: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM semantic_facts
         WHERE memory_type = 'counterfactual'
           AND source = 'distiller_counterfactual'
           AND predicate = 'dead_end'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(
        dead_end.0, 1,
        "exactly one DeadEndAttempt fact should be persisted"
    );

    // Provenance invariant: counterfactual rows MUST carry source_events.
    let prov: (Option<String>,) = sqlx::query_as(
        "SELECT metadata FROM semantic_facts WHERE memory_type = 'counterfactual' LIMIT 1",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    let meta = prov.0.expect("metadata required for counterfactual");
    assert!(
        meta.contains("\"sourceEvents\""),
        "metadata must include sourceEvents (provenance-always invariant)"
    );
}

#[tokio::test]
async fn fix_attempt_success_does_not_emit_counterfactual() {
    let pool = fresh_pool().await;
    let observation = serde_json::json!({
        "kind": "fix_attempt",
        "subject": "auth bug",
        "predicate": "fixed",
        "object": "rotated the JWT signing key",
        "confidence": 0.9,
        "scope": "repo",
        "reasoning": "tests pass",
        "outcome": "success",
    });
    let distiller = build_distiller(&pool, observation).await;

    let _ = distiller.distill_turn("s1", Some("t1")).await.unwrap();

    let dead_end: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM semantic_facts WHERE memory_type = 'counterfactual'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(
        dead_end.0, 0,
        "successful fix must not derive a dead-end fact"
    );
}

#[tokio::test]
async fn fix_attempt_abandoned_emits_counterfactual() {
    let pool = fresh_pool().await;
    let observation = serde_json::json!({
        "kind": "fix_attempt",
        "subject": "flaky CI test",
        "predicate": "tried",
        "object": "increased timeout to 30s",
        "confidence": 0.6,
        "scope": "global",
        "reasoning": "still flaky; switched to a different approach",
        "outcome": "abandoned",
    });
    let distiller = build_distiller(&pool, observation).await;

    let _ = distiller.distill_turn("s1", Some("t1")).await.unwrap();

    let dead_end: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM semantic_facts WHERE memory_type = 'counterfactual'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(dead_end.0, 1, "abandoned outcome must derive a dead-end");
}
