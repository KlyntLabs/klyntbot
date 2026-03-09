use chrono::{Duration, Utc};
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::types::*;
use sqlx::SqlitePool;

async fn setup_pool() -> SqlitePool {
    let pool = storage::StoragePool::connect_in_memory()
        .await
        .expect("in-memory pool");
    let inner = pool.inner().clone();
    storage::StoragePool::run_feature_migrations(
        &inner,
        &feature_productivity::ProductivityFeature::migrations_static(),
    )
    .await
    .unwrap();
    inner
}

#[tokio::test]
async fn test_insert_and_list_activity_events() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let now = Utc::now();

    for i in 0..3 {
        let event = ActivityEvent {
            id: None,
            app_name: format!("App{}", i),
            window_title: Some(format!("Window {}", i)),
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: Some("coding".to_string()),
            started_at: now + Duration::seconds(i * 10),
            ended_at: Some(now + Duration::seconds(i * 10 + 5)),
            duration_secs: Some(5),
            is_idle: false,
            metadata: None,
            project_id: None,
        };
        repos.events.insert(&event).await.unwrap();
    }

    let events = repos
        .events
        .list_range(
            &(now - Duration::seconds(1)),
            &(now + Duration::seconds(60)),
            None,
        )
        .await
        .unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].app_name, "App0");
    assert_eq!(events[2].app_name, "App2");
}

#[tokio::test]
async fn test_batch_insert_events() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let now = Utc::now();

    let events: Vec<ActivityEvent> = (0..10)
        .map(|i| ActivityEvent {
            id: None,
            app_name: format!("BatchApp{}", i),
            window_title: None,
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: None,
            started_at: now + Duration::seconds(i * 5),
            ended_at: Some(now + Duration::seconds(i * 5 + 4)),
            duration_secs: Some(4),
            is_idle: false,
            metadata: None,
            project_id: None,
        })
        .collect();

    repos.events.insert_batch(&events).await.unwrap();
    let result = repos
        .events
        .list_range(
            &(now - Duration::seconds(1)),
            &(now + Duration::seconds(100)),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.len(), 10);
}

#[tokio::test]
async fn test_category_crud() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    // System categories should already exist from migration
    let cats = repos.categories.list_all().await.unwrap();
    assert!(cats.len() >= 7, "Expected at least 7 default categories");

    let coding = repos.categories.get("coding").await.unwrap();
    assert!(coding.is_some());
    let coding = coding.unwrap();
    assert_eq!(coding.category_type, CategoryType::Productive);
    assert!(coding.is_system);

    // Upsert a custom category
    let custom = ActivityCategory {
        id: "my_custom_cat".to_string(),
        name: "My Custom".to_string(),
        category_type: CategoryType::Distracting,
        color: Some("#ff0000".to_string()),
        icon: None,
        rules: Some(CategoryRules {
            app_names: vec!["SomeApp".to_string()],
            bundle_ids: vec![],
            url_patterns: vec![],
        }),
        is_system: false,
    };
    repos.categories.upsert(&custom).await.unwrap();

    let fetched = repos
        .categories
        .get("my_custom_cat")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.name, "My Custom");
    assert_eq!(fetched.category_type, CategoryType::Distracting);

    // Delete custom (should work)
    assert!(repos.categories.delete("my_custom_cat").await.unwrap());

    // Delete system (should fail — is_system = TRUE)
    assert!(!repos.categories.delete("coding").await.unwrap());
}

#[tokio::test]
async fn test_focus_session_lifecycle() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let now = Utc::now();

    let session = FocusSession {
        id: "sess-1".to_string(),
        action_id: Some("task-1".to_string()),
        project_id: None,
        session_type: SessionType::Focus,
        target_mins: Some(45),
        started_at: now,
        ended_at: None,
        actual_mins: None,
        interruptions: 0,
        distraction_events: vec![],
        quality_score: None,
        completed: false,
        notes: None,
        source: SessionSource::Manual,
    };

    repos.sessions.create(&session).await.unwrap();

    // Should be active
    let active = repos.sessions.get_active().await.unwrap();
    assert!(active.is_some());
    assert_eq!(active.unwrap().id, "sess-1");

    // End session
    let mut ended = repos.sessions.get("sess-1").await.unwrap().unwrap();
    ended.ended_at = Some(now + Duration::minutes(40));
    ended.actual_mins = Some(40);
    ended.quality_score = Some(0.85);
    ended.completed = true;
    repos.sessions.update(&ended).await.unwrap();

    // Should no longer be active
    let active = repos.sessions.get_active().await.unwrap();
    assert!(active.is_none());

    // Verify update
    let fetched = repos.sessions.get("sess-1").await.unwrap().unwrap();
    assert!(fetched.completed);
    assert_eq!(fetched.quality_score, Some(0.85));
}

