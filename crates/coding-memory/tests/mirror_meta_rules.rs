use ai_core::{AiMetrics, AiSignal, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::coding_meta_rules::CodingMetaRulesSource;
use storage::StoragePool;

fn fix_failed_signal(problem_hash: &str, attempt_count: u32) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "FixAttemptFailed".into(),
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::FixAttemptFailed {
            problem_hash: problem_hash.into(),
            repo: Some("r1".into()),
            attempt_count,
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

#[tokio::test]
async fn problem_class_refactor_fires_at_threshold() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let source = CodingMetaRulesSource::new(mirror_repo);

    source
        .accumulate(&fix_failed_signal("h1", 1))
        .await
        .unwrap();
    source
        .accumulate(&fix_failed_signal("h1", 2))
        .await
        .unwrap();
    source
        .accumulate(&fix_failed_signal("h1", 3))
        .await
        .unwrap();

    let alerts: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM mirror_snippets WHERE coding_alert_kind = 'problem_class_refactor'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(alerts.0, 1);
}
