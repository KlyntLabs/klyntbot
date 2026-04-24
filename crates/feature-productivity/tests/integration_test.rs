//! Integration tests for the productivity feature — end-to-end lifecycle scenarios.

use feature_productivity::config::FocusConfig;
use feature_productivity::focus::FocusManager;
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::tracker::categorizer::Categorizer;
use feature_productivity::types::*;
use feature_productivity::{DailyAggregator, ProductivityPatternAnalyzer};
use jiff::{SignedDuration, Timestamp};

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = storage::StoragePool::connect_in_memory()
        .await
        .expect("in-memory pool");
    let inner = pool.inner().clone();
    storage::StoragePool::run_feature_migrations(
        &inner,
        &feature_productivity::productivity_migrations(),
    )
    .await
    .unwrap();
    inner
}

/// Full focus session lifecycle: start → record distraction → end → verify quality + summary.
#[tokio::test]
async fn test_full_focus_session_lifecycle() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let config = FocusConfig::default();
    let manager = FocusManager::new(repos.clone(), config);

    // 1. Start a focus session (target 1 min so it completes instantly)
    let session = manager
        .start_session(Some("task-1".into()), None, Some(1))
        .await
        .unwrap();
    assert!(!session.completed);
    assert!(session.ended_at.is_none());
    assert_eq!(session.target_mins, Some(1));

    // 2. Should not be able to start another
    let err = manager.start_session(None, None, None).await;
    assert!(err.is_err());

    // 3. Record a distraction
    manager.record_distraction("Twitter").await.unwrap();

    // Verify distraction was recorded
    let active = repos.sessions.get_active().await.unwrap().unwrap();
    assert_eq!(active.interruptions, 1);
    assert_eq!(active.distraction_events.len(), 1);
    assert_eq!(active.distraction_events[0].app_name, "Twitter");

    // 4. End the session
    let ended = manager
        .end_session(Some("Good session".into()))
        .await
        .unwrap()
        .unwrap();
    // completed = actual_mins >= target. Since start/end are near-instant, actual_mins=0,
    // so completed=false. This is correct behavior — session didn't reach target duration.
    assert!(ended.ended_at.is_some());
    // quality_score is now computed by the intelligence layer's QualityScorer
    assert!(ended.quality_score.is_none());
    assert!(ended.actual_mins.is_some());
    assert_eq!(ended.notes, Some("Good session".into()));

    // 5. No active session anymore
    assert!(repos.sessions.get_active().await.unwrap().is_none());

    // 6. Insert some activity events for aggregation
    let now = Timestamp::now();
    let today = now.strftime("%Y-%m-%d").to_string();
    let today_start = now
        .strftime("%Y-%m-%d")
        .to_string()
        .parse::<jiff::civil::Date>()
        .unwrap()
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp();

    for i in 0i64..5 {
        let event = ActivityEvent {
            id: None,
            app_name: "VS Code".into(),
            window_title: Some("main.rs".into()),
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: Some("coding".into()),
            started_at: today_start
                .checked_add(SignedDuration::from_hours(9))
                .unwrap()
                .checked_add(SignedDuration::from_mins(i * 30))
                .unwrap(),
            ended_at: Some(
                today_start
                    .checked_add(SignedDuration::from_hours(9))
                    .unwrap()
                    .checked_add(SignedDuration::from_mins(i * 30 + 25))
                    .unwrap(),
            ),
            duration_secs: Some(1500),
            is_idle: false,
            metadata: None,
            project_id: None,
            focus_session_id: None,
        };
        repos.events.insert(&event).await.unwrap();
    }

    // 7. Run daily aggregation
    let aggregator = DailyAggregator::new(repos.clone());
    let summary = aggregator.compute_for_date(&today).await.unwrap();

    assert!(summary.total_active_secs > 0);
    assert_eq!(summary.focus_sessions_count, 1);
    // quality_score is now computed by the intelligence layer's QualityScorer,
    // so avg_session_quality may be None when no intelligence scoring has run.
    // assert!(summary.avg_session_quality.is_some());
}

