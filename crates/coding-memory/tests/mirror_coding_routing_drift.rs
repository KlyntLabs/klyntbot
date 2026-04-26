use ai_core::{AiMetrics, AiSignal, MirrorSignalSource, RecallDomain, SalienceVerdict};
use coding_memory::mirror::coding_routing::CodingRoutingSource;
use storage::StoragePool;

fn skill_routed_signal(skill: &str, repo: Option<&str>, confidence: f64) -> AiSignal {
    AiSignal {
        domain: RecallDomain::General,
        event_kind: "SkillRouted".into(),
        importance: 0.4,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(bus::DomainEvent::SkillRouted {
            skill_name: skill.into(),
            confidence,
            source: format!("project:{}", repo.unwrap_or("none")),
            trigger_phrases: vec![],
            session_key: "s".into(),
        }),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

#[tokio::test]
async fn project_skill_50pct_drop_emits_alert() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let mirror_repo = cognitive::mirror::MirrorRepo::new(pool.clone());
    let source = CodingRoutingSource::new(mirror_repo);

    for _ in 0..10 {
        source
            .accumulate(&skill_routed_signal("klyntbot-r1-fmt", Some("r1"), 0.85))
            .await
            .unwrap();
    }
    source.flush().await.unwrap();

    for _ in 0..4 {
        source
            .accumulate(&skill_routed_signal("klyntbot-r1-fmt", Some("r1"), 0.85))
            .await
            .unwrap();
    }
    for _ in 0..10 {
        source
            .accumulate(&skill_routed_signal("klyntbot-r1-test", Some("r1"), 0.85))
            .await
            .unwrap();
    }
    source.flush().await.unwrap();

    let alert_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM mirror_snippets WHERE coding_alert_kind = 'project_skill_obsolete'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert!(alert_count.0 >= 1);
}
