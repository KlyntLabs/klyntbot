use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
use cognitive::consumers::IngestionConsumer;
use jiff::Timestamp;

async fn setup_test_repos() -> (
    sqlx::SqlitePool,
    cognitive::repos::AccumulatedObservationRepo,
    cognitive::repos::EntityRepo,
    cognitive::repos::EpisodicMemoryRepo,
) {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate!("../storage/migrations")
        .run(&pool)
        .await
        .unwrap();
    let migrations = cognitive::repos::cognitive_migrations();
    storage::StoragePool::run_feature_migrations(&pool, &migrations)
        .await
        .unwrap();

    let observation_repo = cognitive::repos::AccumulatedObservationRepo::new(pool.clone());
    let entity_repo = cognitive::repos::EntityRepo::new(pool.clone());
    let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
    (pool, observation_repo, entity_repo, episodic_repo)
}

#[tokio::test]
async fn extract_verdict_writes_observation() {
    let (_pool, observation_repo, entity_repo, episodic_repo) = setup_test_repos().await;
    let consumer = IngestionConsumer::new(
        observation_repo.clone(),
        entity_repo.clone(),
        episodic_repo.clone(),
        None,
    );

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "Completed",
        importance: 0.8,
        salience: SalienceVerdict::Extract,
        content: "Completed: Ship v1 (deviation 80%)".into(),
        entity: None,
        timestamp: Timestamp::now(),
        raw_event: None,
    };

    consumer.consume(&sig).await.unwrap();

    let loaded = observation_repo.load_all().await;
    assert_eq!(loaded.len(), 1);
    let entry = loaded.values().next().unwrap();
    assert_eq!(entry.observations.len(), 1);
    assert_eq!(entry.observations[0].importance, 0.8);
    assert_eq!(entry.observations[0].domain, "tasks");
}

#[tokio::test]
async fn discard_verdict_skips_observation() {
    let (_pool, observation_repo, entity_repo, episodic_repo) = setup_test_repos().await;
    let consumer = IngestionConsumer::new(
        observation_repo.clone(),
        entity_repo.clone(),
        episodic_repo.clone(),
        None,
    );

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "Heartbeat",
        importance: 0.1,
        salience: SalienceVerdict::Discard,
        content: "ping".into(),
        entity: None,
        timestamp: Timestamp::now(),
        raw_event: None,
    };

    consumer.consume(&sig).await.unwrap();

    let loaded = observation_repo.load_all().await;
    assert!(loaded.is_empty());
}

#[tokio::test]
async fn high_importance_creates_episodic_memory() {
    let (_pool, observation_repo, entity_repo, episodic_repo) = setup_test_repos().await;
    let consumer = IngestionConsumer::new(
        observation_repo.clone(),
        entity_repo.clone(),
        episodic_repo.clone(),
        None,
    );

    let sig = AiSignal {
        domain: RecallDomain::Finance,
        event_kind: "Overspent",
        importance: 0.9,
        salience: SalienceVerdict::Accumulate,
        content: "Dining budget exceeded by 40%".into(),
        entity: None,
        timestamp: Timestamp::now(),
        raw_event: None,
    };

    consumer.consume(&sig).await.unwrap();

    let episodic = episodic_repo.list_recent(10).await.unwrap();
    assert_eq!(episodic.len(), 1);
    assert_eq!(episodic[0].domain, "finance");
    assert!((episodic[0].importance - 0.9).abs() < f64::EPSILON);
}

#[tokio::test]
async fn low_importance_skips_episodic_memory() {
    let (_pool, observation_repo, entity_repo, episodic_repo) = setup_test_repos().await;
    let consumer = IngestionConsumer::new(
        observation_repo.clone(),
        entity_repo.clone(),
        episodic_repo.clone(),
        None,
    );

    let sig = AiSignal {
        domain: RecallDomain::General,
        event_kind: "Log",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: "Routine check".into(),
        entity: None,
        timestamp: Timestamp::now(),
        raw_event: None,
    };

    consumer.consume(&sig).await.unwrap();

    let episodic = episodic_repo.list_recent(10).await.unwrap();
    assert!(episodic.is_empty());
}

#[tokio::test]
async fn entity_signal_upserts_entity() {
    let (_pool, observation_repo, entity_repo, episodic_repo) = setup_test_repos().await;
    let consumer = IngestionConsumer::new(
        observation_repo.clone(),
        entity_repo.clone(),
        episodic_repo.clone(),
        None,
    );

    let sig = AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "Created",
        importance: 0.6,
        salience: SalienceVerdict::Accumulate,
        content: "New task: Refactor auth".into(),
        entity: Some(ai_core::EntityRef {
            entity_type: "task",
            id: "task-123".to_string(),
            name: "Refactor auth".to_string(),
        }),
        timestamp: Timestamp::now(),
        raw_event: None,
    };

    consumer.consume(&sig).await.unwrap();

    let found = entity_repo.find_by_name("Refactor auth").await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].entity_type, "task");
}
