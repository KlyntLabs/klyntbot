use ai_core::{AiMetrics, AiSignal, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::pattern_effectiveness::{
    PatternEffectivenessLogRepo, PatternEffectivenessSource,
};
use std::sync::Arc;
use storage::StoragePool;

fn pattern_outcome_signal(pattern_id: &str, outcome: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "PatternOutcome",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::PatternOutcome {
            pattern_id: pattern_id.to_string(),
            outcome: outcome.to_string(),
            evidence: String::new(),
            measured_at: jiff::Timestamp::now().to_string(),
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

#[tokio::test]
async fn updates_effectiveness_via_ema() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO procedural_rules \
         (id, domain, rule_text, confidence, source, signal_count, created_at, updated_at, active, \
          scope_type, scope_id, effectiveness_score, stability, scope_repo_id, application_count) \
         VALUES ('p1','coding','rule','observed',0.8,0,?1,?1,1,'code',NULL,0.5,1.0,'r1',0)",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let log = PatternEffectivenessLogRepo::new(pool.clone());
    let source = Arc::new(PatternEffectivenessSource::new(pool.clone(), log));
    source
        .accumulate(&pattern_outcome_signal("p1", "success"))
        .await
        .unwrap();
    source.flush().await.unwrap();

    let (score,): (f32,) =
        sqlx::query_as("SELECT effectiveness_score FROM procedural_rules WHERE id = 'p1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert!((score - (0.9 * 0.5 + 0.1 * 1.0)).abs() < 1e-3);

    let log_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM pattern_effectiveness_log WHERE pattern_id = 'p1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(log_count.0, 1);
}