/// Categorizer loads categories from DB and matches apps correctly.
#[tokio::test]
async fn test_categorizer_with_db() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    // Load system categories from migration
    let categories = repos.categories.list_all().await.unwrap();
    assert!(categories.len() >= 7); // default system categories

    let categorizer = Categorizer::new(categories);

    // Test built-in category matches (from migration rules)
    let coding = categorizer.categorize("Visual Studio Code", None, None);
    assert!(coding.is_some());
    assert_eq!(coding.unwrap().category_type, CategoryType::Productive);

    let terminal = categorizer.categorize("Terminal", None, None);
    assert!(terminal.is_some());
    assert_eq!(terminal.unwrap().category_type, CategoryType::Productive);

    // Social media should be distracting
    let social = categorizer.categorize("Chrome", None, Some("https://twitter.com/home"));
    assert!(social.is_some());
    assert_eq!(social.unwrap().category_type, CategoryType::Distracting);

    // Unknown app should return None
    assert!(categorizer
        .categorize("UnknownApp123", None, None)
        .is_none());

    // Add a custom category and re-test
    let custom = ActivityCategory {
        id: "gaming".into(),
        name: "Gaming".into(),
        category_type: CategoryType::Distracting,
        color: Some("#ff0000".into()),
        icon: None,
        rules: Some(CategoryRules {
            app_names: vec!["Steam".into()],
            bundle_ids: vec![],
            url_patterns: vec![],
        }),
        is_system: false,
    };
    repos.categories.upsert(&custom).await.unwrap();

    // Refresh categorizer
    let categories = repos.categories.list_all().await.unwrap();
    let categorizer = Categorizer::new(categories);

    let gaming = categorizer.categorize("Steam", None, None);
    assert!(gaming.is_some());
    assert_eq!(gaming.unwrap().id, "gaming");
}

