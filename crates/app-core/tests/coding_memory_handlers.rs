//! Integration tests for the 4 Phase-3 coding-memory handlers
//! (Memory Browser, Activity Timeline, Cost Tracker, Sensitivity Inspector).

use app_core::coding_memory::handlers;
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use jiff::Timestamp;
use std::path::PathBuf;
use storage::StoragePool;
use uuid::Uuid;

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

fn evt(kind: EventKind) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: Some("t".into()),
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    })
}

#[tokio::test]
async fn memory_browser_returns_active_coding_facts_only() {
    let pool = fresh_pool().await;
    let now = Timestamp::now().to_string();
    // Two active coding-domain facts + one superseded (valid_until set) + one off-domain.
    sqlx::query(
        "INSERT INTO semantic_facts (id,domain,subject,predicate,object,confidence,source,valid_from,recorded_at)
         VALUES ('a','coding','user','prefers','rust',0.9,'distiller',?,?),
                ('b','preferences','user','prefers','dark-mode',0.9,'distiller',?,?),
                ('c','coding','user','prefers','old-rust',0.9,'distiller',?,?),
                ('d','health','user','steps','12k',0.9,'distiller',?,?)",
    )
    .bind(&now).bind(&now)
    .bind(&now).bind(&now)
    .bind(&now).bind(&now)
    .bind(&now).bind(&now)
    .execute(pool.inner())
    .await
    .unwrap();
    sqlx::query("UPDATE semantic_facts SET valid_until = ? WHERE id = 'c'")
        .bind(&now)
        .execute(pool.inner())
        .await
        .unwrap();

    let rows = handlers::memory_browser(pool.inner(), 10, 0).await.unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"b"));
    assert!(!ids.contains(&"c"), "superseded row must be filtered");
    assert!(!ids.contains(&"d"), "off-domain row must be filtered");
}

#[tokio::test]
async fn activity_timeline_buckets_by_day() {
    let pool = fresh_pool().await;
    let ingest = IngestEventLogRepo::new(pool.inner().clone());
    for _ in 0..3 {
        ingest
            .insert(&evt(EventKind::UserPrompt {
                text: "x".into(),
                attachments: vec![],
            }))
            .await
            .unwrap();
    }

    let buckets = handlers::activity_timeline(pool.inner(), 7).await.unwrap();
    let total: i64 = buckets.iter().map(|b| b.count).sum();
    assert_eq!(total, 3, "all 3 events must show in the rollup");
    assert!(buckets.iter().all(|b| !b.date.is_empty()));
}

#[tokio::test]
async fn cost_breakdown_returns_per_model_rows_with_aggregates() {
    let pool = fresh_pool().await;
    sqlx::query(
        "INSERT INTO ingest_event_log (id, source, session_id, cwd, occurred_at, kind, payload)
         VALUES ('s0','claude-code','sa','/tmp', datetime('now'), 'sessionStart',
                 '{\"kind\":{\"kind\":\"sessionStart\",\"model\":\"claude-sonnet-4-6\"}}'),
                ('a1','claude-code','sa','/tmp', datetime('now'), 'assistantMsg',
                 '{\"kind\":{\"kind\":\"assistantMsg\",\"token_usage\":{\"prompt_tokens\":1000000,\"completion_tokens\":500000,\"cached_tokens\":50000}}}'),
                ('s1','claude-code','sb','/tmp', datetime('now'), 'sessionStart',
                 '{\"kind\":{\"kind\":\"sessionStart\",\"model\":\"deepseek-chat\"}}'),
                ('a2','claude-code','sb','/tmp', datetime('now'), 'assistantMsg',
                 '{\"kind\":{\"kind\":\"assistantMsg\",\"token_usage\":{\"prompt_tokens\":100000,\"completion_tokens\":40000}}}')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let bd = handlers::cost_breakdown(pool.inner(), 7).await.unwrap();
    assert_eq!(bd.rows.len(), 2, "expected 2 (date, model) rows");
    assert_eq!(bd.model_count, 2);
    assert_eq!(bd.day_count, 1);
    assert_eq!(bd.total_input_tokens, 1_100_000);
    assert_eq!(bd.total_output_tokens, 540_000);
    assert_eq!(bd.total_cached_tokens, 50_000);

    // Sonnet 4.6 long-context (1M > 200K): 1M × $3 × 2 + 500k × $15 × 1.5 = $6 + $11.25 = $17.25
    let sonnet = bd
        .rows
        .iter()
        .find(|r| r.model == "claude-sonnet-4-6")
        .unwrap();
    assert!((sonnet.total_cost_usd - 17.25).abs() < 1e-6);
    assert!((sonnet.input_cost_usd - 6.0).abs() < 1e-6);
    assert!((sonnet.output_cost_usd - 11.25).abs() < 1e-6);

    // DeepSeek: 100k × $0.27 + 40k × $1.10 = $0.027 + $0.044 = $0.071
    let deepseek = bd.rows.iter().find(|r| r.model == "deepseek-chat").unwrap();
    assert!((deepseek.total_cost_usd - 0.071).abs() < 1e-6);
    assert_eq!(deepseek.cached_tokens, 0, "no cached field defaults to 0");

    // Sonnet long-context $17.25 + DeepSeek $0.071 = $17.321
    assert!((bd.total_cost_usd - 17.321).abs() < 1e-6);
}

#[tokio::test]
async fn sensitivity_inspector_extracts_tier_from_metadata() {
    let pool = fresh_pool().await;
    let now = Timestamp::now().to_string();
    sqlx::query(
        "INSERT INTO semantic_facts (id,domain,subject,predicate,object,confidence,source,valid_from,recorded_at,metadata)
         VALUES ('a','coding','user','prefers','rust',0.9,'distiller',?,?, '{\"sensitivity\":\"secret\"}'),
                ('b','coding','user','prefers','tabs',0.9,'distiller',?,?, '{\"foo\":\"bar\"}')",
    )
    .bind(&now).bind(&now)
    .bind(&now).bind(&now)
    .execute(pool.inner())
    .await
    .unwrap();

    let rows = handlers::sensitivity_inspector(pool.inner(), 10, 0)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    let a = rows.iter().find(|r| r.id == "a").unwrap();
    let b = rows.iter().find(|r| r.id == "b").unwrap();
    assert_eq!(a.sensitivity, "secret");
    assert_eq!(
        b.sensitivity, "public",
        "missing field defaults to 'public'"
    );
}
