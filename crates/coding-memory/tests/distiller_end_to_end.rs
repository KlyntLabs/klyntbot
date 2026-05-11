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

/// Mock provider that returns a fixed `record_observation` tool call.
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
            content: Some("".into()),
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

#[tokio::test]
async fn distill_turn_writes_turn_trace_plus_repo_context_fact() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let ingest = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    ingest
        .insert(&evt(
            "s1",
            Some("t1"),
            EventKind::UserPrompt {
                text: "what framework does this repo use?".into(),
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
                text: "It's a Tauri 2 app.".into(),
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

    let observation = serde_json::json!({
        "kind": "repo_context",
        "subject": "repo:unknown",
        "predicate": "framework",
        "object": "tauri",
        "confidence": 0.9,
        "scope": "repo",
        "reasoning": "assistant stated explicitly"
    });
    let provider = Arc::new(ProviderManager::new(
        Arc::new(FixedProvider(vec![ToolCall {
            id: "call1".into(),
            name: "record_observation".into(),
            arguments: observation,
        }])),
        None,
        None,
    ));

    let retriever = Arc::new(cognitive::UnifiedMemoryService::new(SemanticFactRepo::new(
        pool.inner().clone(),
    ))) as Arc<dyn context_engine::MemoryRetriever>;

    let distiller = Distiller::new(
        DistillerConfig::default(),
        ingest.clone(),
        writer,
        provider,
        retriever,
    );
    let report = distiller.distill_turn("s1", Some("t1")).await.unwrap();
    assert!(
        report.episodic_writes >= 1,
        "expected at least one turn_trace episode"
    );
    assert!(
        report.semantic_writes >= 1,
        "expected at least one fact from the LLM observation"
    );

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM episodic_memories WHERE kind = 'turn_trace'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(count.0, 1);

    let fact_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM semantic_facts WHERE subject LIKE 'repo:%' AND predicate = 'framework' AND metadata IS NOT NULL",
    ).fetch_one(pool.inner()).await.unwrap();
    assert_eq!(fact_count.0, 1);

    assert_eq!(ingest.count_unprocessed().await.unwrap(), 0);
}