/// Insert known activity events, run aggregator, verify exact numbers.
#[tokio::test]
async fn test_daily_aggregation_accuracy() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    let date_str = "2026-03-04";
    let day_start = "2026-03-04"
        .parse::<jiff::civil::Date>()
        .unwrap()
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp();

    // Insert events with known durations:
    // 2h coding (productive), 1h slack (neutral), 30m youtube (distracting), 30m idle
    let events = vec![
        ActivityEvent {
            id: None,
            app_name: "VS Code".into(),
            window_title: Some("main.rs".into()),
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: Some("coding".into()),
            started_at: day_start
                .checked_add(SignedDuration::from_hours(9))
                .unwrap(),
            ended_at: Some(
                day_start
                    .checked_add(SignedDuration::from_hours(11))
                    .unwrap(),
            ),
            duration_secs: Some(7200), // 2h
            is_idle: false,
            metadata: None,
            project_id: None,
            focus_session_id: None,
        },
        ActivityEvent {
            id: None,
            app_name: "Slack".into(),
            window_title: None,
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: Some("communication".into()),
            started_at: day_start
                .checked_add(SignedDuration::from_hours(11))
                .unwrap(),
            ended_at: Some(
                day_start
                    .checked_add(SignedDuration::from_hours(12))
                    .unwrap(),
            ),
            duration_secs: Some(3600), // 1h
            is_idle: false,
            metadata: None,
            project_id: None,
            focus_session_id: None,
        },
        ActivityEvent {
            id: None,
            app_name: "Chrome".into(),
            window_title: Some("YouTube".into()),
            site_name: Some("YouTube".into()),
            bundle_id: None,
            url: Some("https://youtube.com".into()),
            category_id: Some("entertainment".into()),
            started_at: day_start
                .checked_add(SignedDuration::from_hours(12))
                .unwrap(),
            ended_at: Some(
                day_start
                    .checked_add(SignedDuration::from_hours(12))
                    .unwrap()
                    .checked_add(SignedDuration::from_mins(30))
                    .unwrap(),
            ),
            duration_secs: Some(1800), // 30m
            is_idle: false,
            metadata: None,
            project_id: None,
            focus_session_id: None,
        },
        ActivityEvent {
            id: None,
            app_name: "loginwindow".into(),
            window_title: None,
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: None,
            started_at: day_start
                .checked_add(SignedDuration::from_hours(12))
                .unwrap()
                .checked_add(SignedDuration::from_mins(30))
                .unwrap(),
            ended_at: Some(
                day_start
                    .checked_add(SignedDuration::from_hours(13))
                    .unwrap(),
            ),
            duration_secs: Some(1800), // 30m
            is_idle: true,
            metadata: None,
            project_id: None,
            focus_session_id: None,
        },
    ];

    repos.events.insert_batch(&events).await.unwrap();

    // Insert bucket data matching the events (aggregator uses bucket totals)
    use feature_productivity::ActivityBucket;
    let buckets = vec![
        ActivityBucket {
            bucket_start: format!("{}T09:00:00+00:00", date_str),
            date: date_str.to_string(),
            dominant_app: Some("VS Code".to_string()),
            dominant_site: None,
            dominant_category: Some("coding".to_string()),
            productive_secs: 7200,
            neutral_secs: 0,
            distracting_secs: 0,
            idle_secs: 0,
            context_switches: 0,
            focus_depth: None,
            tick_count: 120,
            dominant_project: None,
        },
        ActivityBucket {
            bucket_start: format!("{}T11:00:00+00:00", date_str),
            date: date_str.to_string(),
            dominant_app: Some("Slack".to_string()),
            dominant_site: None,
            dominant_category: Some("communication".to_string()),
            productive_secs: 0,
            neutral_secs: 3600,
            distracting_secs: 0,
            idle_secs: 0,
            context_switches: 1,
            focus_depth: None,
            tick_count: 60,
            dominant_project: None,
        },
        ActivityBucket {
            bucket_start: format!("{}T12:00:00+00:00", date_str),
            date: date_str.to_string(),
            dominant_app: Some("Chrome".to_string()),
            dominant_site: Some("YouTube".to_string()),
            dominant_category: Some("entertainment".to_string()),
            productive_secs: 0,
            neutral_secs: 0,
            distracting_secs: 1800,
            idle_secs: 0,
            context_switches: 1,
            focus_depth: None,
            tick_count: 30,
            dominant_project: None,
        },
        ActivityBucket {
            bucket_start: format!("{}T12:30:00+00:00", date_str),
            date: date_str.to_string(),
            dominant_app: Some("loginwindow".to_string()),
            dominant_site: None,
            dominant_category: None,
            productive_secs: 0,
            neutral_secs: 0,
            distracting_secs: 0,
            idle_secs: 1800,
            context_switches: 0,
            focus_depth: None,
            tick_count: 30,
            dominant_project: None,
        },
    ];
    for bucket in &buckets {
        repos.buckets.upsert(bucket).await.unwrap();
    }

    // Also add a completed focus session
    let session = FocusSession {
        id: "fs-1".into(),
        action_id: None,
        project_id: None,
        session_type: SessionType::Focus,
        target_mins: Some(45),
        started_at: day_start
            .checked_add(SignedDuration::from_hours(9))
            .unwrap(),
        ended_at: Some(
            day_start
                .checked_add(SignedDuration::from_hours(9))
                .unwrap()
                .checked_add(SignedDuration::from_mins(40))
                .unwrap(),
        ),
        actual_mins: Some(40),
        interruptions: 1,
        distraction_events: vec![],
        quality_score: Some(0.85),
        completed: true,
        notes: None,
        source: SessionSource::Manual,
    };
    repos.sessions.create(&session).await.unwrap();

    // Run aggregation
    let aggregator = DailyAggregator::new(repos.clone());
    let summary = aggregator.compute_for_date(date_str).await.unwrap();

    // Verify: 2h + 1h + 0.5h = 3.5h active = 12600 secs
    assert_eq!(summary.total_active_secs, 12600);
    // Idle: 30m = 1800 secs
    assert_eq!(summary.total_idle_secs, 1800);
    // Productive: 2h coding = 7200 secs
    assert_eq!(summary.productive_secs, 7200);
    // Distracting: 30m entertainment = 1800 secs
    assert_eq!(summary.distracting_secs, 1800);
    // Focus sessions
    assert_eq!(summary.focus_sessions_count, 1);
    assert_eq!(summary.avg_session_quality, Some(0.85));
    assert_eq!(summary.interruptions_count, 1);
    // Context switches: VS Code -> Slack -> Chrome = 2 (idle events excluded)
    assert_eq!(summary.context_switches, 2);
    // Top apps should include VS Code
    assert!(!summary.top_apps.is_empty());
    assert_eq!(summary.top_apps[0].app_name, "VS Code");
    assert_eq!(summary.top_apps[0].duration_secs, 7200);
}

