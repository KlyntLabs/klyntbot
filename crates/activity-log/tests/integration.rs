//! Integration tests for the unified activity log.

use std::sync::Arc;

use chrono::{Duration, Utc};
use storage::StoragePool;
use tokio_util::sync::CancellationToken;

use activity_log::{
    ActivityIngestionService, ActivityLog, ActivityLogRepo, ActivitySource, ChatMessageInput,
    ChatMessageNormalizer, DomainEventNormalizer, PrivacyFilter, WindowEventInput,
    WindowEventNormalizer,
};

async fn setup() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &ActivityLog::migrations_static())
        .await
        .unwrap();
    pool
}

/// Full flow: DomainEvent fires → subscriber catches → normalizes → ingests → query returns it.
#[tokio::test]
async fn test_domain_event_full_flow() {
    let pool = setup().await;
    let svc = Arc::new(ActivityIngestionService::new(
        pool.clone(),
        PrivacyFilter::default(),
    ));

    let domain_bus = bus::DomainEventBus::new(64);
    let cancel = CancellationToken::new();

    let _subscriber =
        activity_log::ActivityLogSubscriber::start(&domain_bus, Arc::clone(&svc), cancel.clone());

    // Publish a TaskCreated event
    domain_bus.publish(bus::DomainEvent::TaskCreated {
        task_id: "integration-t1".into(),
        project: Some("test-project".into()),
        estimate_mins: Some(60),
        task_type: "manual".into(),
    });

    // Give the subscriber time to process
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let start = Utc::now() - Duration::hours(1);
    let end = Utc::now() + Duration::hours(1);
    let results = ActivityLogRepo::query_range(&pool, start, end, 100, 0)
        .await
        .unwrap();

    assert!(!results.is_empty(), "Should have at least one entry");
    let entry = &results[0];
    assert_eq!(entry.source, ActivitySource::Task);
    assert_eq!(entry.action, "create");
    assert_eq!(entry.resource_id.as_deref(), Some("integration-t1"));

    cancel.cancel();
}

