//! Integration tests for the unified activity log.

use std::sync::Arc;

use ai_core::{AiSignal, RecallDomain, SalienceVerdict, SignalConsumer};
use jiff::{SignedDuration, Timestamp};
use storage::StoragePool;

use activity_log::{
    ActivityIngestionService, ActivityLog, ActivityLogRepo, ActivitySource, ChatMessageInput,
    ChatMessageNormalizer, NormalizerSignalConsumer, PrivacyFilter, WindowEventInput,
    WindowEventNormalizer,
};

async fn setup() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &ActivityLog::migrations_static())
        .await
        .unwrap();
    pool
}

fn task_created_signal(task_id: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "TaskCreated",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: format!("Task created: {task_id}"),
        entity: Some(ai_core::EntityRef {
            entity_type: "task",
            id: task_id.into(),
            name: String::new(),
        }),
        timestamp: Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

fn task_completed_signal(task_id: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::Tasks,
        event_kind: "TaskCompleted",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: format!("Task completed: {task_id}"),
        entity: Some(ai_core::EntityRef {
            entity_type: "task",
            id: task_id.into(),
            name: String::new(),
        }),
        timestamp: Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

fn note_created_signal(note_id: &str, title: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::Notes,
        event_kind: "NoteCreated",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: format!("Note created: {title}"),
        entity: Some(ai_core::EntityRef {
            entity_type: "note",
            id: note_id.into(),
            name: title.into(),
        }),
        timestamp: Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

fn focus_session_started_signal(session_type: &str) -> AiSignal {
    AiSignal {
        domain: RecallDomain::Productivity,
        event_kind: "FocusSessionStarted",
        importance: 0.5,
        salience: SalienceVerdict::Accumulate,
        content: format!("Focus session started: {session_type}"),
        entity: Some(ai_core::EntityRef {
            entity_type: "focus_session",
            id: String::new(),
            name: session_type.into(),
        }),
        timestamp: Timestamp::now(),
        raw_event: None,
        metrics: ai_core::AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    }
}

/// Full flow: AiSignal → consumer normalizes → ingests → query returns it.
#[tokio::test]
async fn test_signal_consumer_full_flow() {
    let pool = setup().await;
    let svc = Arc::new(ActivityIngestionService::new(
        pool.clone(),
        PrivacyFilter::default(),
    ));
    let consumer = NormalizerSignalConsumer::new(Arc::clone(&svc));

    consumer
        .consume(&task_created_signal("integration-t1"))
        .await
        .unwrap();

    let start = Timestamp::now() - SignedDuration::from_secs(3600);
    let end = Timestamp::now() + SignedDuration::from_secs(3600);
    let results = ActivityLogRepo::query_range(&pool, start, end, 100, 0)
        .await
        .unwrap();

    assert!(!results.is_empty(), "Should have at least one entry");
    let entry = &results[0];
    assert_eq!(entry.source, ActivitySource::Task);
    assert_eq!(entry.action, "create");
    assert_eq!(entry.resource_id.as_deref(), Some("integration-t1"));
}

/// Full flow: multiple signals → consumer ingests all → query returns them.
#[tokio::test]
async fn test_multiple_signals() {
    let pool = setup().await;
    let svc = Arc::new(ActivityIngestionService::new(
        pool.clone(),
        PrivacyFilter::default(),
    ));
    let consumer = NormalizerSignalConsumer::new(Arc::clone(&svc));

    consumer
        .consume(&focus_session_started_signal("deep_work"))
        .await
        .unwrap();
    consumer
        .consume(&note_created_signal("n-100", "Integration Note"))
        .await
        .unwrap();

    let start = Timestamp::now() - SignedDuration::from_secs(3600);
    let end = Timestamp::now() + SignedDuration::from_secs(3600);

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

    let start = Timestamp::now() - SignedDuration::from_secs(3600);
    let end = Timestamp::now() + SignedDuration::from_secs(3600);

    let results = ActivityLogRepo::query_by_session(&pool, "telegram:user123", start, end)
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
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
        started_at: Timestamp::now(),
        duration_secs: Some(300),
        project_id: Some("proj-klyntbot".into()),
        is_idle: false,
    };

    svc.normalize_and_ingest(&normalizer, &input).await.unwrap();

    let start = Timestamp::now() - SignedDuration::from_secs(3600);
    let end = Timestamp::now() + SignedDuration::from_secs(3600);

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
    let consumer = NormalizerSignalConsumer::new(Arc::clone(&svc));

    consumer.consume(&task_created_signal("t1")).await.unwrap();
    consumer
        .consume(&task_completed_signal("t1"))
        .await
        .unwrap();
    consumer
        .consume(&note_created_signal("n1", "Test"))
        .await
        .unwrap();

    let start = Timestamp::now() - SignedDuration::from_secs(3600);
    let end = Timestamp::now() + SignedDuration::from_secs(3600);

    let counts = ActivityLogRepo::count_by_source(&pool, start, end)
        .await
        .unwrap();

    assert_eq!(counts.get("task"), Some(&2));
    assert_eq!(counts.get("note"), Some(&1));
}

/// Dedup: same content_hash → only first is stored.
#[tokio::test]
async fn test_dedup_across_ingestion() {
    let pool = setup().await;
    let svc = ActivityIngestionService::new(pool.clone(), PrivacyFilter::default());
    let consumer = NormalizerSignalConsumer::new(Arc::new(svc));

    let signal = task_created_signal("dedup-test");

    // Consume twice — same signal produces same content_hash
    consumer.consume(&signal).await.unwrap();
    consumer.consume(&signal).await.unwrap();

    let start = Timestamp::now() - SignedDuration::from_secs(3600);
    let end = Timestamp::now() + SignedDuration::from_secs(3600);
    let results = ActivityLogRepo::query_range(&pool, start, end, 100, 0)
        .await
        .unwrap();

    assert_eq!(results.len(), 1, "Duplicate should be skipped by dedup");
}

/// Retention cleanup: delete_older_than removes old entries.
#[tokio::test]
async fn test_retention_cleanup() {
    let pool = setup().await;
    let svc = ActivityIngestionService::new(pool.clone(), PrivacyFilter::default());
    let consumer = NormalizerSignalConsumer::new(Arc::new(svc));

    let mut old_signal = note_created_signal("old-note", "Old");
    old_signal.timestamp = Timestamp::now() - SignedDuration::from_secs(400 * 86400);
    consumer.consume(&old_signal).await.unwrap();

    let mut new_signal = note_created_signal("new-note", "New");
    new_signal.timestamp = Timestamp::now();
    consumer.consume(&new_signal).await.unwrap();

    let deleted = ActivityLogRepo::delete_older_than(&pool, 365)
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    let start = Timestamp::now() - SignedDuration::from_secs(500 * 86400);
    let end = Timestamp::now() + SignedDuration::from_secs(3600);
    let remaining = ActivityLogRepo::query_range(&pool, start, end, 100, 0)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].resource_id.as_deref(), Some("new-note"));
}