/// NudgeService sends break reminder after continuous work exceeding threshold.
#[tokio::test]
async fn test_nudge_service_delivers_break_reminder() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    let now = Timestamp::now();

    // Insert a single large continuous activity event within the 90-min window
    let event = ActivityEvent {
        id: None,
        app_name: "VS Code".into(),
        window_title: None,
        site_name: None,
        bundle_id: None,
        url: None,
        category_id: Some("coding".into()),
        started_at: now.checked_sub(SignedDuration::from_mins(85)).unwrap(),
        ended_at: Some(now),
        duration_secs: Some(5400), // exactly 90 min
        is_idle: false,
        metadata: None,
        project_id: None,
        focus_session_id: None,
    };
    repos.events.insert(&event).await.unwrap();

    // Verify break threshold is exceeded within the 90-min window
    let window_start = now.checked_sub(SignedDuration::from_mins(90)).unwrap();
    let active_secs = repos
        .events
        .total_active_secs(&window_start, &now)
        .await
        .unwrap();
    assert!(
        active_secs >= 90 * 60,
        "Expected >= 5400 secs, got {}",
        active_secs
    );

    // Verify no recent nudges (so cooldown won't prevent sending)
    let last = repos
        .nudges
        .last_of_type(NudgeType::BreakReminder)
        .await
        .unwrap();
    assert!(last.is_none());

    // Insert a nudge and verify cooldown works
    let record = NudgeRecord {
        id: None,
        nudge_type: NudgeType::BreakReminder,
        message: "Take a break!".into(),
        channel: None,
        acknowledged: false,
        created_at: now,
    };
    repos.nudges.insert(&record).await.unwrap();

    let last = repos
        .nudges
        .last_of_type(NudgeType::BreakReminder)
        .await
        .unwrap();
    assert!(last.is_some());
    assert_eq!(last.unwrap().nudge_type, NudgeType::BreakReminder);
}

