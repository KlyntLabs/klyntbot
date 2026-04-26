use ai_core::{AiMetrics, AiSignal, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::stale_memory::StaleMemorySource;
use storage::StoragePool;

fn retrieved_signal(memory_ids: &[&str], session_id: &str, turn_id: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "MemoryRetrieved",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::MemoryRetrieved {
            memory_ids: memory_ids.iter().map(|s| s.to_string()).collect(),
            query: "q".into(),
            session_id: session_id.into(),
            turn_id: Some(turn_id.into()),
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

fn cited_signal(cited: &[&str], session_id: &str, turn_id: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "AssistantMsgCompleted",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::AssistantMsgCompleted {
            session_id: session_id.into(),
            turn_id: Some(turn_id.into()),
            cited_memory_ids: cited.iter().map(|s| s.to_string()).collect(),
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

#[tokio::test]
async fn cited_memory_marked_in_utilization_table() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let source = StaleMemorySource::new(pool.clone());
    source
        .accumulate(&retrieved_signal(&["m1", "m2"], "s1", "t1"))
        .await
        .unwrap();
    source
        .accumulate(&cited_signal(&["m1"], "s1", "t1"))
        .await
        .unwrap();
    source.flush().await.unwrap();

    let m1_cited: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM memory_utilization WHERE memory_id='m1' AND cited_in_response=1",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(m1_cited.0, 1);
    let m2_uncited: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM memory_utilization WHERE memory_id='m2' AND cited_in_response=0",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(m2_uncited.0, 1);
}