/// Full flow: DomainEvent fires multiple events → subscriber ingests all → query returns them.
#[tokio::test]
async fn test_multiple_domain_events() {
    let pool = setup().await;
    let svc = Arc::new(ActivityIngestionService::new(
        pool.clone(),
        PrivacyFilter::default(),
    ));

    let domain_bus = bus::DomainEventBus::new(64);
    let cancel = CancellationToken::new();

    let _subscriber =
        activity_log::ActivityLogSubscriber::start(&domain_bus, Arc::clone(&svc), cancel.clone());

    domain_bus.publish(bus::DomainEvent::FocusSessionStarted {
        session_type: "deep_work".into(),
        target_mins: 25,
    });
    domain_bus.publish(bus::DomainEvent::NoteCreated {
        note_id: "n-100".into(),
        title: "Integration Note".into(),
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let start = Utc::now() - Duration::hours(1);
    let end = Utc::now() + Duration::hours(1);

    let focus =
        ActivityLogRepo::query_by_source(&pool, ActivitySource::FocusSession, start, end, 10)
            .await
            .unwrap();
    assert!(!focus.is_empty());
    assert_eq!(focus[0].action, "start");

    let notes = ActivityLogRepo::query_by_source(&pool, ActivitySource::Note, start, end, 10)
        .await
        .unwrap();
    assert!(!notes.is_empty());
    assert_eq!(notes[0].action, "create");

    cancel.cancel();
}

/// Chat message flow: normalize → ingest → query_by_session returns it.
#[tokio::test]
async fn test_chat_message_flow() {
    let pool = setup().await;
    let svc = ActivityIngestionService::new(pool.clone(), PrivacyFilter::default());

    let normalizer = ChatMessageNormalizer;
    let user_msg = ChatMessageInput {
        session_key: "telegram:user123".into(),
        role: "user".into(),
        content: "What tasks do I have today?".into(),
    };
    let assistant_msg = ChatMessageInput {
        session_key: "telegram:user123".into(),
        role: "assistant".into(),
        content: "You have 3 tasks due today.".into(),
    };

    svc.normalize_and_ingest(&normalizer, &user_msg)
        .await
        .unwrap();
    svc.normalize_and_ingest(&normalizer, &assistant_msg)
        .await
        .unwrap();

    let start = Utc::now() - Duration::hours(1);
    let end = Utc::now() + Duration::hours(1);

    let results = ActivityLogRepo::query_by_session(&pool, "telegram:user123", start, end)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    // Results are DESC, so assistant reply comes first
    assert!(results.iter().any(|e| e.action == "prompt"));
    assert!(results.iter().any(|e| e.action == "reply"));
}

/// Window event flow: normalize → ingest → query returns it.
#[tokio::test]
async fn test_window_event_flow() {
    let pool = setup().await;
    let svc = ActivityIngestionService::new(pool.clone(), PrivacyFilter::default());

    let normalizer = WindowEventNormalizer;
    let input = WindowEventInput {
        app_name: "Visual Studio Code".into(),
        window_title: Some("main.rs — klyntbot".into()),
        url: None,
        started_at: Utc::now(),
        duration_secs: Some(300),
        project_id: Some("proj-klyntbot".into()),
        is_idle: false,
    };

    svc.normalize_and_ingest(&normalizer, &input).await.unwrap();

    let start = Utc::now() - Duration::hours(1);
    let end = Utc::now() + Duration::hours(1);

    let results = ActivityLogRepo::query_by_project(&pool, "proj-klyntbot", start, end)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, ActivitySource::OsWindow);
    assert_eq!(results[0].app_name.as_deref(), Some("Visual Studio Code"));
}

/// Source stats: count_by_source returns correct breakdown.
#[tokio::test]
async fn test_count_by_source() {
    let pool = setup().await;
    let svc = Arc::new(ActivityIngestionService::new(
        pool.clone(),
        PrivacyFilter::default(),
    ));

    let domain_bus = bus::DomainEventBus::new(64);
    let cancel = CancellationToken::new();
    let _subscriber =
        activity_log::ActivityLogSubscriber::start(&domain_bus, Arc::clone(&svc), cancel.clone());

    domain_bus.publish(bus::DomainEvent::TaskCreated {
        task_id: "t1".into(),
        project: None,
        estimate_mins: None,
        task_type: "manual".into(),
    });
    domain_bus.publish(bus::DomainEvent::TaskCompleted {
        task_id: "t1".into(),
        actual_duration_mins: Some(30),
        estimated_duration_mins: Some(45),
        deviation_pct: None,
    });
    domain_bus.publish(bus::DomainEvent::NoteCreated {
        note_id: "n1".into(),
        title: "Test".into(),
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let start = Utc::now() - Duration::hours(1);
    let end = Utc::now() + Duration::hours(1);

    let counts = ActivityLogRepo::count_by_source(&pool, start, end)
        .await
        .unwrap();

    assert_eq!(counts.get("task"), Some(&2));
    assert_eq!(counts.get("note"), Some(&1));

    cancel.cancel();
}

/// Dedup: same content_hash → only first is stored.
#[tokio::test]
async fn test_dedup_across_ingestion() {
    let pool = setup().await;
    let svc = ActivityIngestionService::new(pool.clone(), PrivacyFilter::default());

    let normalizer = DomainEventNormalizer;
    let event = bus::DomainEvent::TaskCreated {
        task_id: "dedup-test".into(),
        project: None,
        estimate_mins: None,
        task_type: "manual".into(),
    };

    // Normalize twice — same event produces same content_hash
    let entry1 = activity_log::ActivityNormalizer::normalize(&normalizer, &event).unwrap();
    let hash = entry1.content_hash.clone();

    let id1 = svc.ingest(entry1).await.unwrap();
    assert!(id1.is_some());

    // Create another entry with the same hash
    let mut entry2 = activity_log::ActivityNormalizer::normalize(&normalizer, &event).unwrap();
    entry2.content_hash = hash;
    let id2 = svc.ingest(entry2).await.unwrap();
    assert!(id2.is_none(), "Duplicate should be skipped");
}

/// Retention cleanup: delete_older_than removes old entries.
#[tokio::test]
async fn test_retention_cleanup() {
    let pool = setup().await;
    let svc = ActivityIngestionService::new(pool.clone(), PrivacyFilter::default());

    // Insert an entry with a timestamp 400 days ago
    let mut entry = activity_log::normalize_domain_event(&bus::DomainEvent::NoteCreated {
        note_id: "old-note".into(),
        title: "Old".into(),
    });
    entry.timestamp = Utc::now() - Duration::days(400);
    entry.content_hash = Some("unique-old".into());
    svc.ingest(entry).await.unwrap();

    // Insert a recent entry
    let mut entry2 = activity_log::normalize_domain_event(&bus::DomainEvent::NoteCreated {
        note_id: "new-note".into(),
        title: "New".into(),
    });
    entry2.content_hash = Some("unique-new".into());
    svc.ingest(entry2).await.unwrap();

    let deleted = ActivityLogRepo::delete_older_than(&pool, 365)
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    let start = Utc::now() - Duration::days(500);
    let end = Utc::now() + Duration::hours(1);
    let remaining = ActivityLogRepo::query_range(&pool, start, end, 100, 0)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].resource_id.as_deref(), Some("new-note"));
}