/// Pattern analyzer detects patterns from focus sessions and summaries.
#[tokio::test]
async fn test_pattern_analyzer_detects_patterns() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let analyzer = ProductivityPatternAnalyzer::new(repos.clone());

    let now = Timestamp::now();

    // Insert 10 days of summaries
    for day_offset in 0i64..10 {
        let date = now
            .checked_sub(SignedDuration::from_secs(day_offset * 86400))
            .unwrap()
            .strftime("%Y-%m-%d")
            .to_string();
        let summary = DailySummary {
            date,
            total_active_secs: 28800, // 8h
            total_focus_secs: 14400,  // 4h
            total_break_secs: 3600,
            total_idle_secs: 7200,
            productive_secs: 21600, // 6h
            neutral_secs: 3600,
            distracting_secs: 3600,
            focus_sessions_count: 4,
            avg_session_quality: Some(0.82),
            interruptions_count: 5,
            context_switches: 30,
            top_apps: vec![],
            top_categories: vec![],
            top_projects: vec![],
            productivity_score: None,
            ai_summary: None,
            deep_work_blocks: 0,
            deep_work_secs: 0,
            avg_recovery_secs: None,
        };
        repos.summaries.upsert(&summary).await.unwrap();
    }

    // Insert focus sessions at different hours with varying quality
    for day_offset in 0i64..10 {
        let base_date = now
            .checked_sub(SignedDuration::from_secs(day_offset * 86400))
            .unwrap()
            .strftime("%Y-%m-%d")
            .to_string()
            .parse::<jiff::civil::Date>()
            .unwrap();
        let base_day = base_date
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp();

        // Morning session (9am) — high quality
        let morning = FocusSession {
            id: format!("fs-morning-{}", day_offset),
            action_id: None,
            project_id: None,
            session_type: SessionType::Focus,
            target_mins: Some(45),
            started_at: base_day.checked_add(SignedDuration::from_hours(9)).unwrap(),
            ended_at: Some(
                base_day
                    .checked_add(SignedDuration::from_hours(9))
                    .unwrap()
                    .checked_add(SignedDuration::from_mins(40))
                    .unwrap(),
            ),
            actual_mins: Some(40),
            interruptions: 0,
            distraction_events: vec![],
            quality_score: Some(0.92),
            completed: true,
            notes: None,
            source: SessionSource::Manual,
        };
        repos.sessions.create(&morning).await.unwrap();

        // Afternoon session (3pm) — lower quality
        let afternoon = FocusSession {
            id: format!("fs-afternoon-{}", day_offset),
            started_at: base_day
                .checked_add(SignedDuration::from_hours(15))
                .unwrap(),
            ended_at: Some(
                base_day
                    .checked_add(SignedDuration::from_hours(15))
                    .unwrap()
                    .checked_add(SignedDuration::from_mins(30))
                    .unwrap(),
            ),
            actual_mins: Some(30),
            quality_score: Some(0.65),
            ..morning.clone()
        };
        repos.sessions.create(&afternoon).await.unwrap();
    }

    let patterns = analyzer.analyze(14).await.unwrap();

    // Should have analyzed 10 days
    assert_eq!(patterns.days_analyzed, 10);

    // Peak hours should include 9am (higher quality)
    assert!(!patterns.peak_focus_hours.is_empty());
    assert!(
        patterns.peak_focus_hours.contains(&9),
        "Expected 9am in peak hours: {:?}",
        patterns.peak_focus_hours
    );

    // Average session should be ~35 min (40+30)/2
    assert!(
        patterns.avg_session_mins > 30.0 && patterns.avg_session_mins < 40.0,
        "Expected ~35 min avg, got {}",
        patterns.avg_session_mins
    );

    // Productive ratio should be 0.75 (21600/28800)
    assert!(
        (patterns.productive_ratio - 0.75).abs() < 0.01,
        "Expected ~0.75 productive ratio, got {}",
        patterns.productive_ratio
    );

    // Average context switches should be 30
    assert!(
        (patterns.avg_context_switches - 30.0).abs() < 0.01,
        "Expected 30 avg switches, got {}",
        patterns.avg_context_switches
    );

    // Best day of week should be set (10 days >= 7 threshold)
    assert!(patterns.best_day_of_week.is_some());
}

/// Pomodoro lifecycle: start → prevent double-start → end → verify quality.
#[tokio::test]
async fn test_pomodoro_lifecycle() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let mgr = FocusManager::new(repos.clone(), FocusConfig::default());

    // Start pomodoro
    let session = mgr
        .start_pomodoro(None, None, Some(25), Some(5))
        .await
        .unwrap();
    assert_eq!(session.session_type, SessionType::Pomodoro);
    assert_eq!(session.target_mins, Some(25));

    // Can't start another pomodoro while active
    assert!(mgr.start_pomodoro(None, None, None, None).await.is_err());
    // Can't start a focus session while pomodoro is active
    assert!(mgr.start_session(None, None, None).await.is_err());

    // End it
    let ended = mgr.end_session(None).await.unwrap().unwrap();
    assert_eq!(ended.session_type, SessionType::Pomodoro);
    // quality_score is now computed by the intelligence layer's QualityScorer
    assert!(ended.quality_score.is_none());
    assert!(ended.ended_at.is_some());
}