#[tokio::test]
async fn test_daily_summary_upsert() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    let summary = DailySummary {
        date: "2026-03-04".to_string(),
        total_active_secs: 3600,
        total_focus_secs: 1800,
        total_break_secs: 600,
        total_idle_secs: 1200,
        productive_secs: 2400,
        neutral_secs: 600,
        distracting_secs: 600,
        focus_sessions_count: 2,
        avg_session_quality: Some(0.8),
        interruptions_count: 3,
        context_switches: 15,
        top_apps: vec![AppUsage {
            app_name: "VS Code".to_string(),
            duration_secs: 2400,
            category: Some("coding".to_string()),
        }],
        top_categories: vec![CategoryUsage {
            category_id: "coding".to_string(),
            category: "Coding".to_string(),
            category_type: "productive".to_string(),
            duration_secs: 2400,
        }],
        top_projects: vec![],
        productivity_score: None,
        ai_summary: None,
        deep_work_blocks: 0,
        deep_work_secs: 0,
        avg_recovery_secs: None,
    };

    repos.summaries.upsert(&summary).await.unwrap();
    let fetched = repos.summaries.get("2026-03-04").await.unwrap().unwrap();
    assert_eq!(fetched.total_active_secs, 3600);
    assert_eq!(fetched.focus_sessions_count, 2);

    // Upsert with new values
    let updated = DailySummary {
        total_active_secs: 7200,
        ..summary
    };
    repos.summaries.upsert(&updated).await.unwrap();
    let fetched = repos.summaries.get("2026-03-04").await.unwrap().unwrap();
    assert_eq!(fetched.total_active_secs, 7200);
}

#[tokio::test]
async fn test_nudge_cooldown() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    let nudge = NudgeRecord {
        id: None,
        nudge_type: NudgeType::BreakReminder,
        message: "Time for a break!".to_string(),
        channel: None,
        acknowledged: false,
        created_at: Utc::now(),
    };

    let id = repos.nudges.insert(&nudge).await.unwrap();
    assert!(id > 0);

    let last = repos
        .nudges
        .last_of_type(NudgeType::BreakReminder)
        .await
        .unwrap();
    assert!(last.is_some());
    assert_eq!(last.unwrap().message, "Time for a break!");

    // No focus suggestion nudges yet
    let last = repos
        .nudges
        .last_of_type(NudgeType::FocusSuggestion)
        .await
        .unwrap();
    assert!(last.is_none());

    // Acknowledge
    repos.nudges.acknowledge(id).await.unwrap();
    let recent = repos.nudges.list_recent(10).await.unwrap();
    assert_eq!(recent.len(), 1);
    assert!(recent[0].acknowledged);
}

#[tokio::test]
async fn test_context_switch_count() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let now = Utc::now();

    // Insert alternating app events: A, B, A, B, B, C = 4 switches
    let apps = ["AppA", "AppB", "AppA", "AppB", "AppB", "AppC"];
    let events: Vec<ActivityEvent> = apps
        .iter()
        .enumerate()
        .map(|(i, name)| ActivityEvent {
            id: None,
            app_name: name.to_string(),
            window_title: None,
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: None,
            started_at: now + Duration::seconds(i as i64 * 10),
            ended_at: Some(now + Duration::seconds(i as i64 * 10 + 5)),
            duration_secs: Some(5),
            is_idle: false,
            metadata: None,
            project_id: None,
        })
        .collect();

    repos.events.insert_batch(&events).await.unwrap();

    let switches = repos
        .events
        .count_context_switches(
            &(now - Duration::seconds(1)),
            &(now + Duration::seconds(100)),
        )
        .await
        .unwrap();
    assert_eq!(switches, 4); // A->B, B->A, A->B, B->C
}

#[tokio::test]
async fn test_purge_old_events() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let now = Utc::now();
    let old = now - Duration::days(100);

    // Insert old and new events
    let old_event = ActivityEvent {
        id: None,
        app_name: "OldApp".to_string(),
        window_title: None,
        site_name: None,
        bundle_id: None,
        url: None,
        category_id: None,
        started_at: old,
        ended_at: Some(old + Duration::seconds(5)),
        duration_secs: Some(5),
        is_idle: false,
        metadata: None,
        project_id: None,
    };
    let new_event = ActivityEvent {
        started_at: now,
        ended_at: Some(now + Duration::seconds(5)),
        app_name: "NewApp".to_string(),
        ..old_event.clone()
    };

    repos.events.insert(&old_event).await.unwrap();
    repos.events.insert(&new_event).await.unwrap();

    // Purge events older than 90 days
    let cutoff = now - Duration::days(90);
    let purged = repos.events.purge_before(&cutoff).await.unwrap();
    assert_eq!(purged, 1);

    // Only new event remains
    let remaining = repos
        .events
        .list_range(&(old - Duration::days(1)), &(now + Duration::days(1)), None)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].app_name, "NewApp");
}