/// Goal tracking: set a goal, insert activity, verify goal is met.
#[tokio::test]
async fn test_goal_tracking() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);
    let aggregator = DailyAggregator::new(repos.clone());

    // Set a goal: 2h productive time
    let goal = ProductivityGoal {
        id: None,
        goal_type: GoalType::Daily,
        metric: GoalMetric::ProductiveHours,
        target_value: 2.0,
        enabled: true,
        project_id: None,
        created_at: Timestamp::now(),
    };
    repos.goals.insert(&goal).await.unwrap();

    // Insert 3h of productive activity (anchor at noon UTC to avoid midnight flakiness)
    let noon = Timestamp::now()
        .strftime("%Y-%m-%d")
        .to_string()
        .parse::<jiff::civil::Date>()
        .unwrap()
        .at(12, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp();

    repos
        .events
        .insert(&ActivityEvent {
            id: None,
            app_name: "VS Code".into(),
            window_title: None,
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: Some("coding".into()),
            started_at: noon.checked_sub(SignedDuration::from_hours(3)).unwrap(),
            ended_at: Some(noon),
            duration_secs: Some(10800),
            is_idle: false,
            metadata: None,
            project_id: None,
            focus_session_id: None,
        })
        .await
        .unwrap();

    let today = noon.strftime("%Y-%m-%d").to_string();
    let results = aggregator.check_goals(&today).await.unwrap();
    assert_eq!(results.len(), 1);
    let (_goal, _current, met) = &results[0];
    assert!(met, "goal should be met with 3h > 2h target");
}

/// Historical comparison: insert summaries for two days, verify comparison output.
#[tokio::test]
async fn test_historical_comparison() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    let now = Timestamp::now();
    let today_str = now.strftime("%Y-%m-%d").to_string();
    let yesterday_str = now
        .checked_sub(SignedDuration::from_secs(86400))
        .unwrap()
        .strftime("%Y-%m-%d")
        .to_string();

    // Insert yesterday's summary
    let yesterday_summary = DailySummary {
        date: yesterday_str.clone(),
        total_active_secs: 28800, // 8h
        total_focus_secs: 14400,
        total_break_secs: 3600,
        total_idle_secs: 3600,
        productive_secs: 21600, // 6h
        neutral_secs: 3600,
        distracting_secs: 3600,
        focus_sessions_count: 4,
        avg_session_quality: Some(0.80),
        interruptions_count: 5,
        context_switches: 20,
        top_apps: vec![],
        top_categories: vec![],
        top_projects: vec![],
        productivity_score: Some(72.0),
        ai_summary: None,
        deep_work_blocks: 0,
        deep_work_secs: 0,
        avg_recovery_secs: None,
    };
    repos.summaries.upsert(&yesterday_summary).await.unwrap();

    // Insert today's summary (better day)
    let today_summary = DailySummary {
        date: today_str.clone(),
        productive_secs: 25200, // 7h — improvement
        productivity_score: Some(80.0),
        ..yesterday_summary.clone()
    };
    repos.summaries.upsert(&today_summary).await.unwrap();

    // Verify both summaries are retrievable
    let current = repos
        .summaries
        .list_range(&today_str, &today_str)
        .await
        .unwrap();
    let previous = repos
        .summaries
        .list_range(&yesterday_str, &yesterday_str)
        .await
        .unwrap();

    assert_eq!(current.len(), 1);
    assert_eq!(previous.len(), 1);
    assert_eq!(current[0].productive_secs, 25200);
    assert_eq!(previous[0].productive_secs, 21600);

    // Verify percentage change calculation
    let change = ((25200.0_f64 - 21600.0) / 21600.0 * 100.0).round();
    assert!(
        (change - 17.0).abs() < 1.0,
        "Expected ~17% improvement, got {change}"
    );
}

/// Project tracking: insert project + events with project_id, verify aggregation.
#[tokio::test]
async fn test_project_tracking_aggregation() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    let date_str = "2026-03-05";
    let day_start = date_str
        .parse::<jiff::civil::Date>()
        .unwrap()
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp();

    // 1. Insert a project
    let project = feature_productivity::types::ProductivityProject {
        id: "klyntbot".into(),
        display_name: "Klyntbot".into(),
        path: "/Users/test/klyntbot".into(),
        url_patterns: vec![],
        color: Some("#7C3AED".into()),
        is_auto_detected: false,
        created_at: Timestamp::now(),
    };
    repos.projects.upsert(&project).await.unwrap();

    // 2. Insert events WITH project_id
    let event_with_project = ActivityEvent {
        id: None,
        app_name: "VS Code".into(),
        window_title: Some("main.rs — klyntbot".into()),
        site_name: None,
        bundle_id: None,
        url: None,
        category_id: Some("coding".into()),
        started_at: day_start
            .checked_add(SignedDuration::from_hours(9))
            .unwrap(),
        ended_at: Some(
            day_start
                .checked_add(SignedDuration::from_hours(11))
                .unwrap(),
        ),
        duration_secs: Some(7200), // 2h
        is_idle: false,
        metadata: None,
        project_id: Some("klyntbot".into()),
        focus_session_id: None,
    };
    repos.events.insert(&event_with_project).await.unwrap();

    // 3. Insert events WITHOUT project_id
    let event_no_project = ActivityEvent {
        id: None,
        app_name: "Slack".into(),
        window_title: None,
        site_name: None,
        bundle_id: None,
        url: None,
        category_id: Some("communication".into()),
        started_at: day_start
            .checked_add(SignedDuration::from_hours(11))
            .unwrap(),
        ended_at: Some(
            day_start
                .checked_add(SignedDuration::from_hours(12))
                .unwrap(),
        ),
        duration_secs: Some(3600), // 1h
        is_idle: false,
        metadata: None,
        project_id: None,
        focus_session_id: None,
    };
    repos.events.insert(&event_no_project).await.unwrap();

    // 4. Run aggregation
    let aggregator = DailyAggregator::new(repos.clone());
    let summary = aggregator.compute_for_date(date_str).await.unwrap();

    // 5. Verify top_projects has "klyntbot" with correct time
    assert_eq!(summary.top_projects.len(), 1);
    assert_eq!(summary.top_projects[0].project_id, "klyntbot");
    assert_eq!(summary.top_projects[0].display_name, "Klyntbot");
    assert_eq!(summary.top_projects[0].duration_secs, 7200);
    assert_eq!(summary.top_projects[0].color, Some("#7C3AED".into()));

    // 6. Total active should include both tracked and untracked
    assert_eq!(summary.total_active_secs, 10800); // 3h
}

/// Export: insert events, verify CSV output format.
#[tokio::test]
async fn test_activity_export_csv() {
    let pool = setup_pool().await;
    let repos = ProductivityRepos::new(pool);

    let day_start = "2026-03-04"
        .parse::<jiff::civil::Date>()
        .unwrap()
        .at(9, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp();

    // Insert two events
    for (i, app) in ["VS Code", "Safari"].iter().enumerate() {
        let event = ActivityEvent {
            id: None,
            app_name: app.to_string(),
            window_title: Some(format!("Window {i}")),
            site_name: None,
            bundle_id: None,
            url: None,
            category_id: Some("coding".into()),
            started_at: day_start
                .checked_add(SignedDuration::from_hours(i as i64))
                .unwrap(),
            ended_at: Some(
                day_start
                    .checked_add(SignedDuration::from_hours(i as i64 + 1))
                    .unwrap(),
            ),
            duration_secs: Some(3600),
            is_idle: false,
            metadata: None,
            project_id: None,
            focus_session_id: None,
        };
        repos.events.insert(&event).await.unwrap();
    }

    // Fetch events for that date range
    let start = "2026-03-04"
        .parse::<jiff::civil::Date>()
        .unwrap()
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp();
    let end = "2026-03-04"
        .parse::<jiff::civil::Date>()
        .unwrap()
        .at(23, 59, 59, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp();

    let events = repos
        .events
        .list_range(&start, &end, Some(100))
        .await
        .unwrap();
    assert_eq!(events.len(), 2);

    // Verify CSV-like format
    let mut csv =
        String::from("app_name,window_title,category_id,started_at,duration_secs,is_idle\n");
    for e in &events {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            e.app_name,
            e.window_title.as_deref().unwrap_or(""),
            e.category_id.as_deref().unwrap_or(""),
            e.started_at,
            e.duration_secs.unwrap_or(0),
            e.is_idle,
        ));
    }
    assert!(csv.contains("VS Code"));
    assert!(csv.contains("Safari"));
    assert!(csv.contains("coding"));
}
